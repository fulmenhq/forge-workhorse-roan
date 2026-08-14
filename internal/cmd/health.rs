use super::CliError;
use crate::appid::Identity;
use crate::config::Config;
use crate::observability::Observability;
use crate::BUILD_VERSION;

pub fn run(
    identity: &Identity,
    config: &Config,
    observability: &Observability,
) -> Result<(), CliError> {
    observability.logger.info(
        "running health check",
        &[("binary", identity.binary_name.as_str())],
    );

    observability.logger.info(
        "version information available",
        &[("version", BUILD_VERSION)],
    );
    observability.logger.info("logger initialized", &[]);
    observability.logger.info(
        "configuration system ready",
        &[("health_enabled", config.health.enabled.to_string().as_str())],
    );
    println!("ok");
    println!("status: healthy");
    println!("version: {BUILD_VERSION}");
    Ok(())
}
