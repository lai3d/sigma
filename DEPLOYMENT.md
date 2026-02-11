# Sigma Deployment Guide

## Overview

- **Local Development**: Docker Compose
- **Testing/Staging**: Docker Compose
- **Production**: Kubernetes

---

## 🐳 Docker Compose (Local/Testing)

### Prerequisites

- Docker 24+
- Docker Compose v2

### Setup

```bash
cd sigma

# 1. Configure environment
cp .env.example .env
# Edit .env and set your API_KEY

# 2. Start all services
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

### Database Access

```bash
# Connect to PostgreSQL
docker compose exec db psql -U sigma

# Run migrations (automatic on API startup)
docker compose restart api

# Backup database
docker compose exec db pg_dump -U sigma sigma > backup.sql

# Restore database
docker compose exec -T db psql -U sigma sigma < backup.sql
```

### Rebuilding

```bash
# Rebuild after code changes
docker compose build api
docker compose up -d api

# Rebuild frontend
docker compose build web
docker compose up -d web

# Rebuild all
docker compose up -d --build
```

### Cleanup

```bash
# Stop services
docker compose down

# Remove volumes (⚠️ DELETES DATA)
docker compose down -v
```

---

## ☸️ Kubernetes (Production)

See [k8s/README.md](./k8s/README.md) for detailed instructions.

### Quick Deploy

```bash
cd k8s

# 1. Update secrets
vi secret.yaml  # Change DATABASE_URL and API_KEY

# 2. Update ingress domain
vi ingress.yaml  # Change sigma.yourdomain.com

# 3. Build and push images
docker build -t your-registry/sigma-api:v1.0.0 ../sigma
docker push your-registry/sigma-api:v1.0.0

docker build -t your-registry/sigma-web:v1.0.0 ../sigma-web
docker push your-registry/sigma-web:v1.0.0

# 4. Update deployment images
vi api-deployment.yaml  # Update image: line
vi web-deployment.yaml  # Update image: line

# 5. Deploy
./deploy.sh
```

### Verification

```bash
kubectl get all -n sigma
kubectl logs -f deployment/sigma-api -n sigma
```

---

## 📦 Building Images

### API (Rust)

```bash
cd sigma
docker build -t sigma-api:latest .

# Multi-arch (for ARM/AMD)
docker buildx build --platform linux/amd64,linux/arm64 \
  -t your-registry/sigma-api:latest --push .
```

### Web (React)

```bash
cd sigma-web

# Install dependencies first (for local development)
npm install

# Build Docker image
docker build -t sigma-web:latest .
```

---

## 🔒 Security Checklist

### Production

- [ ] Change default `API_KEY` in secrets
- [ ] Use strong PostgreSQL password
- [ ] Enable TLS/HTTPS via Ingress
- [ ] Use managed database (RDS/Cloud SQL) instead of in-cluster PostgreSQL
- [ ] Set up backup strategy
- [ ] Configure resource limits
- [ ] Enable network policies
- [ ] Use private container registry
- [ ] Scan images for vulnerabilities
- [ ] Set up monitoring and alerting
- [ ] Configure log aggregation

### Docker Compose (Development)

- [ ] Don't commit `.env` file
- [ ] Use different `API_KEY` than production
- [ ] Don't expose PostgreSQL port (5432) if not needed
- [ ] Keep Docker and images updated

---

## 📊 Monitoring

### Prometheus Integration

Sigma exposes `/api/prometheus/targets` for Prometheus file_sd.

If using Prometheus Operator in K8s, add a ServiceMonitor:

```yaml
apiVersion: monitoring.coreos.com/v1
kind: ServiceMonitor
metadata:
  name: sigma-api
  namespace: sigma
spec:
  selector:
    matchLabels:
      app: sigma-api
  endpoints:
  - port: http
    interval: 30s
```

### Health Checks

- API: `GET /api/stats` (requires `X-Api-Key` header)
- Web: `GET /` (nginx serves index.html)

---

## 🔄 CI/CD Example

### GitHub Actions

```yaml
name: Deploy Sigma

on:
  push:
    branches: [main]

jobs:
  build-and-deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Build API
        run: |
          cd sigma
          docker build -t ${{ secrets.REGISTRY }}/sigma-api:${{ github.sha }} .
          docker push ${{ secrets.REGISTRY }}/sigma-api:${{ github.sha }}

      - name: Build Web
        run: |
          cd sigma-web
          docker build -t ${{ secrets.REGISTRY }}/sigma-web:${{ github.sha }} .
          docker push ${{ secrets.REGISTRY }}/sigma-web:${{ github.sha }}

      - name: Deploy to K8s
        run: |
          kubectl set image deployment/sigma-api sigma-api=${{ secrets.REGISTRY }}/sigma-api:${{ github.sha }} -n sigma
          kubectl set image deployment/sigma-web sigma-web=${{ secrets.REGISTRY }}/sigma-web:${{ github.sha }} -n sigma
```

---

## 🆘 Troubleshooting

### Docker Compose

**API can't connect to database:**
```bash
# Check if DB is ready
docker compose logs db

# Check network
docker compose exec api ping db
```

**Port already in use:**
```bash
# Change ports in docker-compose.yml
# Example: "8080:80" instead of "80:80"
```

### Kubernetes

**ImagePullBackOff:**
```bash
# Check image name and registry credentials
kubectl describe pod <pod-name> -n sigma
```

**CrashLoopBackOff:**
```bash
# Check logs
kubectl logs <pod-name> -n sigma --previous
```

**Ingress not working:**
```bash
# Verify ingress controller is installed
kubectl get pods -n ingress-nginx

# Check ingress events
kubectl describe ingress sigma-ingress -n sigma
```
