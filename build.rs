use std::env;
use std::path::Path;
use std::process::Command;

fn detect_version() -> String {
    if let Ok(v) = env::var("NYMPHALIS_VERSION") {
        let trimmed = v.trim();
        if !trimmed.is_empty() {
            return trimmed.strip_prefix('v').unwrap_or(trimmed).to_string();
        }
    }

    if let Ok(output) = Command::new("git").args(["describe", "--tags", "--abbrev=0"]).output() {
        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !s.is_empty() {
                return s.strip_prefix('v').unwrap_or(&s).to_string();
            }
        }
    }

    if let Ok(output) = Command::new("git").args(["describe", "--tags"]).output() {
        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !s.is_empty() {
                return s.strip_prefix('v').unwrap_or(&s).to_string();
            }
        }
    }

    env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".to_string())
}

fn detect_platform() -> String {
    if let Ok(p) = env::var("NYMPHALIS_PLATFORM") {
        let trimmed = p.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    let target = env::var("TARGET").unwrap_or_default();
    match target.as_str() {
        "x86_64-unknown-linux-gnu" => "linux-amd64".to_string(),
        "x86_64-unknown-linux-musl" => "linux-amd64-musl".to_string(),
        "aarch64-unknown-linux-gnu" => "linux-arm64".to_string(),
        "aarch64-unknown-linux-musl" => "linux-arm64-musl".to_string(),
        "x86_64-apple-darwin" => "macos-amd64".to_string(),
        "aarch64-apple-darwin" => "macos-arm64".to_string(),
        "x86_64-pc-windows-msvc" | "x86_64-pc-windows-gnu" => "windows-amd64".to_string(),
        "aarch64-pc-windows-msvc" | "aarch64-pc-windows-gnu" => "windows-arm64".to_string(),
        _ => {
            let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_else(|_| "unknown".to_string());
            let os_name = match os.as_str() {
                "darwin" => "macos",
                other => other,
            };
            let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_else(|_| "unknown".to_string());
            let arch_name = match arch.as_str() {
                "x86_64" => "amd64",
                "aarch64" => "arm64",
                other => other,
            };
            let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
            if target_env == "musl" && os_name == "linux" {
                format!("{}-{}-musl", os_name, arch_name)
            } else {
                format!("{}-{}", os_name, arch_name)
            }
        }
    }
}

fn main() {
    let version = detect_version();
    let platform = detect_platform();

    println!("cargo:rustc-env=NYMPHALIS_VERSION={}", version);
    println!("cargo:rustc-env=NYMPHALIS_PLATFORM={}", platform);
    println!("cargo:rerun-if-env-changed=NYMPHALIS_VERSION");
    println!("cargo:rerun-if-env-changed=NYMPHALIS_PLATFORM");

    if Path::new(".git/HEAD").exists() {
        println!("cargo:rerun-if-changed=.git/HEAD");
    }
    if Path::new(".git/refs/tags").exists() {
        println!("cargo:rerun-if-changed=.git/refs/tags");
    }
}
