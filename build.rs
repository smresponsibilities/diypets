use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=.git/HEAD");
    let git_hash = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "release".to_string());

    let target = std::env::var("TARGET").unwrap_or_else(|_| "unknown".to_string());
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "release".to_string());

    println!("cargo:rustc-env=DIYPETS_GIT_HASH={git_hash}");
    println!("cargo:rustc-env=DIYPETS_TARGET={target}");
    println!("cargo:rustc-env=DIYPETS_PROFILE={profile}");
}
