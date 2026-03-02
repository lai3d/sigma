# Sigma Deployment Guide

## Overview

- **Local Development**: Docker Compose
- **Production**: Kubernetes via ArgoCD (GitOps, pull-based)

---

## Docker Compose (Local/Dev)

### Prerequisites

- Docker 24+
- Docker Compose v2

### Setup

```bash
# 1. Configure environment
cp .env.example .env
# Edit .env and set your API_KEY

# 2. Start all services (from project root)
docker compose up -d

# 3. Check status
docker compose ps

# 4. View logs
docker compose logs -f api
docker compose logs -f web
```

### Using Pre-built Images

If you don't want to build from source, use `docker-compose.prebuilt.yml` which pulls pre-built images from GHCR:

```bash
docker compose -f docker-compose.prebuilt.yml up -d
```

### Services

| Service | Port | URL |
|---------|------|-----|
| PostgreSQL | 5432 | localhost:5432 |
| API | 3000 | http://localhost:3000/api |
| Web | 80 | http://localhost |

### Database

```bash
# Connect to PostgreSQL
docker compose exec db psql -U sigma

# Backup
docker compose exec db pg_dump -U sigma sigma > backup.sql

# Restore
docker compose exec -T db psql -U sigma sigma < backup.sql
```

### Rebuilding

```bash
# Rebuild after code changes
docker compose up -d --build

# Rebuild single service
docker compose up -d --build api
docker compose up -d --build web
```

---

## Kubernetes (Production)

### CI: GitHub Actions

Push to `main` triggers per-package image builds (only when that package changes) and pushes to GHCR:

- `ghcr.io/lai3d/sigma/api:latest` + `ghcr.io/lai3d/sigma/api:sha-<commit>`
- `ghcr.io/lai3d/sigma/web:latest` + `ghcr.io/lai3d/sigma/web:sha-<commit>`
- `ghcr.io/lai3d/sigma/agent:latest` + `ghcr.io/lai3d/sigma/agent:sha-<commit>`
- `ghcr.io/lai3d/sigma/probe:latest` + `ghcr.io/lai3d/sigma/probe:sha-<commit>`

PR builds are validated but not pushed.

### CD: ArgoCD

ArgoCD watches the `k8s/` directory and auto-syncs to the cluster.

```bash
# 1. Install ArgoCD Application
cat <<EOF | kubectl apply -f -
apiVersion: argoproj.io/v1alpha1
kind: Application
metadata:
  name: sigma
  namespace: argocd
spec:
  project: default
  source:
    repoURL: https://github.com/lai3d/sigma.git
    targetRevision: main
    path: k8s
  destination:
    server: https://kubernetes.default.svc
    namespace: sigma
  syncPolicy:
    automated:
      prune: true
      selfHeal: true
    syncOptions:
      - CreateNamespace=true
EOF

# 2. (Optional) ArgoCD Image Updater for auto-updating image tags
# See: https://argocd-image-updater.readthedocs.io/
```

### PostgreSQL Options

Two PostgreSQL deployment modes are available. Choose one:

#### Option A: Standalone PostgreSQL (default)

Single-pod Deployment via `k8s/postgres.yaml`. Simple, no extra operators needed. Good for dev/staging or small-scale production.

```bash
# Included in default k8s/ manifests, no extra steps needed.
# DATABASE_URL in secret.yaml already points to postgres-service.
kubectl apply -f k8s/postgres.yaml
```

#### Option B: CloudNativePG HA Cluster

3-instance HA cluster (1 primary + 2 replicas) via CloudNativePG operator. Automatic failover, read replicas, PgBouncer pooling. Recommended for production.

**Files:** `k8s/cnpg-secret.yaml`, `k8s/cnpg-cluster.yaml`, `k8s/cnpg-pooler.yaml`

**Step 1 — Install the operator:**

```bash
helm repo add cnpg https://cloudnative-pg.github.io/charts
helm install cnpg cnpg/cloudnative-pg -n cnpg-system --create-namespace
```

**Step 2 — Deploy the cluster:**

