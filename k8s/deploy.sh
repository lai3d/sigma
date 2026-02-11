#!/bin/bash
set -e

echo "🚀 Deploying Sigma to Kubernetes..."

# Apply namespace first
kubectl apply -f namespace.yaml

# Apply configs and secrets
kubectl apply -f configmap.yaml
kubectl apply -f secret.yaml

# Deploy PostgreSQL
echo "📦 Deploying PostgreSQL..."
kubectl apply -f postgres.yaml

# Wait for PostgreSQL to be ready
echo "⏳ Waiting for PostgreSQL..."
kubectl wait --for=condition=ready pod -l app=postgres -n sigma --timeout=120s

# Deploy API
echo "🔧 Deploying Sigma API..."
kubectl apply -f api-deployment.yaml

# Wait for API to be ready
echo "⏳ Waiting for API..."
kubectl wait --for=condition=ready pod -l app=sigma-api -n sigma --timeout=120s

# Deploy Web
echo "🌐 Deploying Sigma Web..."
kubectl apply -f web-deployment.yaml

# Deploy Ingress
echo "🌍 Deploying Ingress..."
kubectl apply -f ingress.yaml

echo "✅ Deployment complete!"
echo ""
echo "Check status:"
echo "  kubectl get pods -n sigma"
echo "  kubectl get svc -n sigma"
echo "  kubectl get ingress -n sigma"
