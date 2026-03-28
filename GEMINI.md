# Σ Sigma — Project Context

Σ Sigma is a lightweight VPS fleet management platform designed for high-turnover VPN infrastructure. It tracks instances across dozens of cloud providers, manages IP addresses with carrier-specific labels, and integrates with Prometheus/Grafana for monitoring.

## Project Architecture

- **`sigma-api`**: Rust backend (Axum 0.8, SQLx 0.8, PostgreSQL 16, Redis). Handles core logic, provider/VPS management, and agent communication.
- **`sigma-web`**: React frontend (React 19, Vite 7, TypeScript, Tailwind CSS v4). Dashboard for fleet visibility and management.
- **`sigma-agent`**: Rust VPS agent. Performs self-registration, heartbeats, eBPF traffic monitoring, and acts as an Envoy xDS server or static config synchronizer.
- **`sigma-cli`**: Rust-based CLI client for terminal-based interaction with the API.
- **`sigma-probe`**: IP reachability prober, typically deployed on distributed nodes to monitor connectivity.
- **`sigma-agent-ebpf`**: eBPF source code for low-level traffic monitoring.

## Building and Running

The project uses a `Makefile` for common operations:

- **Development**:
  - `make dev`: Starts the full development environment via Docker Compose (Web: `http://localhost`, API: `http://localhost:3000/api`).
  - `make logs`: Tails logs from all services.
  - `make db-shell`: Opens a PostgreSQL shell in the development database.

- **Building Components**:
  - `make build`: Builds Docker images for all major components.
  - `make cli`: Builds the `sigma-cli` binary.
  - `make agent`: Builds the `sigma-agent` binary.
  - `make probe`: Builds the `sigma-probe` binary.

- **Testing**:
  - `make test`: Runs both backend (Rust/Cargo) and frontend (Vitest) tests.
  - `make test-api`: Runs backend tests (requires a test database).
  - `make test-web`: Runs frontend tests using `vitest`.

- **Deployment**:
  - `make deploy-k8s`: Applies Kubernetes manifests from the `k8s/` directory (managed via ArgoCD in production).

## Development Conventions

- **Backend (Rust)**:
  - Uses `Axum` for web routing and `SQLx` for database interactions.
  - Migrations are located in `sigma-api/migrations` and are automatically run on startup.
  - OpenAPI documentation is generated via `utoipa` and available at `/swagger-ui`.
  - Authentication is handled via multi-API-key management (`X-Api-Key` header, per-key roles stored in DB) or JWT for users. Legacy `API_KEY` env var supported as fallback.

- **Frontend (TypeScript/React)**:
  - Uses `Tailwind CSS v4` for styling and `Lucide React` for icons.
  - State management and API fetching via `@tanstack/react-query`.
  - Routing via `react-router-dom`.

- **Infrastructure**:
  - Docker Compose handles local orchestration and dependencies (PostgreSQL, Redis).
  - Prometheus integration via `/api/prometheus/targets` providing `file_sd` compatible JSON.