```bash
vi k8s/cnpg-secret.yaml   # Set a strong password
kubectl apply -f k8s/cnpg-secret.yaml
kubectl apply -f k8s/cnpg-cluster.yaml

# Wait for cluster to be healthy (~2-3 minutes)
kubectl get cluster sigma-db -n sigma -w

# (Optional) Deploy PgBouncer connection pooler
kubectl apply -f k8s/cnpg-pooler.yaml
```

**Step 3 — Point API to the HA cluster:**

Update `DATABASE_URL` in `k8s/secret.yaml`:

```diff
- DATABASE_URL: "postgres://sigma:CHANGE_ME@postgres-service:5432/sigma"
+ DATABASE_URL: "postgres://sigma:CHANGE_ME@sigma-db-rw:5432/sigma"
```

Optionally add an initContainer to `k8s/api-deployment.yaml` to wait for DB readiness:

```yaml
initContainers:
- name: wait-for-db
  image: busybox:1.36
  command: ['sh', '-c', 'until nc -z sigma-db-rw 5432; do sleep 2; done']
```

**Step 4 — Remove standalone PostgreSQL (if previously deployed):**

```bash
kubectl delete -f k8s/postgres.yaml
```

**Services created automatically by CloudNativePG:**

| Service | Target | Usage |
|---------|--------|-------|
| `sigma-db-rw` | Primary (read-write) | API connects here |
| `sigma-db-ro` | Replicas (read-only) | Future read replicas |
| `sigma-db-r` | All instances | Monitoring |

**Verify:**

```bash
kubectl get cluster sigma-db -n sigma              # Status: healthy
kubectl get pods -n sigma -l cnpg.io/cluster=sigma-db  # 3 pods running
kubectl get svc -n sigma | grep sigma-db            # rw/ro/r services
```

**Migration from standalone to HA:**

```bash
# 1. Scale API to 0
kubectl scale deployment sigma-api -n sigma --replicas=0

# 2. Dump from standalone database
kubectl exec -n sigma deploy/postgres -- pg_dump -U sigma sigma > backup.sql

# 3. Restore into CloudNativePG cluster
kubectl get pods -n sigma -l cnpg.io/cluster=sigma-db,role=primary -o name | \
  xargs -I{} kubectl exec -n sigma -i {} -- psql -U sigma sigma < backup.sql

# 4. Update DATABASE_URL in secret.yaml to sigma-db-rw, then apply
kubectl apply -f k8s/secret.yaml
kubectl apply -f k8s/api-deployment.yaml

# 5. Scale API back up
kubectl scale deployment sigma-api -n sigma --replicas=2

# 6. Remove standalone postgres
kubectl delete -f k8s/postgres.yaml
```

### Pre-deploy Checklist

```bash
# Update secrets before first deploy
vi k8s/secret.yaml     # Set DATABASE_URL, API_KEY
vi k8s/ingress.yaml    # Set your domain
```

### Verification

```bash
kubectl get all -n sigma
kubectl logs -f deployment/sigma-api -n sigma
```

---

## Building Images

### API (Rust)

```bash
docker build -t sigma-api:latest ./sigma-api

# Multi-arch
docker buildx build --platform linux/amd64,linux/arm64 \
  -t ghcr.io/lai3d/sigma/api:latest --push ./sigma-api
```

### Web (React)

```bash
docker build -t sigma-web:latest ./sigma-web
```

---

## Security Checklist

- [ ] Change default `API_KEY` in secrets
- [ ] Use strong PostgreSQL password
- [ ] Enable TLS/HTTPS via Ingress
- [ ] Use managed database (RDS/Cloud SQL) instead of in-cluster PG
- [ ] Set up backup strategy
- [ ] Scan images for vulnerabilities
- [ ] Don't commit `.env` file

---

## Monitoring

### Prometheus Integration

Sigma exposes `GET /api/prometheus/targets` for file_sd.

### Health Checks

- API: `GET /api/stats`
- Web: `GET /` (nginx)

---

## Troubleshooting

**API can't connect to database:**
```bash
docker compose logs db
docker compose exec api ping db
```

**Port already in use:**
```bash
# Change ports in docker-compose.yml, e.g. "8080:80"
```

**K8s ImagePullBackOff:**
```bash
kubectl describe pod <pod-name> -n sigma
```

**K8s CrashLoopBackOff:**
```bash
kubectl logs <pod-name> -n sigma --previous
```
