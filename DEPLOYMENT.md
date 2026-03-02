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

### PostgreSQL HA (CloudNativePG)

Production uses CloudNativePG operator for a 3-instance PostgreSQL HA cluster (1 primary + 2 replicas) with automatic failover.

**Install the operator:**

```bash
helm repo add cnpg https://cloudnative-pg.github.io/charts
helm install cnpg cnpg/cloudnative-pg -n cnpg-system --create-namespace
```

**Deploy the database cluster:**

```bash
# 1. Update credentials
vi k8s/cnpg-secret.yaml   # Set a strong password

# 2. Apply secret + cluster
kubectl apply -f k8s/cnpg-secret.yaml
kubectl apply -f k8s/cnpg-cluster.yaml

# 3. Wait for cluster to be healthy (~2-3 minutes)
kubectl get cluster sigma-db -n sigma -w

# 4. (Optional) Deploy PgBouncer connection pooler
kubectl apply -f k8s/cnpg-pooler.yaml
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

**Migration from single-pod PostgreSQL:**

```bash
# 1. Scale API to 0
kubectl scale deployment sigma-api -n sigma --replicas=0

# 2. Dump from old database
kubectl exec -n sigma deploy/postgres -- pg_dump -U sigma sigma > backup.sql

# 3. Restore into CloudNativePG cluster
kubectl get pods -n sigma -l cnpg.io/cluster=sigma-db,role=primary -o name | \
  xargs -I{} kubectl exec -n sigma -i {} -- psql -U sigma sigma < backup.sql

# 4. Apply updated secret + api-deployment (DATABASE_URL now points to sigma-db-rw)
kubectl apply -f k8s/secret.yaml
kubectl apply -f k8s/api-deployment.yaml

# 5. Scale API back up
kubectl scale deployment sigma-api -n sigma --replicas=2

# 6. Verify, then delete old postgres resources
kubectl delete -f k8s/postgres.yaml  # (already removed from repo)
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
