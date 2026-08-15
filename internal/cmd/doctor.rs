use super::CliError;
use crate::appid::Identity;
use crate::config::LoadedConfig;
use crate::observability::Observability;
use rsfulmen::config::get_app_config_dir;
use rsfulmen::crucible;
use rsfulmen::docscribe;

pub fn run(
    identity: &Identity,
    loaded: &LoadedConfig,
    observability: &Observability,
) -> Result<(), CliError> {
    let banner = format!("{} doctor", identity.binary_name);
    observability.logger.info(&format!("=== {banner} ==="), &[]);

    println!("=== {banner} ===");
    println!();
    let mut ok = true;

    let rustc = env!("CARGO_PKG_RUST_VERSION");
    println!("[1/6] Checking Rust MSRV... {rustc}");

    let crucible_version = crucible::version();
    if crucible_version.is_empty() {
        println!("[2/6] Checking Crucible access... FAIL");
        ok = false;
    } else {
        println!("[2/6] Checking Crucible access... {crucible_version}");
    }

    println!("[3/6] Checking rsfulmen access... {}", rsfulmen::VERSION);

    let config_dir = get_app_config_dir(&identity.config_name);
    println!(
        "[4/6] Checking config directory... {}",
        config_dir.display()
    );

    println!(
        "[5/6] Checking environment... {}/{}",
        std::env::consts::OS,
        std::env::consts::ARCH
    );

    match docscribe::read_doc("architecture/fulmen-forge-workhorse-standard.md") {
        Ok(_) => println!("[6/6] Checking docscribe... workhorse standard present"),
        Err(err) => {
            println!("[6/6] Checking docscribe... FAIL ({err})");
            ok = false;
        }
    }

    println!();
    println!("Identity:");
    println!("  binary_name: {}", identity.binary_name);
    println!("  vendor: {}", identity.vendor);
    println!("  env_prefix: {}", identity.env_prefix);
    println!("  config_name: {}", identity.config_name);
    println!(
        "  listen: {}:{}",
        loaded.config.server.host, loaded.config.server.port
    );

    if !identity.env_prefix.ends_with('_') {
        println!("  env_prefix must end with '_'");
        ok = false;
    }

    println!();
    if ok {
        println!("All checks passed. Installation is healthy.");
        Ok(())
    } else {
        Err(CliError::Config("doctor reported failed checks".into()))
    }
}
