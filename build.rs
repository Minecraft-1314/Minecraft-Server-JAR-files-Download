use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let version = Command::new("rustc")
        .arg("--version")
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout).ok()
            } else {
                None
            }
        })
        .and_then(|text| {
            text.split_whitespace()
                .nth(1)
                .map(|value| value.to_string())
        })
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=APP_RUST_VERSION={version}");
}
