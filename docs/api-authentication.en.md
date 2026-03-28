# API Authentication & API Key Management

## Overview

Sigma supports two authentication methods:

1. **JWT (Bearer Token)** — For web UI and interactive use. Login with email/password, receive a token.
2. **API Key (`X-Api-Key` header)** — For programmatic access. Each key has its own role and can be independently revoked.

## Roles

| Role | Description | Typical Use |
|------|-------------|-------------|
| `admin` | Full access to all endpoints, including user/API key management and audit logs | Platform administrators |
| `operator` | Read + write access to VPS, providers, DNS, cloud, tickets, costs, etc. Cannot manage users or API keys | Day-to-day fleet operators |
| `agent` | Read access + agent registration/heartbeat + Envoy node/route management only | VPS agents (sigma-agent) |
| `readonly` | Read-only access to all GET endpoints | Monitoring, dashboards, external read-only consumers |

### Permission Matrix

| Scope | admin | operator | agent | readonly |
|-------|:-----:|:--------:|:-----:|:--------:|
| All GET endpoints | Y | Y | Y | Y |
| VPS / Provider / DNS / Cloud CRUD | Y | Y | - | - |
| Tickets / IP Checks / Costs / Import | Y | Y | - | - |
| Agent register & heartbeat | Y | Y | Y | - |
| Envoy nodes & routes write | Y | Y | Y | - |
| User management | Y | - | - | - |
| API key management | Y | - | - | - |
| Audit logs / System settings | Y | - | - | - |

## JWT Authentication

### Login

```bash
curl -X POST https://api.example.com/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"email": "user@example.com", "password": "..."}'
```

Response:
```json
{
  "token": "eyJhbGciOiJIUzI1NiIs...",
  "user": { "id": "...", "email": "user@example.com", "role": "admin", ... }
}
```

### Using the Token

```bash
curl https://api.example.com/api/vps \
  -H "Authorization: Bearer eyJhbGciOiJIUzI1NiIs..."
```

Tokens expire after a configurable period (default: 24 hours). Use `POST /api/auth/refresh` to renew.

## API Key Authentication

### Creating an API Key (Admin Only)

Via API:
```bash
curl -X POST https://api.example.com/api/api-keys \
  -H "Authorization: Bearer <admin-token>" \
  -H "Content-Type: application/json" \
  -d '{"name": "my-agent-key", "role": "agent"}'
```

Response:
```json
{
  "id": "550e8400-...",
  "name": "my-agent-key",
  "key": "sk_sigma_a1b2c3d4e5f6...",
  "key_prefix": "sk_sigma",
  "role": "agent",
  "created_at": "2026-03-28T..."
}
```

> **Important:** The full key (`sk_sigma_...`) is only returned once at creation time. Copy and store it securely. It cannot be retrieved later.

Or via the Web UI: **Sidebar > API Keys > Create Key**

### Using an API Key

```bash
curl https://api.example.com/api/vps \
  -H "X-Api-Key: sk_sigma_a1b2c3d4e5f6..."
```

### Managing API Keys

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/api-keys` | List all keys (name, prefix, role, last_used_at) |
| `GET` | `/api/api-keys/{id}` | Get key details |
| `POST` | `/api/api-keys` | Create a new key |
| `DELETE` | `/api/api-keys/{id}` | Permanently delete (revoke) a key |

All management endpoints require `admin` role. Keys are stored as SHA-256 hashes in the database.

### Listing Keys

```bash
curl https://api.example.com/api/api-keys \
  -H "Authorization: Bearer <admin-token>"
```

Response:
```json
{
  "data": [
    {
      "id": "550e8400-...",
      "name": "my-agent-key",
      "key_prefix": "sk_sigma",
      "role": "agent",
      "last_used_at": "2026-03-28T12:00:00Z",
      "created_at": "2026-03-28T..."
    }
  ],
  "total": 1,
  "page": 1,
  "per_page": 25
}
```

### Revoking a Key

```bash
curl -X DELETE https://api.example.com/api/api-keys/550e8400-... \
  -H "Authorization: Bearer <admin-token>"
```

The key is permanently deleted. Any application using it will immediately receive `401 Unauthorized`.

## Agent Deployment Best Practices

Each VPS agent should use its own API key with the `agent` role:

```bash
# Create a per-agent key
curl -X POST https://api.example.com/api/api-keys \
  -H "Authorization: Bearer <admin-token>" \
  -H "Content-Type: application/json" \
  -d '{"name": "agent-hk-relay-01", "role": "agent"}'
```

Configure the agent with the returned key:
```bash
SIGMA_API_URL=https://api.example.com/api
SIGMA_API_KEY=sk_sigma_...
```

**Why per-agent keys?**

- **Blast radius** — If a VPS is compromised, revoke only that key without affecting other agents
- **Least privilege** — `agent` role can only register, heartbeat, and manage Envoy config. It cannot modify VPS records, providers, users, or any other resources
- **Audit trail** — Each API key has its own UUID; audit logs show exactly which agent performed each action
- **Monitoring** — `last_used_at` per key lets you detect stale or inactive agents

## Legacy API Key (`API_KEY` env var)

For backwards compatibility, the API still supports a single static API key via the `API_KEY` environment variable. This key always gets `admin` role.

This is intended as a migration path. For new deployments, use DB-managed API keys via `/api/api-keys` instead.

## Auth Priority

When a request arrives, the auth middleware checks in order:

1. `Authorization: Bearer <token>` — JWT
2. `X-Api-Key: <key>` — DB-managed keys (SHA-256 hash lookup)
3. `X-Api-Key: <key>` — Legacy `API_KEY` env var fallback
4. No `API_KEY` env set — Anonymous access with admin role (dev only, not recommended)
5. Otherwise — `401 Unauthorized`
