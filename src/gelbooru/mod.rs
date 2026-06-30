use crate::config::{self, config_path, load_config, parallel_jobs};
use crate::http::build_client;
use crate::language::{tr, trf};
use indicatif::{HumanBytes, ProgressBar, ProgressStyle};
use serde_json::Value;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::exit;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::thread::{self, sleep};
use std::time::{Duration, Instant};
use std::env;

const API_BASE: &str = "https://gelbooru.com/index.php";
const PAGE_LIMIT: u32 = 100;
const DELAY_BETWEEN_DOWNLOADS: Duration = Duration::from_secs(1);

pub struct Credentials {
    pub user_id: String,
    pub api_key: String,
}

pub fn get_credentials() -> Option<Credentials> {
    let env_user = env::var("GELBOORU_USER_ID").ok().filter(|s| !s.is_empty());
    let env_key = env::var("GELBOORU_API_KEY").ok().filter(|s| !s.is_empty());

    let config = load_config();

    let user_id = env_user.or_else(|| config.user_id.filter(|s| !s.is_empty()));
    let api_key = env_key.or_else(|| config.api_key.filter(|s| !s.is_empty()));

    match (user_id, api_key) {
        (Some(user_id), Some(api_key)) => Some(Credentials { user_id, api_key }),
        _ => None,
    }
}

pub fn dispatch(rest: &[String]) {
    if rest.is_empty() {
        print_service_usage();
        exit(1);
    }

    match rest[0].as_str() {
        "download" => run_download(&rest[1..]),
        "search" => run_search(&rest[1..]),
        "set" => config::run_set(&rest[1..]),
        "--complete-tags" => run_complete_tags(&rest[1..]),
        "-h" | "--help" => {
            print_service_usage();
            exit(0);
        }
        other => {
            eprintln!(
                "{}",
                trf("Unknown command for {}: {}\n", &[&"gelbooru.com", &other])
            );
            print_service_usage();
            exit(1);
        }
    }
}

fn print_service_usage() {
    eprintln!("{}", tr("Commands (gelbooru.com):"));
    eprintln!(
        "{}",
        tr("  nymphalis gelbooru.com download <dir> <tag1> [tag2] ...")
    );
    eprintln!(
        "{}",
        tr("      Download all images for the given tags into <dir>.")
    );
    eprintln!();
    eprintln!("{}", tr("  nymphalis gelbooru.com search <keyword>"));
    eprintln!("{}", tr("      Search for tags matching <keyword>."));
    eprintln!();
    eprintln!("{}", tr("  nymphalis gelbooru.com set user_id <value>"));
    eprintln!("{}", tr("  nymphalis gelbooru.com set api_key <value>"));
    eprintln!(
        "{}",
        trf(
            "      Save Gelbooru credentials to {}.",
            &[&config_path().display()]
        )
    );
}

fn warn_missing_credentials() {
    eprintln!(
        "{}",
        tr("Warning: GELBOORU_USER_ID / GELBOORU_API_KEY are not set")
    );
    eprintln!(
        "{}",
        trf(
            "(checked environment, .env, and {}).",
            &[&config_path().display()]
        )
    );
    eprintln!(
        "{}",
        tr("Gelbooru now rejects anonymous API requests with 401 Unauthorized.")
    );
    eprintln!(
        "{}",
        tr("Get your credentials at gelbooru.com -> My Account -> Options ->")
    );
    eprintln!("{}", tr("API Access Credentials, then either:"));
    eprintln!("{}", tr("  nymphalis set user_id 12345"));
    eprintln!("{}", tr("  nymphalis set api_key yourkey"));
    eprintln!("{}", tr("or export them as environment variables."));
    eprintln!();
}

