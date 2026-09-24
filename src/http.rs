use crate::language::trf;
use std::path::{Path, PathBuf};
use std::process::exit;

// если путь занят — добавляет _ в начало имени файла, пока не найдёт свободный
pub fn unique_dest(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    let dir = path.parent().unwrap_or(Path::new("."));
    let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
    let mut candidate = dir.join(format!("_{}", name));
    while candidate.exists() {
        let n = candidate.file_name().unwrap_or_default().to_string_lossy().into_owned();
        candidate = dir.join(format!("_{}", n));
    }
    candidate
}

pub fn build_client() -> reqwest::blocking::Client {
    let ua = format!("nymphalis/{} (+https://github.com/BlucherSKK/nymphalis)", crate::update::VERSION);
    match reqwest::blocking::Client::builder()
        .user_agent(ua)
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}", trf("Failed to build HTTP client: {}", &[&e]));
            exit(1);
        }
    }
}
