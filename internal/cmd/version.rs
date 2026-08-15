use super::CliError;
use crate::appid::Identity;
use crate::{BUILD_COMMIT, BUILD_DATE, BUILD_VERSION};
use clap::Args;
use rsfulmen::crucible;

/// Flags for `version`.
#[derive(Debug, Args)]
#[command(after_help = "Examples:\n  \
    version\n  \
    version --extended\n")]
pub struct VersionArgs {
    /// Show helper, Crucible, and build metadata.
    #[arg(short, long)]
    pub extended: bool,
}

pub fn run(identity: &Identity, args: VersionArgs) -> Result<(), CliError> {
    if args.extended {
        println!("{} {BUILD_VERSION}", identity.binary_name);
        println!("Commit: {BUILD_COMMIT}");
        println!("Built: {BUILD_DATE}");
        println!("Rustc MSRV: {}", env!("CARGO_PKG_RUST_VERSION"));
        println!();
        println!("rsfulmen: {}", rsfulmen::VERSION);
        println!("Crucible: {}", crucible::version());
    } else {
        println!("{} {BUILD_VERSION}", identity.binary_name);
    }
    Ok(())
}
