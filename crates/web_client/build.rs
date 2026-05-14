fn main() {
    let version = std::process::Command::new("git")
        .args(["describe", "--tags", "--always", "--abbrev=0"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());

    println!("cargo:rustc-env=APP_BUILD_VERSION={}", version);
}
