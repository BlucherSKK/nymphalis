use crate::config::{load_config, save_config};
use crate::language::{tr, trf};
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::blocking::Client;
use reqwest::header;
use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::exit;
use std::thread::sleep;
use std::time::Duration;

const API: &str = "https://www.patreon.com/api";
const REFERER: &str = "https://www.patreon.com/";
const UA: &str =
    "Mozilla/5.0 (X11; Linux x86_64; rv:128.0) Gecko/20100101 Firefox/128.0";

pub fn dispatch(rest: &[String]) {
    match rest.first().map(String::as_str) {
        Some("login")              => run_login(&rest[1..]),
        Some("show")               => run_show(),
        Some("download")           => run_download(&rest[1..]),
        Some("-h") | Some("--help") => { print_usage(); exit(0); }
        Some(other) => {
            eprintln!("{}", trf("Unknown command for patreon.com: {}\n", &[&other]));
            print_usage();
            exit(1);
        }
        None => {
            print_usage();
            exit(1);
        }
    }
}

fn print_usage() {
    eprintln!("{}", tr("Commands (patreon.com):"));
    eprintln!("{}", tr("  nymphalis patreon login [browser]"));
    eprintln!("{}", tr("      Open browser for Google login; session is read automatically."));
    eprintln!("{}", tr("      browser: firefox, chromium, chrome, brave, falkon (default: system default)"));
    eprintln!("{}", tr("  nymphalis patreon show"));
    eprintln!("{}", tr("      List all active subscriptions."));
    eprintln!("{}", tr("  nymphalis patreon download <dir> <creator1> [creator2 ...]"));
    eprintln!("{}", tr("      Download all media from the given creator(s) into <dir>/<creator>/."));
    eprintln!();
    eprintln!("{}", tr("If automatic cookie reading fails, set the session manually:"));
    eprintln!("{}", tr("  nymphalis set patreon_session session_id=<value>"));
}

// HTTP

fn build_client() -> Client {
    let proxy_url = std::env::var("PATREON_PROXY")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| load_config().patreon_proxy.filter(|s| !s.is_empty()));

    let mut builder = Client::builder().user_agent(UA);

    if let Some(url) = proxy_url {
        // socks5h роутит DNS через прокси — нужно для геоблока
        let effective = if url.starts_with("socks5://") {
            url.replacen("socks5://", "socks5h://", 1)
        } else {
            url.clone()
        };
        match reqwest::Proxy::all(&effective) {
            Ok(proxy) => {
                builder = builder.proxy(proxy);
                eprintln!("{}", trf("Using proxy: {}", &[&effective]));
            }
            Err(e) => {
                eprintln!("{}", trf("Invalid proxy URL '{}': {}", &[&effective, &e]));
                exit(1);
            }
        }
    }

    builder.build().unwrap_or_else(|e| {
        eprintln!("{}", trf("Failed to build HTTP client: {}", &[&e]));
        exit(1);
    })
}

fn load_session() -> String {
    load_config()
        .patreon_session
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            eprintln!("{}", tr("Not logged in to Patreon. Run: nymphalis patreon login"));
            exit(1);
        })
}

fn api_get(client: &Client, url: &str, session: &str) -> Result<Value, String> {
    let resp = client
        .get(url)
        .header(header::COOKIE, session)
        .header(header::REFERER, REFERER)
        .send()
        .map_err(|e| {
            use std::error::Error;
            let mut msg = e.to_string();
            let mut src = e.source();
            while let Some(s) = src {
                msg.push_str(&format!("\n  caused by: {}", s));
                src = s.source();
            }
            msg
        })?;

    let status = resp.status();
    if !status.is_success() {
        if status.as_u16() == 401 {
            return Err(tr("session expired or not logged in — run: nymphalis patreon login"));
        }
        return Err(trf("HTTP {}", &[&status.as_u16()]));
    }

    resp.json::<Value>().map_err(|e| trf("JSON parse error: {}", &[&e]))
}

// логин

const LOGIN_URL: &str = "https://www.patreon.com/login";
use crate::browser::{
    chromium_cookie_paths, falkon_cookie_paths, firefox_cookie_paths, open_browser,
    resolve_browser, sqlite3_query, Browser, BrowserKind,
};

