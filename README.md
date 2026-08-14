# forge-workhorse-roan

A Fulmen workhorse application template for robust, scalable Rust backends and CLI tools.

The binary and package name is `roan`. Environment variables use the `ROAN_*` prefix. Breed is locked **roan**. License: MIT plus the 3 Leaps copyright/trademark addendum (see `LICENSE`). Not MIT OR Apache-2.0.

## Overview

This repository is a production-ready starter. It provides:

- HTTP server with `/health`, `/version`, and `/metrics`
- CLI commands `serve`, `version` / `--extended`, `health`, `envinfo`, and `doctor`
- Structured logging and metrics through [rsfulmen](https://github.com/fulmenhq/rsfulmen)
- Three-layer configuration (embedded defaults → user file → env / flags)
- App Identity from `.fulmen/app.yaml` (embedded for standalone binaries)
- CDRL workflow: Clone → Degit → Refit → Launch

There is no domain logic. The HTTP surface is a health/echo placeholder.

## Fulmen layers

```
Level 3: Your application          ← after CDRL refit
Level 2: forge-workhorse-roan      ← this template
Level 1: rsfulmen + goneat         ← helper + DX
Level 0: Crucible                  ← SSOT, accessed only via rsfulmen
```

Workhorses pin the language helper (`rsfulmen`), not Crucible git. There is no `ssot-consumer.yaml` and no `make sync-ssot`.

## Quick start

### Prerequisites

- rustc **1.88** (see `rust-toolchain.toml`)
- [sfetch](https://github.com/3leaps/sfetch) for DX bootstrap (goneat)

### Bootstrap and run

```bash
git clone https://github.com/fulmenhq/forge-workhorse-roan.git my-app
cd my-app
make bootstrap
make run
```

`make bootstrap` installs goneat **v0.5.16** via sfetch and fetches crate dependencies. `make run` starts the server (default `http://0.0.0.0:8080`).

```bash
curl http://127.0.0.1:8080/health
curl http://127.0.0.1:8080/version
curl http://127.0.0.1:8080/metrics
```

## CLI

```bash
make build
./bin/roan --help
./bin/roan serve --port 8080 --log-level info
./bin/roan version --extended
./bin/roan health
./bin/roan envinfo
./bin/roan doctor
```

Every subcommand accepts `--help`. Examples are listed on each help page.

## Configuration

Identity lives in `.fulmen/app.yaml`. Everything else (env prefix, config paths, telemetry namespace, CLI name) is derived from it.

Standard environment variables (prefix from App Identity):

| Variable | Purpose | Default |
| --- | --- | --- |
| `{PREFIX}PORT` / `{PREFIX}HOST` | Listen address | `8080` / `0.0.0.0` |
| `{PREFIX}LOG_LEVEL` | `trace\|debug\|info\|warn\|error` | `info` |
| `{PREFIX}CONFIG_PATH` | Layer 2 file override | Config Path API |
| `{PREFIX}METRICS_PORT` | Metrics port | same as `PORT` |
| `{PREFIX}HEALTH_PORT` | Accepted for flag parity | same as `PORT` |

Copy `.env.example` to `.env` and adjust values. Layered load order: CLI flags → env vars → user config file → embedded defaults.

User config path (rsfulmen Config Path API): `~/.config/{config_name}/config.yaml`.

Repository defaults: `config/{config_name}/v1.0.0/{config_name}-defaults.yaml`. The same document is embedded at `internal/assets/config/defaults.yaml` so a copied binary still starts outside the repo.

## Directory layout

```
.fulmen/app.yaml                 # only hardcoded identity
cmd/<binary>/main.rs             # binary entry
internal/
  appid/                         # workhorse identity loader
  assets/appidentity/app.yaml    # embedded identity mirror
  cmd/                           # clap commands
  config/                        # three-layer loader
  core/                          # domain placeholder (echo)
  observability/                 # rsfulmen logging + metrics
  server/                        # axum health/version/metrics/echo
config/<config_name>/v1.0.0/     # versioned defaults
schemas/<config_name>/v1.0.0/    # versioned config schema
docs/development/fulmen_cdrl_guide.md
```

## Make targets

| Target | Purpose |
| --- | --- |
| `make bootstrap` | sfetch + goneat + `cargo fetch` |
| `make run` | `serve --verbose` |
| `make build` | debug binary in `bin/` |
| `make test` | `cargo test` |
| `make lint` | rustfmt check + clippy |
| `make fmt` | rustfmt (and goneat format when installed) |
| `make check-all` | fmt, identity verify, lint, test |
| `make validate-app-identity` | fail on hardcoded breed strings in `cmd/` / `internal/` |
| `make doctor` | CDRL completeness checks |
| `make sync-embedded-identity` | copy `.fulmen/app.yaml` → embedded mirror |
| `make verify-embedded-identity` | assert the mirror matches |
| `make test-standalone-binary` | run `version` / `--help` from `/tmp` |
| `make sync` | no-op helper shim (does **not** clone Crucible) |

## Pins

- **rsfulmen** `0.1.5` — identity discovery overrides, Crucible shim, three-layer merge helpers, config-path, schema validation, telemetry, logging, errors, signals, docscribe
- **goneat** `v0.5.16` via sfetch (DX only; not a crate dependency)
- **rustc** `1.88`, edition `2021`

`3leaps/sysprims` v0.1.18 and `3leaps/ipcprims` v0.2.3 are optional primitives. They are not crate dependencies and are not required for `make bootstrap && make run`. Add them in your refit if you need those APIs.

## CDRL

See [docs/development/fulmen_cdrl_guide.md](docs/development/fulmen_cdrl_guide.md).

1. Clone this repository
2. Remove `.git` and start a new history
3. Edit `.fulmen/app.yaml`, then rename the Cargo package, `cmd/` directory, `config/`, and `schemas/`
4. `make validate-app-identity && make doctor && make test && make build`

## HTTP

| Path | Purpose |
| --- | --- |
| `GET /health` | `{ "status": "healthy", "version": "..." }` |
| `GET /version` | App, rsfulmen, and Crucible versions |
| `GET /metrics` | Prometheus text |
| `GET\|POST /echo` | Placeholder echo |
| `GET /docs` | Docscribe sample (workhorse standard frontmatter) |
| `POST /admin/signal` | Signal token (`TERM`, `INT`, `HUP`, …) |

## Development

See [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) and [AGENTS.md](AGENTS.md).

```bash
make validate-app-identity
make test
make lint
make check-all
make build
make test-standalone-binary
```

## License

MIT plus the 3 Leaps copyright/trademark addendum. See `LICENSE`.
