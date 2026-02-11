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

Push to `main` triggers image builds and pushes to GHCR:

- `ghcr.io/lai3d/sigma/api:latest` + `ghcr.io/lai3d/sigma/api:sha-<commit>`
- `ghcr.io/lai3d/sigma/web:latest` + `ghcr.io/lai3d/sigma/web:sha-<commit>`

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