fn run_login(rest: &[String]) {
    let browser: Option<Browser> = match rest.first() {
        None => None,
        Some(name) => match resolve_browser(name) {
            Some(b) => Some(b),
            None => {
                eprintln!("{}", trf("Unknown browser: '{}'. Supported: firefox, chromium, chrome, brave, falkon.", &[&name]));
                exit(1);
            }
        },
    };

    let label = browser.as_ref().map(|b| b.bin).unwrap_or("system default");
    println!("{}", trf("Opening {} for Patreon login...", &[&label]));
    println!("{}", trf("  URL: {}", &[&LOGIN_URL]));
    println!();
    println!("{}", tr("Click \"Continue with Google\" and complete the login."));
    println!("{}", tr("After you are redirected to your Patreon feed, press Enter here."));

    open_browser(LOGIN_URL, browser.as_ref());

    let _ = io::stdin().read_line(&mut String::new());

    println!("{}", tr("Reading session from browser cookie database..."));

    let session_value = match read_session(browser.as_ref()) {
        Some(v) => v,
        None => {
            eprintln!();
            eprintln!("{}", tr("Could not find the session_id cookie automatically."));
            eprintln!("{}", tr("Make sure the browser has fully finished loading the Patreon page."));
            eprintln!();
            eprintln!("{}", tr("Manual fallback:"));
            eprintln!("{}", tr("  1. Open DevTools on patreon.com (F12)"));
            eprintln!("{}", tr("  2. Application → Cookies → www.patreon.com → session_id → copy Value"));
            eprintln!("{}", tr("  3. nymphalis set patreon_session session_id=<value>"));
            exit(1);
        }
    };

    let cookie_str = format!("session_id={}", session_value);
    let client = build_client();
    match api_get(&client, &format!("{}/current_user", API), &cookie_str) {
        Ok(json) => {
            let name = json["data"]["attributes"]["full_name"]
                .as_str()
                .unwrap_or("(unknown)");
            println!("{}", trf("Logged in as: {}", &[&name]));
        }
        Err(e) => {
            eprintln!("{}", trf("Warning: session found but verification failed: {}", &[&e]));
            eprintln!("{}", tr("Saving anyway — run 'patreon show' to check."));
        }
    }

    let mut config = load_config();
    config.patreon_session = Some(cookie_str);
    save_config(&config);

    println!("{}", tr("Session saved."));
}

// читает куки нужного браузера, если не указан — перебирает все подряд
fn read_session(browser: Option<&Browser>) -> Option<String> {
    match browser {
        None => read_firefox_patreon_session()
            .or_else(|| read_chromium_patreon_session("chromium"))
            .or_else(|| read_chromium_patreon_session("google-chrome"))
            .or_else(|| read_chromium_patreon_session("brave-browser"))
            .or_else(|| read_falkon_patreon_session()),
        Some(b) => match &b.kind {
            BrowserKind::Firefox        => read_firefox_patreon_session(),
            BrowserKind::Chromium(cfg)  => read_chromium_patreon_session(cfg),
            BrowserKind::Falkon         => read_falkon_patreon_session(),
        },
    }
}

fn read_firefox_patreon_session() -> Option<String> {
    let sql = "SELECT value FROM moz_cookies \
               WHERE (host = 'www.patreon.com' OR host = '.patreon.com') \
               AND name = 'session_id' \
               ORDER BY lastAccessed DESC LIMIT 1";
    for db in firefox_cookie_paths() {
        if let Some(v) = sqlite3_query(&db, sql, "booru_patreon_cookie") {
            return Some(v);
        }
    }
    None
}

// Falkon хранит куки в том же формате что и Chromium
fn read_falkon_patreon_session() -> Option<String> {
    let sql = "SELECT value FROM cookies \
               WHERE (host_key = 'www.patreon.com' OR host_key = '.patreon.com') \
               AND name = 'session_id' \
               AND length(value) > 0 \
               ORDER BY last_access_utc DESC LIMIT 1";
    for db in falkon_cookie_paths() {
        if let Some(v) = sqlite3_query(&db, sql, "booru_patreon_cookie") {
            return Some(v);
        }
    }
    None
}

