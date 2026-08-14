//! Binary entrypoint. The package binary name is declared in Cargo.toml.

fn main() -> std::process::ExitCode {
    app::cmd::execute()
}
