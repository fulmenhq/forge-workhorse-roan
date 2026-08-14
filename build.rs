use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=VERSION");
    println!("cargo:rerun-if-changed=.fulmen/app.yaml");
    println!("cargo:rerun-if-changed=internal/assets/appidentity/app.yaml");

    let version = std::fs::read_to_string("VERSION")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());

    let commit = git_output(&["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| "unknown".into());
    let build_date = std::env::var("SOURCE_DATE_EPOCH")
        .ok()
        .and_then(|epoch| epoch.parse::<i64>().ok())
        .map(|secs| format!("{secs}"))
        .unwrap_or_else(utc_now);

    println!("cargo:rustc-env=APP_BUILD_VERSION={version}");
    println!("cargo:rustc-env=APP_BUILD_COMMIT={commit}");
    println!("cargo:rustc-env=APP_BUILD_DATE={build_date}");
}

fn git_output(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn utc_now() -> String {
    // Keep build.rs dependency-free. RFC3339-ish UTC from `date` when available.
    if let Some(value) = Command::new("date")
        .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
    {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    "unknown".to_string()
}