fn read_chromium_patreon_session(browser: &str) -> Option<String> {
    let sql = "SELECT value FROM cookies \
               WHERE (host_key = 'www.patreon.com' OR host_key = '.patreon.com') \
               AND name = 'session_id' \
               AND length(value) > 0 \
               ORDER BY last_access_utc DESC LIMIT 1";
    for db in chromium_cookie_paths(browser) {
        if let Some(v) = sqlite3_query(&db, sql, "booru_patreon_cookie") {
            return Some(v);
        }
    }
    None
}

// показ

fn run_show() {
    let session = load_session();
    let client = build_client();

    let url = format!(
        "{}/current_user\
         ?include=pledges.creator\
         &fields[pledge]=amount_cents,currency,status\
         &fields[user]=full_name,vanity,url",
        API
    );

    let json = match api_get(&client, &url, &session) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("{}", trf("Failed to fetch subscriptions: {}", &[&e]));
            exit(1);
        }
    };

    let pledge_refs = match json["data"]["relationships"]["pledges"]["data"].as_array() {
        Some(a) if !a.is_empty() => a.clone(),
        _ => {
            println!("{}", tr("No active subscriptions found."));
            return;
        }
    };

    let included = json["included"].as_array().cloned().unwrap_or_default();

    println!("{}", tr("\nActive Patreon subscriptions:\n"));
    println!("{:<40} {:<25} {}", tr("CREATOR"), tr("VANITY"), tr("AMOUNT"));
    println!("{}", "-".repeat(80));

    let mut count = 0usize;

    for pledge_ref in &pledge_refs {
        let pledge_id = pledge_ref["id"].as_str().unwrap_or("");

        let pledge = match included
            .iter()
            .find(|v| v["type"].as_str() == Some("pledge") && v["id"].as_str() == Some(pledge_id))
        {
            Some(p) => p,
            None => continue,
        };

        let status = pledge["attributes"]["status"].as_str().unwrap_or("unknown");
        if status == "declined" {
            continue;
        }

        let amount_cents = pledge["attributes"]["amount_cents"].as_i64().unwrap_or(0);
        let currency = pledge["attributes"]["currency"].as_str().unwrap_or("USD");
        let amount_str = format!("{:.2} {}/mo", amount_cents as f64 / 100.0, currency);

        let creator_id = pledge["relationships"]["creator"]["data"]["id"]
            .as_str()
            .unwrap_or("");

        let creator = included.iter().find(|v| {
            v["type"].as_str() == Some("user") && v["id"].as_str() == Some(creator_id)
        });

        if let Some(c) = creator {
            let name = c["attributes"]["full_name"].as_str().unwrap_or("?");
            let vanity = c["attributes"]["vanity"].as_str().unwrap_or("?");
            println!("{:<40} {:<25} {}", name, vanity, amount_str);
            count += 1;
        }
    }

    println!("{}", trf("\nTotal: {} subscription(s).", &[&count]));
    if count > 0 {
        println!("{}", tr("Use the VANITY name with: nymphalis patreon download <dir> <vanity>"));
    }
}

// скачивание

const DOWNLOAD_DELAY: Duration = Duration::from_millis(1000);

fn run_download(rest: &[String]) {
    if rest.len() < 2 {
        eprintln!("{}", tr("download requires <dir> and at least one creator name."));
        print_usage();
        exit(1);
    }

    let out_dir = &rest[0];
    let creators = &rest[1..];
    let session = load_session();

    if let Err(e) = fs::create_dir_all(out_dir) {
        eprintln!("{}", trf("Failed to create directory '{}': {}", &[&out_dir, &e]));
        exit(1);
    }

    let client = build_client();

    for creator in creators {
        download_creator(&client, &session, out_dir, creator);
    }
}

fn find_campaign_id(client: &Client, session: &str, vanity: &str) -> Option<String> {
    let encoded = percent_encode(vanity);
    let url = format!("{}/campaigns?filter[vanity]={}", API, encoded);
    match api_get(client, &url, session) {
        Ok(json) => json["data"]
            .as_array()
            .and_then(|a| a.first())
            .and_then(|c| c["id"].as_str())
            .map(String::from),
        Err(e) => {
            eprintln!("{}", trf("  Campaign lookup failed for '{}': {}", &[&vanity, &e]));
            None
        }
    }
}

