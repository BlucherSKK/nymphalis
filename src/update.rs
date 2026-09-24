use crate::http::build_client;
use crate::language::{tr, trf};
use serde::Deserialize;
use std::cmp::Ordering;
use std::process::exit;

pub const VERSION: &str = env!("NYMPHALIS_VERSION");
pub const PLATFORM: &str = env!("NYMPHALIS_PLATFORM");

const API_URL: &str = "https://api.github.com/repos/BlucherSKK/nymphalis/releases/latest";

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct ReleaseAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: Option<u64>,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct GithubRelease {
    pub tag_name: String,
    pub name: Option<String>,
    pub html_url: String,
    pub body: Option<String>,
    pub assets: Vec<ReleaseAsset>,
}

pub fn print_version() {
    println!("{} {}", VERSION, PLATFORM);
}

fn parse_version(v: &str) -> Vec<u64> {
    let clean = v.trim().strip_prefix('v').unwrap_or(v.trim());
    clean
        .split(|c: char| c == '.' || c == '-' || c == '+')
        .filter_map(|part| part.parse::<u64>().ok())
        .collect()
}

fn compare_versions(v1: &str, v2: &str) -> Ordering {
    let p1 = parse_version(v1);
    let p2 = parse_version(v2);
    if !p1.is_empty() && !p2.is_empty() {
        p1.cmp(&p2)
    } else {
        let c1 = v1.trim().strip_prefix('v').unwrap_or(v1.trim());
        let c2 = v2.trim().strip_prefix('v').unwrap_or(v2.trim());
        c1.cmp(c2)
    }
}

pub fn find_platform_asset<'a>(assets: &'a [ReleaseAsset], platform: &str) -> Option<&'a ReleaseAsset> {
    let exact_name = format!("nymphalis-{}", platform);
    let exact_exe = format!("nymphalis-{}.exe", platform);

    // Exact match first
    if let Some(a) = assets.iter().find(|a| a.name == exact_name || a.name == exact_exe) {
        return Some(a);
    }

    // Substring match
    assets.iter().find(|a| a.name.contains(platform))
}

pub fn run_update(args: &[String]) {
    println!("{}", tr("Checking for latest release from GitHub..."));

    let client = build_client();
    let response = match client
        .get(API_URL)
        .header("Accept", "application/vnd.github.v3+json")
        .send()
    {
        Ok(resp) => resp,
        Err(e) => {
            eprintln!("{}", trf("Failed to check for updates: {}", &[&e]));
            exit(1);
        }
    };

    if response.status() == reqwest::StatusCode::FORBIDDEN {
        eprintln!("{}", tr("GitHub API rate limit exceeded. Please try again later."));
        exit(1);
    }

    if !response.status().is_success() {
        eprintln!("{}", trf("GitHub API returned error: {}", &[&response.status()]));
        exit(1);
    }

    let release: GithubRelease = match response.json() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{}", trf("Failed to parse release information: {}", &[&e]));
            exit(1);
        }
    };

    let latest_tag = release.tag_name.trim();
    let ord = compare_versions(VERSION, latest_tag);

    match ord {
        Ordering::Equal => {
            println!(
                "{}",
                trf("You are using the latest version: {} ({})", &[&VERSION, &PLATFORM])
            );
        }
        Ordering::Greater => {
            println!(
                "{}",
                trf(
                    "Current version ({} {}) is newer than the latest release ({}).",
                    &[&VERSION, &PLATFORM, &latest_tag]
                )
            );
        }
        Ordering::Less => {
            println!(
                "{}",
                trf(
                    "A new version is available: {} (current: {} {})",
                    &[&latest_tag, &VERSION, &PLATFORM]
                )
            );

            if let Some(name) = &release.name {
                let name = name.trim();
                if !name.is_empty() && name != latest_tag {
                    println!("{}: {}", tr("Release"), name);
                }
            }

            let asset = find_platform_asset(&release.assets, PLATFORM);
            if let Some(asset) = asset {
                println!();
                println!(
                    "{}",
                    trf(
                        "Direct download link for your platform ({}):\n  {}",
                        &[&PLATFORM, &asset.browser_download_url]
                    )
                );
            }

            println!();
            println!("{}: {}", tr("Release page"), release.html_url);

            if let Some(body) = &release.body {
                let trimmed = body.trim();
                if !trimmed.is_empty() {
                    println!();
                    println!("{}:\n{}", tr("Changelog"), trimmed);
                }
            }

            if args.iter().any(|a| a == "--download" || a == "-d") {
                if let Some(asset) = asset {
                    download_asset(&client, asset);
                } else {
                    eprintln!("{}", trf("No compatible asset found for platform {}", &[&PLATFORM]));
                }
            }
        }
    }
}

fn download_asset(client: &reqwest::blocking::Client, asset: &ReleaseAsset) {
    println!();
    println!("{}", trf("Downloading {}...", &[&asset.name]));

    let mut response = match client.get(&asset.browser_download_url).send() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{}", trf("Failed to download asset: {}", &[&e]));
            exit(1);
        }
    };

    if !response.status().is_success() {
        eprintln!("{}", trf("Failed to download asset: HTTP {}", &[&response.status()]));
        exit(1);
    }

    let file_path = std::path::Path::new(&asset.name);
    let mut file = match std::fs::File::create(file_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("{}", trf("Failed to create file: {}", &[&e]));
            exit(1);
        }
    };

    if let Err(e) = std::io::copy(&mut response, &mut file) {
        eprintln!("{}", trf("Failed to write {}: {}", &[&asset.name, &e]));
        exit(1);
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = std::fs::metadata(file_path) {
            let mut perms = metadata.permissions();
            perms.set_mode(0o755);
            let _ = std::fs::set_permissions(file_path, perms);
        }
    }

    println!("{}", trf("Downloaded to {}", &[&file_path.display()]));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_and_compare_versions() {
        assert_eq!(compare_versions("0.0.5", "0.0.7"), Ordering::Less);
        assert_eq!(compare_versions("0.0.7", "0.0.7"), Ordering::Equal);
        assert_eq!(compare_versions("0.0.8", "0.0.7"), Ordering::Greater);
        assert_eq!(compare_versions("v0.0.5", "0.0.7"), Ordering::Less);
        assert_eq!(compare_versions("0.1.0", "0.0.9"), Ordering::Greater);
    }

    #[test]
    fn test_find_platform_asset() {
        let assets = vec![
            ReleaseAsset {
                name: "nymphalis-linux-amd64".to_string(),
                browser_download_url: "https://example.com/linux-amd64".to_string(),
                size: None,
            },
            ReleaseAsset {
                name: "nymphalis-macos-amd64".to_string(),
                browser_download_url: "https://example.com/macos-amd64".to_string(),
                size: None,
            },
            ReleaseAsset {
                name: "nymphalis-windows-amd64.exe".to_string(),
                browser_download_url: "https://example.com/windows-amd64".to_string(),
                size: None,
            },
        ];

        let macos_asset = find_platform_asset(&assets, "macos-amd64");
        assert!(macos_asset.is_some());
        assert_eq!(macos_asset.unwrap().name, "nymphalis-macos-amd64");

        let windows_asset = find_platform_asset(&assets, "windows-amd64");
        assert!(windows_asset.is_some());
        assert_eq!(windows_asset.unwrap().name, "nymphalis-windows-amd64.exe");

        let non_existent = find_platform_asset(&assets, "freebsd-amd64");
        assert!(non_existent.is_none());
    }
}

