# Changelog

## Unreleased

- Wire `rsfulmen::signals` for SIGTERM/SIGINT, catalog double-tap, and SIGHUP config reload.
- Add default `X-Request-ID` middleware and request-correlated logs.

## 0.1.0

Initial Rust workhorse template: CLI (`serve`, `version`, `health`, `envinfo`, `doctor`), HTTP (`/health`, `/version`, `/metrics`), App Identity, rsfulmen 0.1.5, goneat v0.5.16 DX, and CDRL make targets.