fn explain_401() {
    eprintln!();
    eprintln!(
        "{}",
        tr("401 Unauthorized: Gelbooru requires API credentials for every")
    );
    eprintln!(
        "{}",
        tr("request (anonymous access was disabled in 2025). Set:")
    );
    eprintln!("{}", tr("  GELBOORU_USER_ID  and  GELBOORU_API_KEY"));
    eprintln!(
        "{}",
        tr("(get them at gelbooru.com -> My Account -> Options ->")
    );
    eprintln!("{}", tr("API Access Credentials) and try again."));
}

fn run_complete_tags(rest: &[String]) {
    let keyword = match rest.first() {
        Some(k) if !k.is_empty() => k,
        _ => return,
    };

    let pattern = format!("%{}%", keyword);
    let client = build_client();
    let creds = get_credentials();

    let mut query: Vec<(&str, &str)> = vec![
        ("page", "dapi"),
        ("s", "tag"),
        ("q", "index"),
        ("json", "1"),
        ("name_pattern", pattern.as_str()),
        ("limit", "20"),
        ("orderby", "count"),
    ];
    if let Some(c) = &creds {
        query.push(("user_id", c.user_id.as_str()));
        query.push(("api_key", c.api_key.as_str()));
    }

    let resp = match client.get(API_BASE).query(&query).send() {
        Ok(r) => r,
        Err(_) => return,
    };

    if !resp.status().is_success() {
        return;
    }

    let body = match resp.text() {
        Ok(t) => t,
        Err(_) => return,
    };

    let json: Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(_) => return,
    };

    for tag in extract_array(&json, "tag") {
        if let Some(name) = tag.get("name").and_then(Value::as_str) {
            println!("{}", name);
        }
    }
}

fn discover_posts(
    client: &reqwest::blocking::Client,
    creds: &Option<Credentials>,
    tags_query: &str,
    out_dir: &str,
) -> Option<(Vec<(String, String)>, u64)> {
    let mut to_download: Vec<(String, String)> = Vec::new();
    let already_have: u64 = 0;
    let mut pid: u32 = 0;

    loop {
        let pid_str = pid.to_string();
        let limit_str = PAGE_LIMIT.to_string();

        let mut query: Vec<(&str, &str)> = vec![
            ("page", "dapi"),
            ("s", "post"),
            ("q", "index"),
            ("json", "1"),
            ("tags", tags_query),
            ("limit", limit_str.as_str()),
            ("pid", pid_str.as_str()),
        ];
        if let Some(c) = creds {
            query.push(("user_id", c.user_id.as_str()));
            query.push(("api_key", c.api_key.as_str()));
        }

        let resp = match client.get(API_BASE).query(&query).send() {
            Ok(r) => r,
            Err(e) => {
                eprintln!(
                    "{}",
                    trf("API request error (page {}): {}", &[&pid, &e])
                );
                return None;
            }
        };

        let status = resp.status();
        if !status.is_success() {
            eprintln!(
                "{}",
                trf("API returned status {} on page {}", &[&status, &pid])
            );
            if status.as_u16() == 401 {
                explain_401();
            }
            return None;
        }

        let body = match resp.text() {
            Ok(t) => t,
            Err(e) => {
                eprintln!("{}", trf("Failed to read response: {}", &[&e]));
                return None;
            }
        };

        let json: Value = match serde_json::from_str(&body) {
            Ok(v) => v,
            Err(e) => {
                eprintln!(
                    "{}",
                    trf(
                        "Failed to parse JSON: {}\nServer response: {}",
                        &[&e, &body]
                    )
                );
                return None;
            }
        };

        let posts = extract_array(&json, "post");
        if posts.is_empty() {
            break;
        }

        for post in &posts {
            let file_url = post.get("file_url").and_then(Value::as_str).unwrap_or("");
            if file_url.is_empty() {
                continue;
            }

            let id = post
                .get("id")
                .map(|v| v.to_string())
                .unwrap_or_else(|| "unknown".to_string());

            let ext = file_url
                .rsplit('.')
                .next()
                .filter(|e| e.len() <= 5 && !e.contains('/'))
                .unwrap_or("jpg");

            let filename = format!("{}.{}", id, ext);
            to_download.push((filename, file_url.to_string()));
        }

        if (posts.len() as u32) < PAGE_LIMIT {
            break;
        }

        pid += 1;
    }

    Some((to_download, already_have))
}

