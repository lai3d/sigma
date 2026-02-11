# Sigma Kubernetes Deployment

## Prerequisites

- Kubernetes cluster (1.25+)
- `kubectl` configured
- Nginx Ingress Controller installed
- (Optional) cert-manager for TLS

## Quick Start

### 1. Update Secrets

Edit `secret.yaml` and replace:
- `DATABASE_URL` - PostgreSQL connection string
- `API_KEY` - Your API authentication key

### 2. Update Ingress

Edit `ingress.yaml` and replace:
- `sigma.yourdomain.com` - Your actual domain

### 3. Build and Push Images

```bash
# Build API
cd ../sigma-api
docker build -t your-registry/sigma-api:latest .
docker push your-registry/sigma-api:latest

# Build Web
cd ../sigma-web
docker build -t your-registry/sigma-web:latest .
docker push your-registry/sigma-web:latest
```

Update image names in:
- `api-deployment.yaml`
- `web-deployment.yaml`

### 4. Deploy

```bash
chmod +x deploy.sh
./deploy.sh
```

Or manually:

```bash
kubectl apply -f namespace.yaml
kubectl apply -f configmap.yaml
kubectl apply -f secret.yaml
kubectl apply -f postgres.yaml
kubectl apply -f api-deployment.yaml
kubectl apply -f web-deployment.yaml
kubectl apply -f ingress.yaml
```

## Verification

```bash
# Check pods
kubectl get pods -n sigma

# Check services
kubectl get svc -n sigma

# Check ingress
kubectl get ingress -n sigma

# View logs
kubectl logs -f deployment/sigma-api -n sigma
kubectl logs -f deployment/sigma-web -n sigma
```

## Production Recommendations

### 1. Use External Database

Replace `postgres.yaml` with a managed PostgreSQL service:
- AWS RDS
- Google Cloud SQL
- Azure Database for PostgreSQL
- DigitalOcean Managed Database

Update `DATABASE_URL` in `secret.yaml`.

### 2. Enable TLS

Uncomment TLS section in `ingress.yaml` and install cert-manager:

```bash
kubectl apply -f https://github.com/cert-manager/cert-manager/releases/download/v1.13.0/cert-manager.yaml
```

### 3. Resource Limits

Adjust resource requests/limits based on actual usage:
- Monitor with `kubectl top pods -n sigma`
- Use Prometheus + Grafana for detailed metrics

### 4. Horizontal Pod Autoscaling

```yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: sigma-api-hpa
  namespace: sigma
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: sigma-api
  minReplicas: 2
  maxReplicas: 10
  metrics:
  - type: Resource
    resource:
      name: cpu
      target:
        type: Utilization
        averageUtilization: 70
```

### 5. Backup Strategy

For PostgreSQL PVC:

```bash
# Create backup
kubectl exec -n sigma deployment/postgres -- pg_dump -U sigma sigma > backup.sql

# Restore
kubectl exec -i -n sigma deployment/postgres -- psql -U sigma sigma < backup.sql
```

Or use Velero for full cluster backups.

## Monitoring

### Prometheus Integration

Sigma exposes `/api/prometheus/targets` which is already integrated with your monitoring stack.

Add ServiceMonitor (if using Prometheus Operator):

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
    path: /api/stats
    interval: 30s
```

## Troubleshooting

### Pods not starting

```bash
kubectl describe pod <pod-name> -n sigma
kubectl logs <pod-name> -n sigma
```

### Database connection issues

```bash
# Test connectivity
kubectl exec -it deployment/sigma-api -n sigma -- /bin/sh
# Inside pod: try connecting to postgres-service:5432
```

### Ingress not working

```bash
kubectl describe ingress sigma-ingress -n sigma
kubectl logs -n ingress-nginx deployment/ingress-nginx-controller
```

## Cleanup

```bash
kubectl delete namespace sigma
```
