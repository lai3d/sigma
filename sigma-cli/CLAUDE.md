# Sigma CLI — Project Context

## What is this?

Standalone Rust CLI binary that calls the Sigma REST API (`sigma-api`). Built with clap (derive) + reqwest + comfy-table. Binary name: `sigma`.

## Structure

```
sigma-cli/src/
├── main.rs          # Clap CLI definition (Cli → Commands → subcommands), dispatch
├── config.rs        # Config loading: ~/.config/sigma/config.toml → env vars → CLI flags
├── client.rs        # SigmaClient: reqwest wrapper with X-Api-Key header, error handling
├── models.rs        # API request/response types (mirrors sigma-api/src/models.rs)
├── output.rs        # print_table (comfy-table), print_json, print_pagination
└── commands/
    ├── mod.rs
    ├── providers.rs  # list, get, create, update, delete, export, import
    ├── vps.rs        # list, get, create, update, delete, retire, export, import
    └── stats.rs      # Dashboard stats display
```

## Key Design Decisions

- **Mirrors API types but doesn't share code** — CLI models use `skip_serializing_if = "Option::is_none"` for partial updates; API models use `#[serde(default)]` for deserialization. Different enough to warrant separate types.
- **IP address format** — CLI accepts `--ip 1.2.3.4:label` (colon-separated), parsed into `IpEntry { ip, label }` in `commands/vps.rs`
- **Config layering** — file < env < CLI flags, same pattern as many CLI tools. Config file path: `~/.config/sigma/config.toml`
- **`--json` global flag** — every command checks this to switch between table and JSON output
- **`get_text` vs `get<T>`** — export endpoints return raw CSV/JSON text, not structured data, so `client.get_text()` is used instead of `client.get::<T>()`
- **`post_empty`** — retire endpoint takes no body, separate from `post` which serializes JSON

## Command → API mapping

| Command | Method | API Path |
|---------|--------|----------|
| `providers list` | GET | `/providers?page=&per_page=` |
| `providers get <ID>` | GET | `/providers/{id}` |
| `providers create` | POST | `/providers` |
| `providers update <ID>` | PUT | `/providers/{id}` |
| `providers delete <ID>` | DELETE | `/providers/{id}` |
| `providers export` | GET | `/providers/export?format=` |
| `providers import <FILE>` | POST | `/providers/import` |
| `vps list` | GET | `/vps?status=&country=&...` |
| `vps get <ID>` | GET | `/vps/{id}` |
| `vps create` | POST | `/vps` |
| `vps update <ID>` | PUT | `/vps/{id}` |
| `vps delete <ID>` | DELETE | `/vps/{id}` |
| `vps retire <ID>` | POST | `/vps/{id}/retire` |
| `vps export` | GET | `/vps/export?format=` |
| `vps import <FILE>` | POST | `/vps/import` |
| `stats` | GET | `/stats` |

## Build & Run

```bash
# Build
cd sigma-cli && cargo build

# Install
make cli-install   # from project root

# Use
sigma config set-url http://localhost:3000/api
sigma providers list
sigma vps list --status active --json
```

## Dependencies

clap 4 (derive), tokio 1, reqwest 0.12 (json), serde/serde_json, uuid, chrono, comfy-table 7, toml 0.8, dirs 6, anyhow
