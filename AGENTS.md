# forge-workhorse-roan — agent notes

This repository is a Fulmen workhorse template. Consumers follow CDRL: Clone → Degit → Refit → Launch. Keep changes easy to refit.

## Identity

- `.fulmen/app.yaml` is the only hardcoded identity.
- Load binary name, vendor, env prefix, and config name through `internal/appid`.
- After editing `.fulmen/app.yaml`, run `make sync-embedded-identity`.
- `make validate-app-identity` must stay clean: do not put the breed string or env prefix in `cmd/` or `internal/` Rust sources (the embedded YAML mirror is allowed).
- The library crate is named `app` so module paths do not encode the breed.

## Commands

Prefer Make targets:

- `make bootstrap` — sfetch + goneat v0.5.16 + crate fetch
- `make run` / `make build` / `make test` / `make lint` / `make fmt` / `make check-all`
- `make validate-app-identity` / `make doctor`
- `make sync-embedded-identity` / `make verify-embedded-identity`
- `make test-standalone-binary`

`make sync` does not clone Crucible. SSOT access is the rsfulmen shim only. Do not add `ssot-consumer.yaml` or `make sync-ssot`.

## Layout

```
cmd/<binary>/main.rs
internal/{appid,cmd,config,core,observability,server}
config/<config_name>/v1.0.0/
schemas/<config_name>/v1.0.0/
```

`internal/core` is a placeholder (echo). Do not add product features to the template.

## Helper modules

Use rsfulmen 0.1.5 for Crucible access, three-layer merge helpers, config-path, schema validation, telemetry, logging, error envelopes, signals, and docscribe. Do not hand-roll those catalogs.

## Git

- `.plans/` is gitignored. Never stage it.
- Do not commit secrets or `.env`.
- Run `make test` and `make lint` before commits.

Commit message shape (public trailers only):

```
<type>: <subject>

<body>

Changes:
- <change>

Co-Authored-By: <Model> <noreply@3leaps.net>
```

## References

- [Fulmen Forge Workhorse Standard](https://github.com/fulmenhq/crucible/blob/main/docs/architecture/fulmen-forge-workhorse-standard.md)
- [rsfulmen](https://github.com/fulmenhq/rsfulmen)
- [docs/development/fulmen_cdrl_guide.md](docs/development/fulmen_cdrl_guide.md)
- [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md)
