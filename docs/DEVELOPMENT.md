# Development

## Toolchain

- rustc 1.88 (`rust-toolchain.toml`)
- edition 2021
- rsfulmen 0.1.5
- goneat v0.5.16 via sfetch (optional for `cargo test` / `cargo build`)

```bash
rustup show
make tools
```

## Common workflow

```bash
make bootstrap          # first clone
make fmt
make validate-app-identity
make test
make lint
make build
make test-standalone-binary
```

`make check-all` runs fmt, embedded-identity verify, validate-app-identity, lint, and test.

## Tests

Unit and integration tests live next to the modules they cover (`internal/**`). HTTP routes are exercised with axum `oneshot` (no network). Skip helpers are not required for the default suite.

```bash
cargo test --workspace --all-targets
```

Network-dependent checks should be gated behind an explicit feature or ignored by default.

## Identity

1. Edit `.fulmen/app.yaml`
2. `make sync-embedded-identity`
3. `make verify-embedded-identity`
4. `make validate-app-identity`

The binary embeds `internal/assets/appidentity/app.yaml` so `version` and `--help` work when the process cwd is not the repository.

## Logging and metrics

rsfulmen logging profiles: `SIMPLE` (stderr text) and `STRUCTURED` (JSON lines). Metrics on `/metrics` are Prometheus text. Application series use the telemetry namespace from App Identity.

## Signals

`serve` shuts down on Ctrl+C / SIGINT. `POST /admin/signal` accepts Foundry tokens (`TERM`, `INT`, `HUP`, …). Double-tap timing comes from the rsfulmen signal catalog.

## Optional crates

`3leaps/sysprims` v0.1.18 and `3leaps/ipcprims` v0.2.3 are not wired as dependencies. Add them in a refit if you need those APIs. They are not part of `make run`.

## TypeScript note

The TypeScript workhorse is a separate stack. Do not copy its runtime into this repository.
