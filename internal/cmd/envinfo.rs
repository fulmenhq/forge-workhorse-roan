use super::CliError;
use crate::appid::Identity;
use crate::config::{env_var_names, LoadedConfig};
use crate::observability::Observability;
use crate::{BUILD_COMMIT, BUILD_DATE, BUILD_VERSION};
use rsfulmen::config::get_app_config_dir;
use rsfulmen::crucible;
use rsfulmen::docscribe;

pub fn run(
    identity: &Identity,
    loaded: &LoadedConfig,
    observability: &Observability,
) -> Result<(), CliError> {
    let header = format!("=== {} Environment Information ===", identity.binary_name);
    observability.logger.info(&header, &[]);

    println!("{header}");
    println!();
    println!("Application:");
    println!("  Name: {}", identity.binary_name);
    println!("  Vendor: {}", identity.vendor);
    println!("  Version: {BUILD_VERSION}");
    println!("  Commit: {BUILD_COMMIT}");
    println!("  Built: {BUILD_DATE}");
    println!("  Env prefix: {}", identity.env_prefix);
    println!("  Config name: {}", identity.config_name);
    println!();

    println!("SSOT:");
    println!("  rsfulmen: {}", rsfulmen::VERSION);
    println!("  Crucible: {}", crucible::version());
    if let Ok(doc) = docscribe::read_parsed_doc("architecture/fulmen-forge-workhorse-standard.md") {
        if let Some(title) = doc.frontmatter.title {
            println!("  Docscribe: {title}");
        }
    }
    println!();

    println!("Runtime:");
    println!("  OS: {}", std::env::consts::OS);
    println!("  Arch: {}", std::env::consts::ARCH);
    println!("  Family: {}", std::env::consts::FAMILY);
    println!();

    println!("Configuration:");
    println!("  Server Host: {}", loaded.config.server.host);
    println!("  Server Port: {}", loaded.config.server.port);
    println!("  Log Level: {}", loaded.config.logging.level);
    println!("  Log Profile: {}", loaded.config.logging.profile);
    println!("  Metrics Port: {}", loaded.config.metrics.port);
    match &loaded.user_path {
        Some(path) if path.exists() => println!("  Config File: {}", path.display()),
        Some(path) => println!(
            "  Config File: {} (not present; defaults + env)",
            path.display()
        ),
        None => println!("  Config File: (defaults and environment variables)"),
    }
    let config_dir = get_app_config_dir(&identity.config_name);
    println!("  Config Path API: {}", config_dir.display());
    println!();

    println!("Environment Variables:");
    println!("  Name                              Effective");
    println!("  ------------------------------------------------------------");
    for (name, path) in env_var_names(identity) {
        let effective = std::env::var(&name).unwrap_or_else(|_| "-".to_string());
        let display = if is_sensitive(&name) && effective != "-" {
            "[set]".to_string()
        } else {
            effective
        };
        println!("  {name:<33} {display} ({path})");
    }
    println!();
    println!("=== End Environment Information ===");
    Ok(())
}

fn is_sensitive(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    upper.contains("TOKEN")
        || upper.contains("SECRET")
        || upper.contains("PASSWORD")
        || upper.contains("KEY")
}