fn download_creator(client: &Client, session: &str, base_dir: &str, creator: &str) {
    println!("{}", trf("\nFetching campaign for '{}'...", &[&creator]));

    let campaign_id = match find_campaign_id(client, session, creator) {
        Some(id) => id,
        None => {
            eprintln!("{}", trf(
                "  Creator '{}' not found. Make sure you are subscribed and the name matches patreon.com/{}",
                &[&creator, &creator],
            ));
            return;
        }
    };

    let creator_dir = PathBuf::from(base_dir);
    if let Err(e) = fs::create_dir_all(&creator_dir) {
        eprintln!("{}", trf("Cannot create {}: {}", &[&creator_dir.display(), &e]));
        return;
    }

    let (to_download, already_have) = scan_posts(client, session, &campaign_id, &creator_dir);

    if to_download.is_empty() {
        if already_have > 0 {
            println!("{}", trf("  Nothing new for '{}' — {} file(s) already on disk.", &[&creator, &already_have]));
        } else {
            println!("{}", trf("  No accessible media found for '{}'.", &[&creator]));
        }
        return;
    }

    println!("{}", trf("  {} new file(s) to download, {} already on disk.", &[&to_download.len(), &already_have]));

    if !confirm_download(to_download.len()) {
        println!("{}", tr("Cancelled."));
        return;
    }

    let display = crate::cli::DownloadDisplay::new(
        to_download.len() as u64,
        creator.chars().take(22).collect::<String>(),
    );

    let mut n_ok = 0usize;
    let mut n_err = 0usize;

    for (filename, url) in &to_download {
        let dest = creator_dir.join(filename);

        display.set_msg(filename.chars().take(30).collect::<String>());
        let handle = display.begin(crate::cli::ContentUnit::bytes(filename.as_str()));

        match download_file(client, session, url, &dest, &handle) {
            Ok(bytes) => { display.end_ok(handle, bytes);  n_ok += 1; }
            Err(e)    => { display.end_err(handle, &e);    n_err += 1; }
        }

        display.advance();
        sleep(DOWNLOAD_DELAY);
    }

    display.finish(tr("done"));

    println!("{}", trf(
        "Done for '{}': {} downloaded, {} failed, {} skipped (already on disk).",
        &[&creator, &n_ok, &n_err, &already_have],
    ));
}

fn confirm_download(count: usize) -> bool {
    print!("{}", trf("About to download {} file(s). Continue? [y/N]: ", &[&count]));
    let _ = io::stdout().flush();
    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_err() {
        return false;
    }
    let input = input.trim().to_lowercase();
    input.starts_with('y') || input.starts_with('д')
}

// сканирование

// листает все посты, собирает что качать
fn scan_posts(
    client: &Client,
    session: &str,
    campaign_id: &str,
    creator_dir: &PathBuf,
) -> (Vec<(String, String)>, usize) {
    let spin = ProgressBar::new_spinner();
    spin.set_style(
        ProgressStyle::with_template("{spinner:.cyan} {msg}")
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );

    let mut to_download: Vec<(String, String)> = Vec::new();
    let mut already_have = 0usize;
    let mut seen: HashSet<String> = HashSet::new();
    let mut cursor: Option<String> = None;
    let mut page = 0usize;

    loop {
        page += 1;
        spin.set_message(trf("Scanning page {}…  {} new / {} on disk",
            &[&page, &to_download.len(), &already_have]));
        spin.tick();

        let mut url = format!(
            "{}/posts\
             ?filter[campaign_id]={}\
             &filter[is_draft]=false\
             &include=images,attachments,audio,media\
             &fields[post]=title\
             &fields[media]=file_name,image_urls,download_url\
             &fields[attachment]=name,url\
             &fields[audio]=download_url\
             &page[count]=12\
             &sort=-published_at",
            API, campaign_id
        );
        if let Some(ref c) = cursor {
            url.push_str(&format!("&page[cursor]={}", c));
        }

        let json = match api_get(client, &url, session) {
            Ok(j) => j,
            Err(e) => {
                spin.finish_and_clear();
                eprintln!("{}", trf("  Scan failed on page {}: {}", &[&page, &e]));
                break;
            }
        };

        let posts = match json["data"].as_array() {
            Some(p) if !p.is_empty() => p.clone(),
            _ => break,
        };

        let included = json["included"].as_array().cloned().unwrap_or_default();

        for post in &posts {
            collect_post_items(post, &included, creator_dir, &mut to_download, &mut already_have, &mut seen);
        }

        match json["meta"]["pagination"]["cursors"]["next"].as_str() {
            Some(c) => cursor = Some(c.to_string()),
            None => break,
        }
    }

    spin.finish_and_clear();
    (to_download, already_have)
}

