use std::{env, process::Command};

fn emit_version() {
    let package_name = env::var("CARGO_PKG_NAME").unwrap();
    let package_version = env::var("CARGO_PKG_VERSION").unwrap();

    let git_commit_sha = env::var("TUNE_MANAGER_VERSION").unwrap_or_else(|_| {
        Command::new("git")
            .args(["rev-parse", "HEAD"])
            .output()
            .map_err(|e| {
                println!("cargo:warning=git rev-parse failure, unable to determine revision");
                e
            })
            .map(|cmd| String::from_utf8(cmd.stdout).unwrap())
            .unwrap_or("unknown-rev".to_string())
    });

    let release_name = format!("{}@{}+{}", package_name, package_version, git_commit_sha);
    println!("cargo:rustc-env=TUNE_MANAGER_VERSION={}", release_name);
}

fn main() {
    emit_version()
}
