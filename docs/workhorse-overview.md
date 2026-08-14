---
title: "Workhorse Forge Overview"
description: "Module catalog for the Rust workhorse template"
---

# Workhorse Forge Overview

## Purpose and scope

Template for durable Rust backends and CLI tools. Pre-wires identity, configuration, observability, and a placeholder HTTP server. No product features.

## Integrated modules

Accessed through rsfulmen 0.1.5. This template does not depend on Crucible git.

| Module              | Tier        | Summary                                     |
| ------------------- | ----------- | ------------------------------------------- |
| App Identity        | Required    | `.fulmen/app.yaml` + embedded mirror        |
| Crucible Shim       | Required    | `rsfulmen::crucible` version and assets     |
| Three-layer config  | Required    | Embedded defaults → user file → env/flags   |
| Config Path API     | Required    | `rsfulmen::config::get_app_config_dir`      |
| Schema validation   | Required    | Embedded config schema + rsfulmen helpers   |
| Telemetry / metrics | Required    | Registry + Prometheus `/metrics`            |
| Logging             | Required    | SIMPLE / STRUCTURED profiles                |
| Error handling      | Required    | Fulmen error envelope                       |
| Signal handling     | Required    | Catalog, graceful shutdown, `/admin/signal` |
| Docscribe           | Required    | `/docs` sample + `envinfo`                  |
| Foundry             | Recommended | Exit-code catalog                           |
| sysprims / ipcprims | Optional    | Documented only; not crate deps             |

## Launch

See `docs/development/fulmen_cdrl_guide.md`, then `make bootstrap` and `make run`.
