use crate::language::trf;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserKind {
    Firefox,
    Chromium(&'static str),
    Falkon,
}

#[derive(Debug, Clone, Copy)]
pub struct Browser {
    pub bin: &'static str,
    #[allow(dead_code)]
    pub mac_app: &'static str,
    #[allow(dead_code)]
    pub mac_bundle: &'static str,
    pub kind: BrowserKind,
}

pub fn resolve_browser(name: &str) -> Option<Browser> {
    match name.to_lowercase().as_str() {
        "firefox" | "ff" => Some(Browser {
            bin: "firefox",
            mac_app: "Firefox",
            mac_bundle: "org.mozilla.firefox",
            kind: BrowserKind::Firefox,
        }),
        "chromium" => Some(Browser {
            bin: "chromium",
            mac_app: "Chromium",
            mac_bundle: "org.chromium.Chromium",
            kind: BrowserKind::Chromium("chromium"),
        }),
        "chrome" | "google-chrome" => Some(Browser {
            bin: "google-chrome",
            mac_app: "Google Chrome",
            mac_bundle: "com.google.Chrome",
            kind: BrowserKind::Chromium("google-chrome"),
        }),
        "brave" | "brave-browser" => Some(Browser {
            bin: "brave-browser",
            mac_app: "Brave Browser",
            mac_bundle: "com.brave.Browser",
            kind: BrowserKind::Chromium("brave-browser"),
        }),
        "falkon" => Some(Browser {
            bin: "falkon",
            mac_app: "Falkon",
            mac_bundle: "org.kde.falkon",
            kind: BrowserKind::Falkon,
        }),
        _ => None,
    }
}

#[cfg(target_os = "macos")]
fn default_open_command() -> &'static str {
    "open"
}

#[cfg(target_os = "windows")]
fn default_open_command() -> &'static str {
    "start"
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn default_open_command() -> &'static str {
    "xdg-open"
}

pub fn open_browser(url: &str, browser: Option<&Browser>) {
    let launched = open_browser_impl(url, browser);
    if !launched {
        let cmd = browser.map(|b| b.bin).unwrap_or_else(default_open_command);
        println!("{}", trf("Could not launch '{}' automatically.", &[&cmd]));
        println!("{}", trf("Please open this URL manually: {}", &[&url]));
    }
}