fn parse_content_range_total(value: &str) -> Option<u64> {
    value.rsplit('/').next()?.parse::<u64>().ok()
}

fn head_size(client: &reqwest::blocking::Client, url: &str) -> Option<u64> {
    let resp = client
        .get(url)
        .header(reqwest::header::REFERER, "https://gelbooru.com/")
        .header(reqwest::header::RANGE, "bytes=0-0")
        .send()
        .ok()?;

    if resp.status() == reqwest::StatusCode::PARTIAL_CONTENT {
        if let Some(cr) = resp.headers().get(reqwest::header::CONTENT_RANGE) {
            if let Ok(cr) = cr.to_str() {
                if let Some(total) = parse_content_range_total(cr) {
                    return Some(total);
                }
            }
        }
    }

    if resp.status().is_success() {
        return resp.content_length();
    }

    None
}

fn check_sizes(
    client: &reqwest::blocking::Client,
    items: &[(String, String)],
    jobs: usize,
) -> u64 {
    let pb = ProgressBar::new(items.len() as u64);
    pb.set_style(ProgressStyle::with_template("{msg} {pos}/{len}").unwrap());
    pb.set_message(tr("Checking file sizes..."));

    let total = AtomicU64::new(0);
    let index = AtomicUsize::new(0);

    thread::scope(|scope| {
        for _ in 0..jobs {
            let pb = &pb;
            let total = &total;
            let index = &index;
            let client = &client;
            let items = &items;

            scope.spawn(move || loop {
                let i = index.fetch_add(1, Ordering::Relaxed);
                if i >= items.len() {
                    break;
                }
                let (_, url) = &items[i];
                if let Some(size) = head_size(client, url) {
                    total.fetch_add(size, Ordering::Relaxed);
                }
                pb.inc(1);
            });
        }
    });

    pb.finish_and_clear();
    total.load(Ordering::Relaxed)
}

fn confirm_download(count: usize, total_size: u64) -> bool {
    print!(
        "{}",
        trf(
            "About to download {} images (~{}). Continue? [y/N]: ",
            &[&count, &HumanBytes(total_size)]
        )
    );
    let _ = io::stdout().flush();

    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_err() {
        return false;
    }

    let input = input.trim().to_lowercase();
    input.starts_with('y') || input.starts_with('д')
}

