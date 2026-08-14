# CDRL guide: refitting forge-workhorse-roan

Customize this template with Clone → Degit → Refit → Launch.

## 1. Clone

```bash
git clone https://github.com/fulmenhq/forge-workhorse-roan.git my-app
cd my-app
```

## 2. Degit

```bash
rm -rf .git
git init
```

## 3. Refit

### 3.1 App Identity

Edit `.fulmen/app.yaml`:

```yaml
app:
  vendor: mycompany
  binary_name: myapi
  env_prefix: MYAPI_
  config_name: myapi
  description: My API service
  version: 0.1.0
metadata:
  repository_category: workhorse
```

Then:

```bash
make sync-embedded-identity
```

Identity controls:

- Environment variable prefixes (`MYAPI_*`)
- Config paths (`~/.config/myapi/config.yaml`)
- Telemetry namespace
- CLI binary name and `/version` `name`

`env_prefix` must be uppercase and end with `_`.

### 3.2 Cargo package and binary path

Edit `Cargo.toml`:

```toml
[package]
name = "myapi"

[[bin]]
name = "myapi"
path = "cmd/myapi/main.rs"
```

Keep `[lib] name = "app"` unless you also rename every `app::` import.

```bash
mkdir -p cmd/myapi
git mv cmd/roan/main.rs cmd/myapi/main.rs
```

Reset `VERSION` if you are starting a new product:

```bash
echo "0.1.0" > VERSION
```

### 3.3 Config and schemas

```bash
mv config/roan config/myapi
mv config/myapi/v1.0.0/roan-defaults.yaml config/myapi/v1.0.0/myapi-defaults.yaml
mv schemas/roan schemas/myapi
```

Keep `internal/assets/config/defaults.yaml` in sync with the renamed defaults file.

### 3.4 Environment file

```bash
cp .env.example .env
```

Rename every `ROAN_` key to `MYAPI_`.

### 3.5 Remaining template strings

```bash
rg -n "roan|ROAN_" --glob '!target/**' --glob '!.git/**'
```

Update README, Makefile help text that still names the old prefix, and this guide if you keep it.

`make validate-app-identity` scans `cmd/` and `internal/` (except the embedded identity YAML). Those trees must not contain the breed string or env prefix as literals.

### 3.6 Domain placeholder

Replace `internal/core` with your services. Keep `/health`, `/version`, and `/metrics`.

### 3.7 Documentation

Update `README.md`, `AGENTS.md`, and CLI `--help` copy (`description` in `.fulmen/app.yaml` is the starting point).

## 4. Launch

```bash
make bootstrap
make validate-app-identity
make doctor
make test
make build
./bin/myapi serve
```

## Verification

- [ ] `.fulmen/app.yaml` updated
- [ ] `make sync-embedded-identity` / `make verify-embedded-identity` pass
- [ ] `Cargo.toml` package and `[[bin]]` match `binary_name`
- [ ] `cmd/<binary>/` renamed
- [ ] `config/<config_name>/` and `schemas/<config_name>/` renamed
- [ ] `.env.example` / `.env` use the new prefix
- [ ] `make validate-app-identity` exits 0
- [ ] `make doctor` exits 0
- [ ] `make test` and `make build` pass
- [ ] `./bin/<binary> version --extended` shows rsfulmen and Crucible
- [ ] Binary copied out of the repo still runs `version` and `--help`

## Troubleshooting

### Identity not found

```bash
ls -la .fulmen/app.yaml
export FULMEN_APP_IDENTITY_FILE="$PWD/.fulmen/app.yaml"
```

Standalone binaries use the embedded mirror. If you changed identity and skipped `make sync-embedded-identity`, rebuild after syncing.

### Env vars ignored

```bash
grep env_prefix .fulmen/app.yaml
env | grep MYAPI_
./bin/myapi envinfo
```

### Config file not found

Defaults are embedded. A missing user file is not an error. To use a file:

```bash
myapi serve --config ./config/myapi/v1.0.0/myapi-defaults.yaml
```

## Parameterization points

| Point               | Source                                          |
| ------------------- | ----------------------------------------------- |
| Binary name         | `app.binary_name`                               |
| Env prefix          | `app.env_prefix`                                |
| Config paths        | `app.vendor` + `app.config_name`                |
| Cargo package       | `Cargo.toml`                                    |
| Telemetry namespace | `metadata.telemetry_namespace` or `binary_name` |

## Support

- Template: https://github.com/fulmenhq/forge-workhorse-roan
- Helper: https://github.com/fulmenhq/rsfulmen
- Standard: https://github.com/fulmenhq/crucible/blob/main/docs/architecture/fulmen-forge-workhorse-standard.md
