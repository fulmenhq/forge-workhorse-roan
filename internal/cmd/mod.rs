//! CLI surface: serve, version, health, envinfo, doctor.

mod doctor;
mod envinfo;
mod health;
mod serve;
mod version;

use crate::appid;
use crate::config::{self, LoadOptions};
use crate::observability::Observability;
use clap::{Parser, Subcommand};
use rsfulmen::error_handling::ErrorResponse;
use rsfulmen::foundry::exit_codes;
use std::path::PathBuf;
use std::process::ExitCode;

/// CLI arguments shared by every subcommand.
#[derive(Debug, Parser)]
#[command(
    name = env!("CARGO_PKG_NAME"),
    version = crate::BUILD_VERSION,
    about = "Fulmen workhorse CLI",
    long_about = None,
    next_line_help = true
)]
pub struct Cli {
    /// Path to a Layer 2/3 YAML config file.
    #[arg(long, global = true, value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// Verbose output (sets log level to debug).
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Subcommand to run.
    #[command(subcommand)]
    pub command: Commands,
}

/// Workhorse subcommands.
#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Start the HTTP server.
    Serve(serve::ServeArgs),
    /// Print version information.
    Version(version::VersionArgs),
    /// Run a local self-health check.
    Health,
    /// Print effective environment and config layers.
    Envinfo,
    /// Run diagnostic checks.
    Doctor,
}

/// Execute the CLI.
pub fn execute() -> ExitCode {
    match execute_inner() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            let code = err.exit_code();
            let payload = ErrorResponse::new(err.code(), err.to_string());
            if let Ok(json) = payload.to_json_string() {
                eprintln!("{json}");
            } else {
                eprintln!("{err}");
            }
            ExitCode::from(code)
        }
    }
}

fn execute_inner() -> Result<(), CliError> {
    let cli = Cli::parse();
    let identity = appid::get().map_err(|e| CliError::Identity(e.to_string()))?;
    let runtime = runtime_overrides(&cli)?;
    let load_options = LoadOptions {
        config_path: cli.config.clone(),
        runtime_overrides: runtime,
    };
    let loaded = config::load(identity, load_options.clone())
        .map_err(|e| CliError::Config(e.to_string()))?;

    let obs = Observability::new(identity, &loaded.config.logging, cli.verbose);

    match cli.command {
        Commands::Serve(args) => serve::run(identity, &loaded.config, &obs, args, load_options),
        Commands::Version(args) => version::run(identity, args),
        Commands::Health => health::run(identity, &loaded.config, &obs),
        Commands::Envinfo => envinfo::run(identity, &loaded, &obs),
        Commands::Doctor => doctor::run(identity, &loaded, &obs),
    }
}

fn runtime_overrides(cli: &Cli) -> Result<Option<serde_yaml::Value>, CliError> {
    if let Commands::Serve(args) = &cli.command {
        return Ok(Some(args.as_runtime_overrides()?));
    }
    Ok(None)
}

/// CLI failures.
#[derive(Debug, thiserror::Error)]
pub enum CliError {
    /// Identity could not be loaded.
    #[error("{0}")]
    Identity(String),
    /// Config could not be loaded.
    #[error("{0}")]
    Config(String),
    /// Server failed.
    #[error("{0}")]
    Server(String),
    /// Invalid CLI flag.
    #[error("{0}")]
    Usage(String),
}

impl CliError {
    fn code(&self) -> &'static str {
        match self {
            CliError::Identity(_) => "IDENTITY_NOT_FOUND",
            CliError::Config(_) => "CONFIG_INVALID",
            CliError::Server(_) => "SERVER_ERROR",
            CliError::Usage(_) => "INVALID_ARGUMENT",
        }
    }

    fn exit_code(&self) -> u8 {
        match self {
            CliError::Identity(_) => exit_code_or(66),
            CliError::Config(_) => exit_code_or(78),
            CliError::Server(_) => exit_code_or(69),
            CliError::Usage(_) => exit_code_or(64),
        }
    }
}

fn exit_code_or(fallback: u8) -> u8 {
    // Prefer Foundry catalog codes when the helper exposes them.
    let _ = exit_codes::get_exit_name(i32::from(fallback));
    fallback
}