fn run_download(rest: &[String]) {
    if rest.len() < 2 {
        eprintln!(
            "{}",
            tr("download requires a directory and at least one tag.\n")
        );
        print_service_usage();
        exit(1);
    }

    let out_dir = rest[0].clone();
    let tags: Vec<String> = rest[1..].to_vec();
    let tags_query = tags.join(" ");

    if let Err(e) = fs::create_dir_all(&out_dir) {
        eprintln!(
            "{}",
            trf("Failed to create directory '{}': {}", &[&out_dir, &e])
        );
        exit(1);
    }

    let client = build_client();
    let creds = get_credentials();
    if creds.is_none() {
        warn_missing_credentials();
    }

    println!("{}", trf("Tags: {}", &[&tags_query]));
    println!("{}", trf("Output directory: {}", &[&out_dir]));

    let (to_download, already_have) =
        match discover_posts(&client, &creds, &tags_query, &out_dir) {
            Some(v) => v,
            None => exit(1),
        };

    if to_download.is_empty() {
        if already_have > 0 {
            println!(
                "{}",
                tr("No new images to download, everything is already on disk.")
            );
        } else {
            println!("{}", tr("No images found for the given tags."));
        }
        return;
    }

    let jobs = parallel_jobs();
    let total_size = check_sizes(&client, &to_download, jobs);

    if !confirm_download(to_download.len(), total_size) {
        println!("{}", tr("Cancelled."));
        return;
    }

    println!("{}", trf("Parallel workers: {}", &[&jobs]));

    let display = crate::cli::DownloadDisplay::new(to_download.len() as u64, tr("Total"));
    let start_time = Instant::now();
    let total_bytes_downloaded = AtomicU64::new(0);
    let total_downloaded = AtomicU64::new(0);
    let queue =
        std::sync::Mutex::new(std::collections::VecDeque::from(to_download));

    thread::scope(|scope| {
        for _ in 0..jobs {
            let queue = &queue;
            let display = &display;
            let client = &client;
            let out_dir = &out_dir;
            let total_downloaded = &total_downloaded;
            let total_bytes_downloaded = &total_bytes_downloaded;

            scope.spawn(move || loop {
                let item = {
                    let mut q = queue.lock().unwrap();
                    q.pop_front()
                };
                let (filename, file_url) = match item {
                    Some(v) => v,
                    None => break,
                };

                let filepath = Path::new(out_dir).join(&filename);
                let handle = display.begin(crate::cli::ContentUnit::bytes(&filename));

                match download_file(client, &file_url, &filepath, &handle, "https://gelbooru.com/") {
                    Ok(size) => {
                        total_downloaded.fetch_add(1, Ordering::Relaxed);
                        total_bytes_downloaded.fetch_add(size, Ordering::Relaxed);
                        display.end_ok(handle, size);
                    }
                    Err(e) => {
                        display.end_err(handle, &e);
                    }
                }

                display.advance();
                let elapsed = start_time.elapsed().as_secs_f64().max(0.001);
                let bytes = total_bytes_downloaded.load(Ordering::Relaxed);
                let speed = (bytes as f64 / elapsed) as u64;
                display.set_msg(trf(
                    "{} ({}/s)",
                    &[&HumanBytes(bytes), &HumanBytes(speed)],
                ));

                sleep(DELAY_BETWEEN_DOWNLOADS);
            });
        }
    });

    display.finish(trf(
        "Done. Downloaded: {}, skipped (already existed): {}",
        &[&total_downloaded.load(Ordering::Relaxed), &already_have],
    ));

    println!(
        "{}",
        trf(
            "Done. Downloaded: {}, skipped (already existed): {}",
            &[
                &total_downloaded.load(Ordering::Relaxed),
                &already_have
            ],
        )
    );
}

fn run_search(rest: &[String]) {
    if rest.len() != 1 {
        eprintln!("{}", tr("search expects exactly one keyword.\n"));
        print_service_usage();
        exit(1);
    }

    let keyword = &rest[0];
    let pattern = format!("%{}%", keyword);

    let client = build_client();
    let creds = get_credentials();
    if creds.is_none() {
        warn_missing_credentials();
    }

    let mut query: Vec<(&str, &str)> = vec![
        ("page", "dapi"),
        ("s", "tag"),
        ("q", "index"),
        ("json", "1"),
        ("name_pattern", pattern.as_str()),
        ("limit", "100"),
        ("orderby", "count"),
    ];
    if let Some(c) = &creds {
        query.push(("user_id", c.user_id.as_str()));
        query.push(("api_key", c.api_key.as_str()));
    }

    let resp = match client.get(API_BASE).query(&query).send() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{}", trf("API request error: {}", &[&e]));
            exit(1);
        }
    };

    let status = resp.status();
    if !status.is_success() {
        eprintln!("{}", trf("API returned status {}", &[&status]));
        if status.as_u16() == 401 {
            explain_401();
        }
        exit(1);
    }

    let body = match resp.text() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("{}", trf("Failed to read response: {}", &[&e]));
            exit(1);
        }
    };

    let json: Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "{}",
                trf(
                    "Failed to parse JSON: {}\nServer response: {}",
                    &[&e, &body]
                )
            );
            exit(1);
        }
    };

    let mut tags = extract_array(&json, "tag");

    if tags.is_empty() {
        println!("{}", trf("No tags found matching '{}'.", &[&keyword]));
        return;
    }

    tags.sort_by(|a, b| {
        let ca = a.get("count").and_then(Value::as_i64).unwrap_or(0);
        let cb = b.get("count").and_then(Value::as_i64).unwrap_or(0);
        cb.cmp(&ca)
    });

    println!("{}", trf("Tags matching '{}':\n", &[&keyword]));
    println!("{:<40} {:>10}", tr("TAG"), tr("POSTS"));
    println!("{}", "-".repeat(52));

    for tag in &tags {
        let name = tag.get("name").and_then(Value::as_str).unwrap_or("?");
        let count = tag.get("count").and_then(Value::as_i64).unwrap_or(0);
        println!("{:<40} {:>10}", name, count);
    }

    println!(
        "\n{}",
        trf(
            "{} tags found. Use the exact tag name with: nymphalis gelbooru.com download <dir> <tag>",
            &[&tags.len()],
        )
    );
}