#[cfg(target_os = "macos")]
fn open_browser_impl(url: &str, browser: Option<&Browser>) -> bool {
    let open_bin = if Path::new("/usr/bin/open").exists() {
        "/usr/bin/open"
    } else {
        "open"
    };

    match browser {
        None => {
            std::process::Command::new(open_bin)
                .arg(url)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        }
        Some(b) => {
            // 1. Try launching by application name via `open -a "<mac_app>" <url>`
            if let Ok(status) = std::process::Command::new(open_bin)
                .args(["-a", b.mac_app, url])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
            {
                if status.success() {
                    return true;
                }
            }

            // 2. Try launching by bundle identifier via `open -b "<mac_bundle>" <url>`
            if !b.mac_bundle.is_empty() {
                if let Ok(status) = std::process::Command::new(open_bin)
                    .args(["-b", b.mac_bundle, url])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status()
                {
                    if status.success() {
                        return true;
                    }
                }
            }

            // 3. Try known CLI directories for Homebrew on Apple Silicon (ARM) & Intel (x86_64)
            let brew_dirs = [
                Path::new("/opt/homebrew/bin"), // macOS ARM (Apple Silicon)
                Path::new("/usr/local/bin"),     // macOS x86 (Intel)
            ];
            for prefix in &brew_dirs {
                let candidate = prefix.join(b.bin);
                if candidate.is_file() {
                    if let Ok(_) = std::process::Command::new(&candidate)
                        .arg(url)
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .spawn()
                    {
                        return true;
                    }
                }
            }

            // 4. Try direct command execution from PATH
            if let Ok(_) = std::process::Command::new(b.bin)
                .arg(url)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
            {
                return true;
            }

            false
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn open_browser_impl(url: &str, browser: Option<&Browser>) -> bool {
    let cmd = browser.map(|b| b.bin).unwrap_or("xdg-open");
    let launched = std::process::Command::new(cmd)
        .arg(url)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .is_ok();

    if !launched && browser.is_some() {
        std::process::Command::new("xdg-open")
            .arg(url)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .is_ok()
    } else {
        launched
    }
}

#[cfg(target_os = "windows")]
fn open_browser_impl(url: &str, browser: Option<&Browser>) -> bool {
    match browser {
        None => {
            std::process::Command::new("cmd")
                .args(["/c", "start", "", url])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .is_ok()
        }
        Some(b) => {
            let launched = std::process::Command::new("cmd")
                .args(["/c", "start", b.bin, url])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .is_ok();
            if !launched {
                std::process::Command::new(b.bin)
                    .arg(url)
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                    .is_ok()
            } else {
                true
            }
        }
    }
}

pub fn firefox_cookie_paths() -> Vec<PathBuf> {
    let home = match std::env::var("HOME") {
        Ok(h) => PathBuf::from(h),
        Err(_) => return Vec::new(),
    };
    let candidate_dirs = [
        home.join(".mozilla").join("firefox"),
        home.join("Library").join("Application Support").join("Firefox").join("Profiles"),
    ];
    let mut paths = Vec::new();
    for base in &candidate_dirs {
        if let Ok(entries) = fs::read_dir(base) {
            for entry in entries.flatten() {
                let db = entry.path().join("cookies.sqlite");
                if db.exists() {
                    paths.push(db);
                }
            }
        }
    }
    paths
}

pub fn chromium_cookie_paths(browser_name: &str) -> Vec<PathBuf> {
    let home = match std::env::var("HOME") {
        Ok(h) => PathBuf::from(h),
        Err(_) => return Vec::new(),
    };

    let mut candidate_dirs = Vec::new();

    // Linux paths: ~/.config/<browser_name>/...
    candidate_dirs.push(home.join(".config").join(browser_name));

    // macOS paths: ~/Library/Application Support/...
    let mac_subpath = match browser_name {
        "google-chrome" => Some(home.join("Library").join("Application Support").join("Google").join("Chrome")),
        "chromium"      => Some(home.join("Library").join("Application Support").join("Chromium")),
        "brave-browser" => Some(home.join("Library").join("Application Support").join("BraveSoftware").join("Brave-Browser")),
        _ => None,
    };
    if let Some(p) = mac_subpath {
        candidate_dirs.push(p);
    }

    let mut paths = Vec::new();
    for base in &candidate_dirs {
        let def = base.join("Default").join("Cookies");
        if def.exists() {
            paths.push(def);
        }
        if let Ok(entries) = fs::read_dir(base) {
            for entry in entries.flatten() {
                let p = entry.path();
                if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with("Profile ") {
                        let db = p.join("Cookies");
                        if db.exists() {
                            paths.push(db);
                        }
                    }
                }
            }
        }
    }
    paths
}

pub fn falkon_cookie_paths() -> Vec<PathBuf> {
    let home = match std::env::var("HOME") {
        Ok(h) => PathBuf::from(h),
        Err(_) => return Vec::new(),
    };
    let candidate_dirs = [
        home.join(".config").join("falkon").join("profiles"),
        home.join("Library").join("Application Support").join("falkon").join("profiles"),
    ];
    let mut paths = Vec::new();
    for base in &candidate_dirs {
        if let Ok(entries) = fs::read_dir(base) {
            for entry in entries.flatten() {
                let db = entry.path().join("Cookies");
                if db.exists() {
                    paths.push(db);
                }
            }
        }
    }
    paths
}

pub fn sqlite3_query(db: &Path, sql: &str, tmp_prefix: &str) -> Option<String> {
    let tmp     = std::env::temp_dir().join(format!("{}.sqlite", tmp_prefix));
    let tmp_wal = std::env::temp_dir().join(format!("{}.sqlite-wal", tmp_prefix));
    let tmp_shm = std::env::temp_dir().join(format!("{}.sqlite-shm", tmp_prefix));

    fs::copy(db, &tmp).ok()?;
    let wal = PathBuf::from(format!("{}-wal", db.display()));
    let shm = PathBuf::from(format!("{}-shm", db.display()));
    if wal.exists() { let _ = fs::copy(&wal, &tmp_wal); }
    if shm.exists() { let _ = fs::copy(&shm, &tmp_shm); }

    let out = std::process::Command::new("sqlite3")
        .arg(&tmp)
        .arg(sql)
        .output()
        .or_else(|_| {
            std::process::Command::new("/usr/bin/sqlite3")
                .arg(&tmp)
                .arg(sql)
                .output()
        })
        .ok();

    let _ = fs::remove_file(&tmp);
    let _ = fs::remove_file(&tmp_wal);
    let _ = fs::remove_file(&tmp_shm);

    let out = out?;
    if !out.status.success() { return None; }
    let v = String::from_utf8(out.stdout).ok()?;
    let v = v.trim().to_string();
    if v.is_empty() { None } else { Some(v) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_browser() {
        assert_eq!(resolve_browser("firefox").map(|b| b.bin), Some("firefox"));
        assert_eq!(resolve_browser("ff").map(|b| b.bin), Some("firefox"));
        assert_eq!(resolve_browser("chrome").map(|b| b.bin), Some("google-chrome"));
        assert_eq!(resolve_browser("google-chrome").map(|b| b.bin), Some("google-chrome"));
        assert_eq!(resolve_browser("chromium").map(|b| b.bin), Some("chromium"));
        assert_eq!(resolve_browser("brave").map(|b| b.bin), Some("brave-browser"));
        assert_eq!(resolve_browser("brave-browser").map(|b| b.bin), Some("brave-browser"));
        assert_eq!(resolve_browser("falkon").map(|b| b.bin), Some("falkon"));
        assert!(resolve_browser("unknown-browser").is_none());
    }

    #[test]
    fn test_browser_macos_metadata() {
        let ff = resolve_browser("firefox").unwrap();
        assert_eq!(ff.mac_app, "Firefox");
        assert_eq!(ff.mac_bundle, "org.mozilla.firefox");

        let gc = resolve_browser("chrome").unwrap();
        assert_eq!(gc.mac_app, "Google Chrome");
        assert_eq!(gc.mac_bundle, "com.google.Chrome");

        let br = resolve_browser("brave").unwrap();
        assert_eq!(br.mac_app, "Brave Browser");
        assert_eq!(br.mac_bundle, "com.brave.Browser");

        let cr = resolve_browser("chromium").unwrap();
        assert_eq!(cr.mac_app, "Chromium");
        assert_eq!(cr.mac_bundle, "org.chromium.Chromium");

        let fa = resolve_browser("falkon").unwrap();
        assert_eq!(fa.mac_app, "Falkon");
        assert_eq!(fa.mac_bundle, "org.kde.falkon");
    }

    #[test]
    fn test_resolve_browser_case_insensitivity() {
        assert_eq!(resolve_browser("FIREFOX").map(|b| b.bin), Some("firefox"));
        assert_eq!(resolve_browser("Chrome").map(|b| b.bin), Some("google-chrome"));
        assert_eq!(resolve_browser("BrAvE").map(|b| b.bin), Some("brave-browser"));
        assert_eq!(resolve_browser("CHROMIUM").map(|b| b.bin), Some("chromium"));
        assert_eq!(resolve_browser("FALKON").map(|b| b.bin), Some("falkon"));
    }

    #[test]
    fn test_cookie_paths_do_not_panic() {
        let _ = firefox_cookie_paths();
        let _ = chromium_cookie_paths("google-chrome");
        let _ = chromium_cookie_paths("chromium");
        let _ = chromium_cookie_paths("brave-browser");
        let _ = falkon_cookie_paths();
    }
}
