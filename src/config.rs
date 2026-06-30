use crate::language::{tr, trf};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::exit;

const CONFIG_FILENAME: &str = "nymphalis.conf";
pub const DEFAULT_JOBS: usize = 4;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jobs: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub desu_session: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patreon_session: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patreon_proxy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub desu_proxy: Option<String>,
}

pub fn config_path() -> PathBuf {
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    Path::new(&home).join(".config").join(CONFIG_FILENAME)
}

pub fn load_config() -> Config {
    match fs::read_to_string(config_path()) {
        Ok(contents) => serde_yaml::from_str(&contents).unwrap_or_default(),
        Err(_) => Config::default(),
    }
}

pub fn save_config(config: &Config) {
    let path = config_path();
    if let Some(parent) = path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            eprintln!(
                "{}",
                trf("Failed to create directory '{}': {}", &[&parent.display(), &e])
            );
            exit(1);
        }
    }
    let yaml = match serde_yaml::to_string(config) {
        Ok(y) => y,
        Err(e) => {
            eprintln!("{}", trf("Failed to serialize config: {}", &[&e]));
            exit(1);
        }
    };
    if let Err(e) = fs::write(&path, yaml) {
        eprintln!("{}", trf("Failed to write {}: {}", &[&path.display(), &e]));
        exit(1);
    }
}

pub fn parallel_jobs() -> usize {
    if let Some(n) = env::var("BOORU_JOBS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|&n| n > 0)
    {
        return n;
    }

    let config = load_config();
    if let Some(n) = config.jobs.filter(|&n| n > 0) {
        return n as usize;
    }

    DEFAULT_JOBS
}

pub fn run_set(rest: &[String]) {
    if rest.len() != 2 {
        eprintln!("{}", tr("set requires exactly <variable> <value>.\n"));
        exit(1);
    }

    let key = rest[0].to_lowercase();
    let value = rest[1].clone();

    let mut config = load_config();

    match key.as_str() {
        "user_id"         => config.user_id         = Some(value),
        "api_key"         => config.api_key         = Some(value),
        "desu_session"    => config.desu_session    = Some(value),
        "patreon_session" => config.patreon_session = Some(value),
        "patreon_proxy"   => config.patreon_proxy   = Some(value),
        "desu_proxy"      => config.desu_proxy      = Some(value),
        "jobs" => match value.parse::<u32>() {
            Ok(n) if n > 0 => config.jobs = Some(n),
            _ => {
                eprintln!("{}", tr("jobs must be a positive integer."));
                exit(1);
            }
        },
        other => {
            eprintln!(
                "{}",
                trf(
                    "Unknown variable '{}'. Supported: user_id, api_key, desu_session, desu_proxy, patreon_session, patreon_proxy, jobs",
                    &[&other]
                )
            );
            exit(1);
        }
    }

    save_config(&config);
    println!(
        "{}",
        trf("Saved '{}' to {}", &[&key, &config_path().display()])
    );
}
