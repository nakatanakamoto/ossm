fn main() {
    println!("cargo:rustc-env=OSSM_DEVICE={}", std::env::var("CARGO_PKG_NAME").unwrap());

    let commit = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=OSSM_COMMIT_HASH={commit}");

    let release_id = std::env::var("OSSM_RELEASE_ID").unwrap_or_else(|_| "dev".into());
    println!("cargo:rustc-env=OSSM_RELEASE_ID={release_id}");

    let build_time = std::process::Command::new("date")
        .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=OSSM_BUILD_TIME={build_time}");
}
