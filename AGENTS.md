# Repository Guidelines

## Project Structure & Module Organization
This repository is split by runtime:

- `sigma-api/`: Rust backend built with Axum and SQLx; routes live in `src/routes/`, migrations in `migrations/`, integration tests in `tests/`.
- `sigma-web/`: React 19 + Vite + TypeScript frontend; API clients are under `src/api/`, shared hooks in `src/hooks/`, UI in `src/components/`, page-level screens in `src/pages/`, and tests in `src/test/`.
- `sigma-cli/`, `sigma-probe/`, `sigma-agent/`: Rust binaries for CLI, reachability probing, and host agents.
- `k8s/`, `grafana/`, and `docs/`: deployment manifests, dashboards, and project documentation.

## Build, Test, and Development Commands
- `make dev`: start the local stack with Docker Compose.
- `make logs`, `make logs-api`, `make logs-web`: inspect service logs.
- `make test`: run backend and frontend tests.
- `make test-api`: start the test database and run `sigma-api` tests with the expected env vars.
- `make test-web`: run Vitest for `sigma-web`.
- `cd sigma-web && npm run build`: type-check and build the frontend bundle.
- `cd sigma-web && npm run lint`: run ESLint on TypeScript and React files.
- `cargo build --manifest-path sigma-api/Cargo.toml`: compile an individual Rust package.

## Coding Style & Naming Conventions
Rust packages use Edition 2024 defaults: 4-space indentation, `snake_case` modules/functions, and small route modules grouped by domain, for example `src/routes/providers.rs`. Frontend code follows the existing Vite ESLint config: TypeScript-first, PascalCase React components like `StatusBadge.tsx`, camelCase hooks like `useProviders.ts`, and lowercase API modules like `api/providers.ts`. Keep files focused and colocate tests near the feature area when possible.

## Testing Guidelines
Backend integration tests live in `sigma-api/tests/` and follow the `*_test.rs` pattern. Frontend tests use Vitest and Testing Library in `sigma-web/src/test/` with `*.test.ts` naming. Add or update tests for behavior changes, especially around API routes, auth, and data formatting helpers.

## Commit & Pull Request Guidelines
Recent history uses short imperative commit subjects, for example `Fix agent heartbeat lookup for merged VPS hostnames`. Keep commits narrowly scoped. PRs should explain the user-visible change, note schema or config updates, link the related issue, and include screenshots for `sigma-web` UI changes. Mention the commands you ran, such as `make test` or `npm run lint`.

## Security & Configuration Tips
Copy `.env.example` to `.env` for local work and avoid committing secrets. Treat `make clean` and database restore commands as destructive. When changing API auth, migrations, or Docker/Kubernetes manifests, document the rollout impact in the PR.
