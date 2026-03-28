# API 认证与 API Key 管理

## 概述

Sigma 支持两种认证方式：

1. **JWT (Bearer Token)** — 用于 Web UI 和交互式访问。通过邮箱/密码登录获取 token。
2. **API Key (`X-Api-Key` 请求头)** — 用于程序化访问。每个 key 有独立的角色，可以单独吊销。

## 角色

| 角色 | 说明 | 典型使用场景 |
|------|------|-------------|
| `admin` | 完全访问所有端点，包��用户/API Key 管理和审计日志 | 平台管理员 |
| `operator` | 读写 VPS、Provider、DNS、Cloud、Ticket、费用等。不能管理用户和 API Key | 日常运维人员 |
| `agent` | 读取权限 + agent 注册/心跳 + Envoy 节点/路由管理 | VPS 上的 sigma-agent |
| `readonly` | 仅读取所有 GET 端点 | 监控、仪表盘、外部只读消费者 |

### 权限矩阵

| 范围 | admin | operator | agent | readonly |
|------|:-----:|:--------:|:-----:|:--------:|
| 所有 GET 端点 | Y | Y | Y | Y |
| VPS / Provider / DNS / Cloud 增删改 | Y | Y | - | - |
| Ticket / IP 检测 / 费用 / 导入 | Y | Y | - | - |
| Agent 注册与心跳 | Y | Y | Y | - |
| Envoy 节点与路由写操作 | Y | Y | Y | - |
| 用户管理 | Y | - | - | - |
| API Key 管理 | Y | - | - | - |
| 审计日志 / 系统设置 | Y | - | - | - |

## JWT 认证

### 登录

```bash
curl -X POST https://api.example.com/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"email": "user@example.com", "password": "..."}'
```

返回：
```json
{
  "token": "eyJhbGciOiJIUzI1NiIs...",
  "user": { "id": "...", "email": "user@example.com", "role": "admin", ... }
}
```

### 使用 Token

```bash
curl https://api.example.com/api/vps \
  -H "Authorization: Bearer eyJhbGciOiJIUzI1NiIs..."
```

Token 在可配置的时间后过期（默认 24 小时），使用 `POST /api/auth/refresh` 刷新。

## API Key 认证

### 创建 API Key（仅管理员）

通过 API：
```bash
curl -X POST https://api.example.com/api/api-keys \
  -H "Authorization: Bearer <admin-token>" \
  -H "Content-Type: application/json" \
  -d '{"name": "my-agent-key", "role": "agent"}'
```

返回：
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

> **重要���** 完整的 key（`sk_sigma_...`）仅在创建时返回一次。请立即复制并安全保存，之后无法再次查看。

或通过 Web UI：**侧边栏 > API Keys > Create Key**

### 使用 API Key

```bash
curl https://api.example.com/api/vps \
  -H "X-Api-Key: sk_sigma_a1b2c3d4e5f6..."
```

### 管理 API Key

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/api/api-keys` | 列出所有 key（名称、前缀、角色、最后使用时间） |
| `GET` | `/api/api-keys/{id}` | 获取 key 详情 |
| `POST` | `/api/api-keys` | 创建新 key |
| `DELETE` | `/api/api-keys/{id}` | 永久删除（吊销）key |

所有管理端点需要 `admin` 角色。Key 以 SHA-256 哈希存储在数据库中。

### 列出 Key

```bash
curl https://api.example.com/api/api-keys \
  -H "Authorization: Bearer <admin-token>"
```

返回：
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

### 吊销 Key

```bash
curl -X DELETE https://api.example.com/api/api-keys/550e8400-... \
  -H "Authorization: Bearer <admin-token>"
```

Key 被永久删除，使用该 key 的应用会立即收到 `401 Unauthorized`。

## Agent 部署最佳实践

每台 VPS 上的 agent 应使用独立的 `agent` 角色 API key：

```bash
# 为每个 agent 创建专属 key
curl -X POST https://api.example.com/api/api-keys \
  -H "Authorization: Bearer <admin-token>" \
  -H "Content-Type: application/json" \
  -d '{"name": "agent-hk-relay-01", "role": "agent"}'
```

将返回的 key 配置到 agent：
```bash
SIGMA_API_URL=https://api.example.com/api
SIGMA_API_KEY=sk_sigma_...
```

**为什么每个 agent 要独立的 key？**

- **爆炸半径** — 如果某台 VPS 被入侵，只需吊销该 key，不影响其他 agent
- **最小权限** — `agent` 角色只能注册、心跳和管理 Envoy 配置，无法修改 VPS 记录、Provider、用户等���何其他资源
- **审计追踪** — 每个 API key 有独立的 UUID，审计日志能精确显示是哪个 agent 执行了什么操作
- **监控** — 每个 key 的 `last_used_at` 字段可以检测不活跃或异常的 agent

## 旧版 API Key（`API_KEY` 环境变量）

为了向后兼容，API 仍然支持通过 `API_KEY` 环境变量设置的单���静态 key。该 key 始终获得 `admin` 角色。

这是过渡方案。新部署建���使用 `/api/api-keys` 管理的数据库 API key。

## 认证优先级

当请求到达时，认证中间件按以下顺序检查：

1. `Authorization: Bearer <token>` — JWT
2. `X-Api-Key: <key>` — 数据库管���的 key（SHA-256 哈希查找）
3. `X-Api-Key: <key>` — 旧版 `API_KEY` 环境变量兜底
4. 未设置 `API_KEY` 环境变量 — 匿名访问，admin 角色（仅开发环境，不推荐）
5. 以上都不匹配 — `401 Unauthorized`