// собирает медиафайлы из одного поста
fn collect_post_items(
    post: &Value,
    included: &[Value],
    dir: &PathBuf,
    to_download: &mut Vec<(String, String)>,
    already_have: &mut usize,
    seen: &mut HashSet<String>,
) {
    for id in rel_ids(post, "images").iter().chain(rel_ids(post, "media").iter()) {
        if let Some(m) = included.iter().find(|v| {
            matches!(v["type"].as_str(), Some("media") | Some("post_image"))
                && v["id"].as_str() == Some(id)
        }) {
            let name = m["attributes"]["file_name"].as_str().unwrap_or("file");
            let url = m["attributes"]["download_url"]
                .as_str()
                .or_else(|| m["attributes"]["image_urls"]["original"].as_str())
                .or_else(|| m["attributes"]["image_urls"]["default"].as_str())
                .unwrap_or("");
            push_item(name, url, dir, to_download, already_have, seen);
        }
    }

    for id in rel_ids(post, "attachments") {
        if let Some(a) = included.iter().find(|v| {
            v["type"].as_str() == Some("attachment") && v["id"].as_str() == Some(&id)
        }) {
            let name = a["attributes"]["name"].as_str().unwrap_or("attachment");
            let url  = a["attributes"]["url"].as_str().unwrap_or("");
            push_item(name, url, dir, to_download, already_have, seen);
        }
    }

    for id in rel_ids(post, "audio") {
        if let Some(a) = included.iter().find(|v| {
            v["type"].as_str() == Some("audio") && v["id"].as_str() == Some(&id)
        }) {
            let url = a["attributes"]["download_url"].as_str().unwrap_or("");
            if url.is_empty() { continue; }
            let name = url.rsplit('/').next()
                .and_then(|n| n.split('?').next())
                .unwrap_or("audio.mp3");
            push_item(name, url, dir, to_download, already_have, seen);
        }
    }
}

fn push_item(
    filename: &str,
    url: &str,
    dir: &PathBuf,
    to_download: &mut Vec<(String, String)>,
    already_have: &mut usize,
    seen: &mut HashSet<String>,
) {
    if url.is_empty() { return; }
    let safe = sanitize_filename(filename);
    if safe.is_empty() || !seen.insert(safe.clone()) { return; }

    let dest = dir.join(&safe);
    if dest.exists() {
        *already_have += 1;
        return;
    }

    to_download.push((safe, url.to_string()));
}

fn sanitize_filename(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '.' | '-' | '_' | '(' | ')' | '[' | ']' | ' ') {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

// хелперы

fn rel_ids(post: &Value, rel: &str) -> Vec<String> {
    post["relationships"][rel]["data"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v["id"].as_str().map(String::from)).collect())
        .unwrap_or_default()
}

fn download_file(
    client: &Client,
    session: &str,
    url: &str,
    dest: &PathBuf,
    handle: &crate::cli::ItemHandle,
) -> Result<u64, String> {
    let mut resp = client
        .get(url)
        .header(header::COOKIE, session)
        .header(header::REFERER, REFERER)
        .send()
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status().as_u16()));
    }

    match resp.content_length() {
        Some(len) if len > 0 => handle.set_length(len),
        _                    => handle.switch_to_spinner(),
    }

    let tmp = dest.with_extension("part");
    let mut file = fs::File::create(&tmp).map_err(|e| format!("create: {}", e))?;

    let mut buf = [0u8; 64 * 1024];
    let mut written = 0u64;
    loop {
        let n = io::Read::read(&mut resp, &mut buf).map_err(|e| format!("read: {}", e))?;
        if n == 0 { break; }
        file.write_all(&buf[..n]).map_err(|e| format!("write: {}", e))?;
        written += n as u64;
        handle.set_position(written);
    }

    drop(file);
    let final_dest = crate::http::unique_dest(dest);
    fs::rename(&tmp, &final_dest).map_err(|e| format!("rename: {}", e))?;
    Ok(written)
}

fn percent_encode(s: &str) -> String {
    percent_encoding::utf8_percent_encode(s, percent_encoding::NON_ALPHANUMERIC).to_string()
}
