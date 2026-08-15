//! Fulmen workhorse library crate.
//!
//! The library is named `app` so application identity can change during CDRL
//! without rewriting every Rust module path. The binary name comes from
//! `.fulmen/app.yaml` and `Cargo.toml`.

#![warn(missing_docs)]
#![warn(rust_2018_idioms)]

pub mod appid;
pub mod cmd;
pub mod config;
pub mod core;
pub mod observability;
pub mod server;

/// Build version injected by `build.rs` (falls back to the crate version).
pub const BUILD_VERSION: &str = env!("APP_BUILD_VERSION");
/// Git commit injected by `build.rs`.
pub const BUILD_COMMIT: &str = env!("APP_BUILD_COMMIT");
/// UTC build timestamp injected by `build.rs`.
pub const BUILD_DATE: &str = env!("APP_BUILD_DATE");
