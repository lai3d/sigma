.PHONY: help dev build up down logs clean deploy-k8s

# Default registry (override with: make build REGISTRY=your-registry.com)
REGISTRY ?= your-registry
TAG ?= latest

help: ## Show this help message
	@echo "Sigma - VPS Fleet Management"
	@echo ""
	@echo "Usage: make [target]"
	@echo ""
	@echo "Targets:"
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-20s\033[0m %s\n", $$1, $$2}'

dev: ## Start development environment with Docker Compose
	docker compose up -d
	@echo "✅ Services started:"
	@echo "   Web: http://localhost"
	@echo "   API: http://localhost:3000/api"
	@echo "   DB:  localhost:5432"

build: ## Build Docker images
	@echo "🔨 Building API..."
	cd sigma-api && docker build -t $(REGISTRY)/sigma-api:$(TAG) .
	@echo "🔨 Building Web..."
	cd sigma-web && docker build -t $(REGISTRY)/sigma-web:$(TAG) .
	@echo "✅ Build complete"

push: build ## Build and push images to registry
	@echo "⬆️  Pushing images..."
	docker push $(REGISTRY)/sigma-api:$(TAG)
	docker push $(REGISTRY)/sigma-web:$(TAG)
	@echo "✅ Push complete"

up: ## Start all services
	docker compose up -d

down: ## Stop all services
	docker compose down

logs: ## Tail logs from all services
	docker compose logs -f

logs-api: ## Tail API logs
	docker compose logs -f api

logs-web: ## Tail web logs
	docker compose logs -f web

logs-db: ## Tail database logs
	docker compose logs -f db

restart: ## Restart all services
	docker compose restart

restart-api: ## Restart API service
	docker compose restart api

restart-web: ## Restart web service
	docker compose restart web

ps: ## Show running containers
	docker compose ps

clean: ## Stop services and remove volumes (⚠️  DELETES DATA)
	docker compose down -v

db-backup: ## Backup PostgreSQL database
	@echo "📦 Creating backup..."
	docker compose exec -T db pg_dump -U sigma sigma > backup_$(shell date +%Y%m%d_%H%M%S).sql
	@echo "✅ Backup created"

db-restore: ## Restore PostgreSQL database (usage: make db-restore FILE=backup.sql)
	@if [ -z "$(FILE)" ]; then echo "❌ Error: FILE not specified. Usage: make db-restore FILE=backup.sql"; exit 1; fi
	@echo "⚠️  Restoring database from $(FILE)..."
	docker compose exec -T db psql -U sigma sigma < $(FILE)
	@echo "✅ Restore complete"

db-shell: ## Open PostgreSQL shell
	docker compose exec db psql -U sigma

deploy-k8s: ## Apply Kubernetes manifests
	kubectl apply -f k8s/

k8s-status: ## Check Kubernetes deployment status
	kubectl get all -n sigma

k8s-logs-api: ## Tail API logs in Kubernetes
	kubectl logs -f deployment/sigma-api -n sigma

k8s-logs-web: ## Tail web logs in Kubernetes
	kubectl logs -f deployment/sigma-web -n sigma

k8s-delete: ## Delete Kubernetes deployment
	kubectl delete namespace sigma

test-api: ## Test API health
	@echo "🧪 Testing API..."
	@curl -s -H "X-Api-Key: ${API_KEY:-change-me}" http://localhost:3000/api/stats | jq . || echo "❌ API not responding"

test-web: ## Test web frontend
	@echo "🧪 Testing Web..."
	@curl -s http://localhost/ > /dev/null && echo "✅ Web is up" || echo "❌ Web not responding"