pub fn extract_array(json: &Value, key: &str) -> Vec<Value> {
    match json {
        Value::Array(arr) => arr.clone(),
        Value::Object(obj) => match obj.get(key) {
            Some(Value::Array(arr)) => arr.clone(),
            Some(single @ Value::Object(_)) => vec![single.clone()],
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

pub fn download_file(
    client: &reqwest::blocking::Client,
    url: &str,
    dest: &Path,
    handle: &crate::cli::ItemHandle,
    referer: &str,
) -> Result<u64, String> {
    let mut resp = client
        .get(url)
        .header(reqwest::header::REFERER, referer)
        .send()
        .map_err(|e| trf("request error: {}", &[&e]))?;

    if !resp.status().is_success() {
        return Err(trf("status {}", &[&resp.status()]));
    }

    match resp.content_length() {
        Some(len) if len > 0 => handle.set_length(len),
        _ => handle.switch_to_spinner(),
    }

    let tmp_path = dest.with_extension("part");
    let mut file =
        fs::File::create(&tmp_path).map_err(|e| trf("create file: {}", &[&e]))?;

    let mut buf = [0u8; 64 * 1024];
    let mut total: u64 = 0;
    loop {
        let n =
            std::io::Read::read(&mut resp, &mut buf).map_err(|e| trf("read: {}", &[&e]))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])
            .map_err(|e| trf("write: {}", &[&e]))?;
        total += n as u64;
        handle.set_position(total);
    }

    drop(file);
    let final_dest = crate::http::unique_dest(dest);
    fs::rename(&tmp_path, &final_dest).map_err(|e| trf("rename file: {}", &[&e]))?;

    Ok(total)
}

pub fn download_file_simple(
    client: &reqwest::blocking::Client,
    url: &str,
    dest: &PathBuf,
    referer: &str,
) -> Result<u64, String> {
    let mut resp = client
        .get(url)
        .header(reqwest::header::REFERER, referer)
        .send()
        .map_err(|e| trf("request error: {}", &[&e]))?;

    if !resp.status().is_success() {
        return Err(trf("status {}", &[&resp.status()]));
    }

    let tmp_path = dest.with_extension("part");
    let mut file =
        fs::File::create(&tmp_path).map_err(|e| trf("create file: {}", &[&e]))?;

    let mut buf = [0u8; 64 * 1024];
    let mut total: u64 = 0;
    loop {
        let n =
            std::io::Read::read(&mut resp, &mut buf).map_err(|e| trf("read: {}", &[&e]))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])
            .map_err(|e| trf("write: {}", &[&e]))?;
        total += n as u64;
    }

    drop(file);
    let final_dest = crate::http::unique_dest(dest);
    fs::rename(&tmp_path, &final_dest).map_err(|e| trf("rename file: {}", &[&e]))?;

    Ok(total)
}
