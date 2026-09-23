mod epub;

use crate::config::{load_config, save_config};
use crate::gelbooru::download_file_simple;
use crate::language::{tr, trf};
use regex::Regex;
use serde::Deserialize;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::exit;

const BASE: &str = "https://desu.uno";
const REFERER: &str = "https://desu.uno/";
const LOGIN_URL: &str = "https://desu.uno/login/";

fn session_cookie() -> Option<String> {
    load_config()
        .desu_session
        .filter(|s| !s.is_empty())
}

pub fn dispatch(rest: &[String]) {
    if rest.is_empty() {
        print_service_usage();
        exit(1);
    }

    match rest[0].as_str() {
        "download" => run_download(&rest[1..]),
        "search"   => run_search(&rest[1..]),
        "login"    => run_login(&rest[1..]),
        "-h" | "--help" => {
            print_service_usage();
            exit(0);
        }
        other => {
            eprintln!(
                "{}",
                trf("Unknown command for {}: {}\n", &[&"desu.uno", &other])
            );
            print_service_usage();
            exit(1);
        }
    }
}

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
    println!("{}", trf("Opening {} for desu.uno login...", &[&label]));
    println!("{}", trf("  URL: {}", &[&LOGIN_URL]));
    println!();
    println!("{}", tr("Log in with your account, then press Enter here."));

    open_browser(LOGIN_URL, browser.as_ref());

    let _ = io::stdin().read_line(&mut String::new());

    println!("{}", tr("Reading session from browser cookie database..."));

    let client = build_client();

    let (cookie_str, username) = match browser.as_ref() {
        Some(b) => {
            let cookie = match read_cookie_for_browser(b) {
                Some(c) => c,
                None => {
                    eprintln!("{}", trf("Could not find xf_session cookie in {}.", &[&b.bin]));
                    eprintln!("{}", tr("Make sure you completed the login and the page fully loaded."));
                    exit(1);
                }
            };
            match verify_session(&client, &cookie) {
                Some(u) => (cookie, u),
                None => {
                    eprintln!();
                    eprintln!("{}", tr("Session found but verification failed — the cookie may be stale."));
                    eprintln!("{}", tr("Make sure you actually completed the login in the browser."));
                    exit(1);
                }
            }
        }
        None => {
            match find_valid_session(&client) {
                Some(v) => v,
                None => {
                    eprintln!();
                    eprintln!("{}", tr("Could not find a valid session in any browser."));
                    eprintln!("{}", tr("Make sure you are logged in on desu.uno in your browser."));
                    eprintln!();
                    eprintln!("{}", tr("Or specify the browser: nymphalis desu login <browser>"));
                    eprintln!("{}", tr("Supported: firefox, chromium, chrome, brave, falkon"));
                    eprintln!();
                    eprintln!("{}", tr("Manual fallback:"));
                    eprintln!("{}", tr("  1. Open DevTools on desu.uno (F12)"));
                    eprintln!("{}", tr("  2. Application → Cookies → desu.uno → xf_session → copy Value"));
                    eprintln!("{}", tr("  3. nymphalis set desu_session xf_session=<value>"));
                    exit(1);
                }
            }
        }
    };

    println!("{}", trf("Logged in as: {}", &[&username]));
    save_session(cookie_str);
}

// читает куки указанного браузера
fn read_cookie_for_browser(b: &Browser) -> Option<String> {
    match &b.kind {
        BrowserKind::Firefox       => read_firefox_desu_session(),
        BrowserKind::Chromium(cfg) => read_chromium_desu_session(cfg),
        BrowserKind::Falkon        => read_falkon_desu_session(),
    }
}

// перебирает браузеры, возвращает первую рабочую сессию
fn find_valid_session(client: &reqwest::blocking::Client) -> Option<(String, String)> {
    let candidates: &[(&str, Box<dyn Fn() -> Option<String>>)] = &[
        ("firefox",      Box::new(read_firefox_desu_session)),
        ("chromium",     Box::new(|| read_chromium_desu_session("chromium"))),
        ("google-chrome",Box::new(|| read_chromium_desu_session("google-chrome"))),
        ("brave-browser",Box::new(|| read_chromium_desu_session("brave-browser"))),
        ("falkon",       Box::new(read_falkon_desu_session)),
    ];

    for (name, read_fn) in candidates {
        if let Some(cookie) = read_fn() {
            if let Some(username) = verify_session(client, &cookie) {
                eprintln!("{}", trf("  (found valid session in {})", &[&name]));
                return Some((cookie, username));
            }
        }
    }
    None
}

// проверяет сессию, возвращает юзернейм или None
fn verify_session(client: &reqwest::blocking::Client, cookie: &str) -> Option<String> {
    let resp = client
        .get(BASE)
        .header(reqwest::header::COOKIE, cookie)
        .header(reqwest::header::REFERER, REFERER)
        .send()
        .ok()?;

    let body = resp.text().ok()?;

    if !body.contains("LoggedIn") {
        return None;
    }

    let username = extract_xenforo_username(&body).unwrap_or_else(|| "(authenticated)".to_string());
    Some(username)
}

fn extract_xenforo_username(html: &str) -> Option<String> {
    // ищем ссылку аккаунта — там обычно юзернейм
    for marker in &["p-navgroup-link--user", "class=\"username\"", "AccountMenu"] {
        if let Some(pos) = html.find(marker) {
            let window = &html[pos..std::cmp::min(html.len(), pos + 300)];
            if let Some(a_pos) = window.find('>') {
                let after = &window[a_pos + 1..];
                if let Some(end) = after.find('<') {
                    let name = after[..end].trim();
                    if !name.is_empty() && !name.contains('{') {
                        return Some(name.to_string());
                    }
                }
            }
        }
    }
    None
}

fn read_firefox_desu_session() -> Option<String> {
    let sql = "SELECT GROUP_CONCAT(name || '=' || value, '; ') \
               FROM (SELECT name, value FROM moz_cookies \
                     WHERE (host = 'desu.uno' OR host = '.desu.uno') \
                     AND name LIKE 'xf_%' AND length(value) > 0 \
                     GROUP BY name ORDER BY lastAccessed DESC)";
    for db in firefox_cookie_paths() {
        if let Some(v) = sqlite3_query(&db, sql, "booru_desu_cookie") {
            return Some(v);
        }
    }
    None
}

fn read_chromium_desu_session(browser: &str) -> Option<String> {
    let sql = "SELECT GROUP_CONCAT(name || '=' || value, '; ') \
               FROM (SELECT name, value FROM cookies \
                     WHERE (host_key = 'desu.uno' OR host_key = '.desu.uno') \
                     AND name LIKE 'xf_%' AND length(value) > 0 \
                     GROUP BY name ORDER BY last_access_utc DESC)";
    for db in chromium_cookie_paths(browser) {
        if let Some(v) = sqlite3_query(&db, sql, "booru_desu_cookie") {
            return Some(v);
        }
    }
    None
}

fn read_falkon_desu_session() -> Option<String> {
    let sql = "SELECT GROUP_CONCAT(name || '=' || value, '; ') \
               FROM (SELECT name, value FROM cookies \
                     WHERE (host_key = 'desu.uno' OR host_key = '.desu.uno') \
                     AND name LIKE 'xf_%' AND length(value) > 0 \
                     GROUP BY name ORDER BY last_access_utc DESC)";
    for db in falkon_cookie_paths() {
        if let Some(v) = sqlite3_query(&db, sql, "booru_desu_cookie") {
            return Some(v);
        }
    }
    None
}

fn save_session(value: String) {
    let mut config = load_config();
    config.desu_session = Some(value);
    save_config(&config);
    println!("{}", tr("Session saved. Restricted content is now accessible."));
}

// справка

fn print_service_usage() {
    eprintln!("{}", tr("Commands (desu.uno):"));
    eprintln!(
        "{}",
        tr("  nymphalis desu.uno download [manga|ranobe] <dir> <slug.id> [slug.id2] ...")
    );
    eprintln!(
        "{}",
        tr("      Download all chapters of the given title(s) into <dir> (default: manga).")
    );
    eprintln!();
    eprintln!("{}", tr("  nymphalis desu.uno search <keyword>"));
    eprintln!(
        "{}",
        tr("      Search for manga. Output: Human Title | slug.id")
    );
    eprintln!();
    eprintln!("{}", tr("  nymphalis desu.uno login [browser]"));
    eprintln!("{}", tr("      Open browser login page and save xf_session cookie automatically."));
    eprintln!("{}", tr("      Supported browsers: firefox, chromium, chrome, brave, falkon."));
}

// HTTP клиент

fn build_client() -> reqwest::blocking::Client {
    let proxy_url = std::env::var("DESU_PROXY").ok()
        .filter(|s| !s.is_empty())
        .or_else(|| load_config().desu_proxy.filter(|s| !s.is_empty()));

    let mut builder = reqwest::blocking::Client::builder()
        .user_agent("Mozilla/5.0 (X11; Linux x86_64; rv:128.0) Gecko/20100101 Firefox/128.0");

    if let Some(url) = proxy_url {
        let effective = if url.starts_with("socks5://") {
            url.replacen("socks5://", "socks5h://", 1)
        } else {
            url.clone()
        };
        match reqwest::Proxy::all(&effective) {
            Ok(proxy) => { builder = builder.proxy(proxy); eprintln!("Using proxy: {}", effective); }
            Err(e)    => { eprintln!("Invalid proxy URL '{}': {}", effective, e); std::process::exit(1); }
        }
    }

    builder.build().unwrap_or_else(|e| {
        eprintln!("Failed to build HTTP client: {}", e);
        std::process::exit(1);
    })
}

// HTTP хелперы

fn status_error(status: reqwest::StatusCode) -> String {
    match status.as_u16() {
        451 => tr("HTTP 451 — title blocked by Roskomnadzor (149-FZ). Use 'search' to find an available slug.id."),
        403 => tr("HTTP 403 — access denied (login may be required)."),
        404 => tr("HTTP 404 — title not found. Check slug.id via 'search'."),
        _ => trf("HTTP {}", &[&status]),
    }
}

#[allow(dead_code)]
fn xhr_get(client: &reqwest::blocking::Client, url: &str) -> Result<String, String> {
    xhr_get_with_url(client, url).map(|(body, _)| body)
}

// то же что xhr_get, но ещё возвращает финальный URL
#[allow(dead_code)]
fn xhr_get_with_url(
    client: &reqwest::blocking::Client,
    url: &str,
) -> Result<(String, String), String> {
    let mut req = client
        .get(url)
        .header(reqwest::header::REFERER, REFERER)
        .header("X-Requested-With", "XMLHttpRequest")
        .query(&[("_xfResponseType", "json")]);
    if let Some(cookie) = session_cookie() {
        req = req.header(reqwest::header::COOKIE, cookie);
    }
    let resp = req.send().map_err(|e| trf("API request error: {}", &[&e]))?;

    if !resp.status().is_success() {
        return Err(status_error(resp.status()));
    }

    let final_url = resp.url().to_string();
    let body = resp
        .text()
        .map_err(|e| trf("Failed to read response: {}", &[&e]))?;
    Ok((body, final_url))
}

fn plain_get(client: &reqwest::blocking::Client, url: &str) -> Result<String, String> {
    plain_get_with_url(client, url).map(|(body, _)| body)
}

fn plain_get_with_url(
    client: &reqwest::blocking::Client,
    url: &str,
) -> Result<(String, String), String> {
    let mut req = client
        .get(url)
        .header(reqwest::header::REFERER, REFERER);
    if let Some(cookie) = session_cookie() {
        req = req.header(reqwest::header::COOKIE, cookie);
    }
    let resp = req.send().map_err(|e| trf("API request error: {}", &[&e]))?;

    if !resp.status().is_success() {
        return Err(status_error(resp.status()));
    }

    let final_url = resp.url().to_string();
    let body = resp.text().map_err(|e| trf("Failed to read response: {}", &[&e]))?;
    Ok((body, final_url))
}

fn template_html_full(body: &str) -> Result<String, String> {
    let full: serde_json::Value = serde_json::from_str(body)
        .map_err(|e| trf("Failed to parse JSON: {}\nServer response: {}", &[&e, &body]))?;
    full["templateHtml"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "templateHtml missing in response".to_string())
}

// парсеры

// парсит slug.id или просто id
fn parse_slug_id(input: &str) -> Option<(String, u64)> {
    if let Some(pos) = input.rfind('.') {
        let after = &input[pos + 1..];
        if let Ok(id) = after.parse::<u64>() {
            return Some((input[..pos].to_string(), id));
        }
    }
    if let Ok(id) = input.parse::<u64>() {
        return Some((id.to_string(), id));
    }
    None
}

#[derive(Debug, Clone)]
struct DesuChapter {
    id: Option<u64>,
    path: String,
}

#[derive(Debug, Deserialize)]
struct ChapterApiResponse {
    chapter: Option<ChapterApiData>,
    errors: Option<Vec<ApiErrorItem>>,
    error: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct ChapterApiData {
    pages: Option<Vec<ChapterApiPage>>,
}

#[derive(Debug, Deserialize)]
struct ChapterApiPage {
    url: String,
}

#[derive(Debug, Deserialize, Clone)]
struct ApiErrorItem {
    message: Option<String>,
}

fn fetch_chapter_pages_api(
    client: &reqwest::blocking::Client,
    manga_id: u64,
    chapter_id: u64,
) -> Result<Vec<String>, String> {
    let url = format!("{}/api/manga/{}/chapters/{}", BASE, manga_id, chapter_id);
    let body = plain_get(client, &url)?;
    let resp: ChapterApiResponse = serde_json::from_str(&body)
        .map_err(|e| trf("Failed to parse chapter API JSON: {}", &[&e]))?;

    if let Some(chapter) = resp.chapter {
        if let Some(pages) = chapter.pages {
            let urls: Vec<String> = pages.into_iter().map(|p| p.url).collect();
            if !urls.is_empty() {
                return Ok(urls);
            }
        }
    }

    if let Some(errors) = resp.errors {
        for err in errors {
            if let Some(msg) = err.message {
                return Err(msg);
            }
        }
    }

    if let Some(errors) = resp.error {
        if let Some(msg) = errors.into_iter().next() {
            return Err(msg);
        }
    }

    Err(tr("No pages found in chapter API response."))
}

#[derive(Debug, Deserialize, Clone)]
struct RanobeChaptersResponse {
    chapters: Option<Vec<RanobeChapterItem>>,
    errors: Option<Vec<ApiErrorItem>>,
    error: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Clone)]
struct RanobeChapterItem {
    id: u64,
    #[serde(default)]
    volume: serde_json::Value,
    #[serde(default)]
    number: serde_json::Value,
    title: Option<String>,
    #[serde(rename = "type")]
    #[allow(dead_code)]
    chapter_type: Option<String>,
    view_url: Option<String>,
}

impl RanobeChapterItem {
    fn volume_str(&self) -> String {
        match &self.volume {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            _ => String::new(),
        }
    }

    fn number_str(&self) -> String {
        match &self.number {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            _ => String::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RanobeChapterDetailResponse {
    chapter: Option<RanobeChapterDetail>,
    errors: Option<Vec<ApiErrorItem>>,
    error: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct RanobeChapterDetail {
    #[allow(dead_code)]
    id: u64,
    #[serde(default)]
    #[allow(dead_code)]
    volume: serde_json::Value,
    #[serde(default)]
    #[allow(dead_code)]
    number: serde_json::Value,
    title: Option<String>,
    content: Option<Vec<RanobeChapterContentItem>>,
}

#[derive(Debug, Deserialize)]
struct RanobeChapterContentItem {
    #[serde(rename = "type")]
    item_type: String,
    html: Option<String>,
    url: Option<String>,
    #[allow(dead_code)]
    media_type: Option<String>,
    position: Option<u32>,
}

fn fetch_ranobe_chapters_api(
    client: &reqwest::blocking::Client,
    ranobe_id: u64,
) -> Result<Vec<RanobeChapterItem>, String> {
    let url = format!("{}/api/ranobe/{}/chapters", BASE, ranobe_id);
    let body = plain_get(client, &url)?;
    let resp: RanobeChaptersResponse = serde_json::from_str(&body)
        .map_err(|e| trf("Failed to parse ranobe chapters JSON: {}", &[&e]))?;

    if let Some(chapters) = resp.chapters {
        return Ok(chapters);
    }

    if let Some(errors) = resp.errors {
        for err in errors {
            if let Some(msg) = err.message {
                return Err(msg);
            }
        }
    }

    if let Some(errors) = resp.error {
        if let Some(msg) = errors.into_iter().next() {
            return Err(msg);
        }
    }

    Err(tr("No chapters found in ranobe chapters API response."))
}

fn fetch_ranobe_chapter_detail_api(
    client: &reqwest::blocking::Client,
    ranobe_id: u64,
    chapter_id: u64,
) -> Result<RanobeChapterDetail, String> {
    let url = format!("{}/api/ranobe/{}/chapters/{}", BASE, ranobe_id, chapter_id);
    let body = plain_get(client, &url)?;
    let resp: RanobeChapterDetailResponse = serde_json::from_str(&body)
        .map_err(|e| trf("Failed to parse ranobe chapter content JSON: {}", &[&e]))?;

    if let Some(chapter) = resp.chapter {
        return Ok(chapter);
    }

    if let Some(errors) = resp.errors {
        for err in errors {
            if let Some(msg) = err.message {
                return Err(msg);
            }
        }
    }

    if let Some(errors) = resp.error {
        if let Some(msg) = errors.into_iter().next() {
            return Err(msg);
        }
    }

    Err(tr("No chapter data in ranobe chapter content response."))
}

fn parse_chapter_id_from_reader_html(html: &str) -> Option<u64> {
    let re = Regex::new(r#""chapter"\s*:\s*\{[^}]*"id"\s*:\s*(\d+)"#).ok()?;
    let cap = re.captures(html)?;
    cap.get(1)?.as_str().parse().ok()
}

fn parse_manga_id_from_chlist(html: &str) -> Option<u64> {
    let re = Regex::new(r#"data-manga_id=["'](\d+)["']"#).ok()?;
    let cap = re.captures(html)?;
    cap.get(1)?.as_str().parse().ok()
}

// собирает ссылки на главы и id (из кнопок скачивания chDownload)
fn parse_chapters(html: &str, slug_id: &str) -> Vec<DesuChapter> {
    let re_chlist = Regex::new(r#"(?s)<ul[^>]*\bclass=["'][^"']*chlist[^"']*["'][^>]*>(.*?)</ul>"#).ok();
    let re_li = Regex::new(r"(?s)<li[^>]*>(.*?)</li>").ok();
    let re_cid = Regex::new(r#"data-chapters_id=["'](\d+)["']"#).ok();
    let re_href = Regex::new(r#"href=["']([^"']+)["']"#).ok();

    let mut chapters = Vec::new();
    let mut seen_paths = std::collections::HashSet::new();

    let search_scope = if let Some(ref r_chlist) = re_chlist {
        if let Some(cap) = r_chlist.captures(html) {
            cap.get(1).map(|m| m.as_str()).unwrap_or(html)
        } else {
            html
        }
    } else {
        html
    };

    if let (Some(r_li), Some(r_cid), Some(r_href)) = (re_li, re_cid, re_href) {
        for cap in r_li.captures_iter(search_scope) {
            let li_text = &cap[1];
            let cid = r_cid
                .captures(li_text)
                .and_then(|c| c.get(1))
                .and_then(|m| m.as_str().parse::<u64>().ok());

            if let Some(href_cap) = r_href.captures(li_text).and_then(|c| c.get(1)) {
                let href = href_cap.as_str().trim_end_matches('/');
                if href.contains("/vol") || href.contains("/ch") || href.ends_with("/rus") {
                    let rel = href
                        .find("/manga/")
                        .map(|i| &href[i + 1..])
                        .unwrap_or(href)
                        .trim_start_matches('/')
                        .to_string();

                    if seen_paths.insert(rel.clone()) {
                        chapters.push(DesuChapter { id: cid, path: rel });
                    }
                }
            }
        }
    }

    if chapters.is_empty() {
        for url in parse_chapter_urls(html, slug_id) {
            chapters.push(DesuChapter { id: None, path: url });
        }
    }

    chapters.sort_by(|a, b| chapter_sort_key(&a.path).cmp(&chapter_sort_key(&b.path)));
    chapters
}

// собирает ссылки на главы из HTML (fallback)
fn parse_chapter_urls(html: &str, slug_id: &str) -> Vec<String> {
    let needle     = format!("/manga/{}/vol", slug_id.trim_end_matches('/'));
    let needle_rel = format!("manga/{}/vol",  slug_id.trim_end_matches('/'));
    let mut seen = std::collections::HashSet::new();
    let mut chapters = Vec::new();

    let mut pos = 0;
    while let Some(href_start) = html[pos..].find("href=\"") {
        let href_start = pos + href_start + 6; // skip `href="`
        let Some(href_end) = html[href_start..].find('"') else {
            break;
        };
        let url = &html[href_start..href_start + href_end];
        pos = href_start + href_end + 1;

        let url_trimmed = url.trim_end_matches('/');

        if (url_trimmed.contains(&needle) || url_trimmed.contains(&needle_rel))
            && (url_trimmed.ends_with("/rus") || url_trimmed.contains("/rus/"))
        {
            let rel = url_trimmed
                .find("/manga/")
                .map(|i| &url_trimmed[i + 1..]) // strip leading "/"
                .unwrap_or(url_trimmed)
                .to_string();
            if seen.insert(rel.clone()) {
                chapters.push(rel);
            }
        }
    }

    chapters.sort_by(|a, b| chapter_sort_key(a).cmp(&chapter_sort_key(b)));
    chapters
}

fn chapter_sort_key(url: &str) -> (u32, u32, u32) {
    let vol = extract_num_after(url, "/vol").unwrap_or(0);
    let ch_str = extract_str_after(url, "/ch").unwrap_or_default();
    // дробные главы умножаем на 10 чтоб сохранить порядок
    let ch = (ch_str
        .split('/')
        .next()
        .unwrap_or("0")
        .parse::<f64>()
        .unwrap_or(0.0)
        * 10.0) as u32;
    let subch = 0u32;
    (vol, ch, subch)
}

// достаёт slug.id из финального URL
fn slug_from_url(url: &str, id: &str) -> Option<String> {
    let after = url.strip_prefix(&format!("{}/manga/", BASE))?;
    let slug_id = after.split('/').next()?;
    if slug_id.ends_with(&format!(".{}", id)) {
        Some(slug_id.to_string())
    } else {
        None
    }
}

fn extract_num_after(s: &str, pat: &str) -> Option<u32> {
    let pos = s.find(pat)? + pat.len();
    let rest = &s[pos..];
    let end = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
    rest[..end].parse().ok()
}

fn extract_str_after<'a>(s: &'a str, pat: &str) -> Option<&'a str> {
    let pos = s.find(pat)? + pat.len();
    Some(&s[pos..])
}

// парсит Reader.init — возвращает (base_url, имена файлов)
fn parse_reader_init(html: &str) -> Option<(String, Vec<String>)> {
    let init_pos = html.find("Reader.init(")?;
    let obj_start = html[init_pos..].find('{')? + init_pos;
    let obj_end = find_matching_brace(html, obj_start)?;
    let obj = &html[obj_start..=obj_end];

    let dir = parse_js_string(obj, "dir")?;
    let dir = if dir.starts_with("//") {
        format!("https:{}", dir)
    } else {
        dir
    };

    let images_pos = obj.find("images:")?;
    let arr_start = images_pos + obj[images_pos..].find('[')? + 1;
    let arr = &obj[arr_start..];

    let mut filenames = Vec::new();
    let mut pos = 0;
    while let Some(inner_start) = arr[pos..].find('[') {
        let inner_start = pos + inner_start + 1;
        let Some(inner_end) = arr[inner_start..].find(']') else {
            break;
        };
        let entry = &arr[inner_start..inner_start + inner_end];
        if let Some(fname) = parse_first_string(entry) {
            let fname = fname.split('?').next().unwrap_or(&fname).to_string();
            filenames.push(fname);
        }
        pos = inner_start + inner_end + 1;
        if arr[pos..].starts_with(']') {
            break;
        }
    }

    Some((dir, filenames))
}

fn find_matching_brace(s: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (i, ch) in s[open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(open + i);
                }
            }
            _ => {}
        }
    }
    None
}

fn parse_js_string(obj: &str, key: &str) -> Option<String> {
    let key_pat = format!("{}:", key);
    let pos = obj.find(&key_pat)? + key_pat.len();
    let rest = obj[pos..].trim_start();
    if rest.starts_with('"') {
        let inner = &rest[1..];
        let end = inner.find('"')?;
        Some(inner[..end].to_string())
    } else {
        None
    }
}

fn parse_first_string(entry: &str) -> Option<String> {
    let start = entry.find('"')? + 1;
    let rest = &entry[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

// поиск

fn run_search(rest: &[String]) {
    if rest.len() != 1 {
        eprintln!("{}", tr("search expects exactly one keyword.\n"));
        print_service_usage();
        exit(1);
    }

    let keyword = &rest[0];
    let client = build_client();

    let url = format!("{}/manga/search/", BASE);
    let body = match client
        .get(&url)
        .header(reqwest::header::REFERER, REFERER)
        .header("X-Requested-With", "XMLHttpRequest")
        .query(&[("q", keyword.as_str()), ("_xfResponseType", "json")])
        .send()
        .map_err(|e| trf("API request error: {}", &[&e]))
        .and_then(|r| {
            if r.status().is_success() {
                r.text().map_err(|e| trf("Failed to read response: {}", &[&e]))
            } else {
                Err(trf("API returned status {}", &[&r.status()]))
            }
        }) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("{}", e);
            exit(1);
        }
    };

    let html = match template_html_full(&body) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("{}", e);
            exit(1);
        }
    };

    let results = parse_search_results(&html);

    if results.is_empty() {
        println!("{}", trf("No manga found matching '{}'.", &[&keyword]));
        return;
    }

    println!("{}", trf("Manga matching '{}':\n", &[&keyword]));
    println!("{:<55} {}", tr("HUMAN TITLE"), tr("SYSTEM ID"));
    println!("{}", "-".repeat(75));

    for (slug_id, en_title, ru_title) in &results {
        let display = if ru_title.is_empty() {
            en_title.clone()
        } else {
            format!("{} / {}", ru_title, en_title)
        };
        println!("{:<55} {}", display, slug_id);
    }

    println!(
        "\n{}",
        trf(
            "{} results. Use the id/slug.id with: nymphalis desu.uno download <slug.id>",
            &[&results.len()]
        )
    );
}

fn parse_search_results(html: &str) -> Vec<(String, String, String)> {
    let mut results = Vec::new();

    let re_li = Regex::new(r"(?s)<li[^>]*>(.*?)</li>").ok();
    let re_href = Regex::new(r#"href=["'](?:/)?manga/([^"']+)["']"#).ok();
    let re_title_new = Regex::new(r#"(?s)class=["'][^"']*AniMangaSearchCard__title[^"']*["'][^>]*>(.*?)</span>"#).ok();
    let re_sub_new = Regex::new(r#"(?s)class=["'][^"']*AniMangaSearchCard__subtitle[^"']*["'][^>]*>(.*?)</span>"#).ok();

    if let (Some(r_li), Some(r_href)) = (re_li, re_href) {
        for cap in r_li.captures_iter(html) {
            let block = &cap[1];
            let slug_id = if let Some(h) = r_href.captures(block).and_then(|c| c.get(1)) {
                h.as_str().trim_end_matches('/').to_string()
            } else {
                continue;
            };

            let mut ru_title = String::new();
            let mut en_title = String::new();

            if let Some(ref r_t) = re_title_new {
                if let Some(t) = r_t.captures(block).and_then(|c| c.get(1)) {
                    ru_title = strip_tags(t.as_str());
                }
            }
            if let Some(ref r_s) = re_sub_new {
                if let Some(t) = r_s.captures(block).and_then(|c| c.get(1)) {
                    en_title = strip_tags(t.as_str());
                }
            }

            if ru_title.is_empty() && en_title.is_empty() {
                en_title = extract_div_class(block, "itemTitle");
                ru_title = extract_div_class(block, "itemSubTitle");
            }

            if !slug_id.is_empty() {
                results.push((slug_id, en_title, ru_title));
            }
        }
    }

    if results.is_empty() {
        let manga_section = html
            .find(">Манга<")
            .map(|p| {
                let rest = &html[p..];
                rest.find(">Аниме<").map(|e| &rest[..e]).unwrap_or(rest)
            })
            .unwrap_or(html);

        let mut pos = 0;
        while let Some(li_start) = manga_section[pos..].find("<li>") {
            let li_start = pos + li_start;
            let li_end = manga_section[li_start..].find("</li>").map(|e| li_start + e + 5);
            let block = &manga_section[li_start..li_end.unwrap_or(manga_section.len())];
            pos = li_end.unwrap_or(manga_section.len());

            let slug_id = extract_manga_href(block);
            let en_title = extract_div_class(block, "itemTitle");
            let ru_title = extract_div_class(block, "itemSubTitle");

            if let Some(slug_id) = slug_id {
                results.push((slug_id, en_title, ru_title));
            }
        }
    }

    results
}

fn extract_manga_href(block: &str) -> Option<String> {
    let href_pat = "href=\"manga/";
    let pos = block.find(href_pat)? + href_pat.len();
    let rest = &block[pos..];
    let end = rest.find('"')?;
    let slug_id = rest[..end].trim_end_matches('/');
    Some(slug_id.to_string())
}

fn extract_div_class(block: &str, class: &str) -> String {
    let pat = format!("class=\"{}\"", class);
    let p = match block.find(&pat) {
        Some(v) => v,
        None => return String::new(),
    };
    let after = &block[p + pat.len()..];
    let inner_start = match after.find('>') {
        Some(v) => v + 1,
        None => return String::new(),
    };
    let inner = &after[inner_start..];
    let inner_end = inner.find('<').unwrap_or(inner.len());
    inner[..inner_end].trim().to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DownloadKind {
    Manga,
    Ranobe,
}

fn run_download(rest: &[String]) {
    let (kind, rest_args) = match rest.first().map(|s| s.as_str()) {
        Some("ranobe") => (DownloadKind::Ranobe, &rest[1..]),
        Some("manga") => (DownloadKind::Manga, &rest[1..]),
        _ => (DownloadKind::Manga, rest),
    };

    if rest_args.len() < 2 {
        let msg = match kind {
            DownloadKind::Ranobe => tr("download ranobe requires an output directory and at least one title.\n"),
            DownloadKind::Manga => tr("download requires at least one manga title.\n"),
        };
        eprintln!("{}", msg);
        print_service_usage();
        exit(1);
    }

    let out_dir = &rest_args[0];
    let titles = &rest_args[1..];

    if let Err(e) = fs::create_dir_all(out_dir) {
        eprintln!(
            "{}",
            trf("Failed to create directory '{}': {}", &[&out_dir, &e])
        );
        exit(1);
    }

    let client = build_client();

    for title in titles {
        match kind {
            DownloadKind::Manga => download_manga(&client, out_dir, title),
            DownloadKind::Ranobe => download_ranobe(&client, out_dir, title),
        }
    }
}

fn download_manga(client: &reqwest::blocking::Client, base_dir: &str, input: &str) {
    let (slug, id) = match parse_slug_id(input) {
        Some(v) => v,
        None => {
            eprintln!(
                "{}",
                trf(
                    "Invalid manga id format: '{}'. Expected numeric id or 'slug.id'.",
                    &[&input]
                )
            );
            return;
        }
    };

    let slug_id = format!("{}.{}", slug, id);
    let manga_url = format!("{}/manga/{}/", BASE, slug_id);

    // ксенфоро ломает XHR на эту страницу
    let (html, final_url) = match plain_get_with_url(client, &manga_url) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "{}",
                trf("Failed to fetch manga info for id {}: {}", &[&id, &e])
            );
            return;
        }
    };

    let manga_name = parse_h1(&html).unwrap_or_else(|| slug_id.clone());
    let id_str = id.to_string();
    let actual_slug_id = slug_from_url(&final_url, &id_str).unwrap_or_else(|| slug_id.clone());

    let effective_manga_id = parse_manga_id_from_chlist(&html).unwrap_or(id);
    let chapters = parse_chapters(&html, &actual_slug_id);

    if chapters.is_empty() {
        println!("{}", trf("No chapters found for '{}'.", &[&manga_name]));
        return;
    }

    println!(
        "{}",
        trf(
            "Downloading '{}': {} chapter(s)",
            &[&manga_name, &chapters.len()]
        )
    );

    let root_dir = PathBuf::from(base_dir);
    if let Err(e) = fs::create_dir_all(&root_dir) {
        eprintln!(
            "{}",
            trf(
                "Failed to create directory '{}': {}",
                &[&root_dir.display(), &e]
            )
        );
        return;
    }

    if !confirm_chapters(chapters.len()) {
        println!("{}", tr("Cancelled."));
        return;
    }

    // обложка
    let cover_url = format!("https://static.desu.uno/data/manga/covers/preview/{}.jpg", id);
    let cover_path = root_dir.join("cover.jpg");
    if !cover_path.exists() {
        let _ = download_file_simple(client, &cover_url, &cover_path, REFERER);
    }

    let display = crate::cli::DownloadDisplay::new(
        chapters.len() as u64,
        manga_name.chars().take(22).collect::<String>(),
    );

    let total = chapters.len();
    let mut downloaded = 0usize;

    for (idx, chapter) in chapters.iter().enumerate() {
        display.set_msg(trf("Chapter {}/{}", &[&(idx + 1), &total]));

        let chapter_name = chapter_dir_name(&chapter.path);
        let chapter_dir = root_dir.join(&chapter_name);

        if let Err(e) = fs::create_dir_all(&chapter_dir) {
            display.println(trf(
                "Failed to create directory '{}': {}",
                &[&chapter_dir.display(), &e],
            ));
            display.advance();
            continue;
        }

        let mut page_urls: Vec<String> = Vec::new();

        // 1. Попытка через API (по кнопке chDownload / data-chapters_id)
        if let Some(cid) = chapter.id {
            match fetch_chapter_pages_api(client, effective_manga_id, cid) {
                Ok(urls) => page_urls = urls,
                Err(e) => {
                    display.println(trf(
                        "Chapter {} API error: {}",
                        &[&chapter_name, &e],
                    ));
                }
            }
        }

        // 2. Fallback: если страниц нет, открываем страницу читалки
        if page_urls.is_empty() {
            let chapter_url = format!("{}/{}", BASE, chapter.path);
            match plain_get(client, &chapter_url) {
                Ok(chapter_html) => {
                    // Пробуем извлечь id главы из window.MangaReader
                    if let Some(cid) = parse_chapter_id_from_reader_html(&chapter_html) {
                        if let Ok(urls) = fetch_chapter_pages_api(client, effective_manga_id, cid) {
                            page_urls = urls;
                        }
                    }
                    // Если всё ещё нет, проверяем старый Reader.init
                    if page_urls.is_empty() {
                        if let Some((dir, filenames)) = parse_reader_init(&chapter_html) {
                            page_urls = filenames
                                .into_iter()
                                .map(|f| format!("{}{}", dir, f))
                                .collect();
                        }
                    }
                }
                Err(e) => {
                    display.println(trf(
                        "Failed to fetch pages for chapter {}: {}",
                        &[&chapter_name, &e],
                    ));
                }
            }
        }

        if page_urls.is_empty() {
            display.println(trf("No pages in chapter {}.", &[&chapter_name]));
            display.advance();
            continue;
        }

        let handle = display.begin(crate::cli::ContentUnit::pages(
            &chapter_name,
            page_urls.len() as u64,
        ));

        let mut chapter_ok = true;
        for (pg, img_url) in page_urls.iter().enumerate() {
            let clean_url = img_url.split('?').next().unwrap_or(img_url);
            let ext = clean_url
                .rsplit('.')
                .next()
                .filter(|e| e.len() <= 5 && !e.contains('/'))
                .unwrap_or("jpg");
            let dest = chapter_dir.join(format!("{:04}.{}", pg + 1, ext));

            if dest.exists() && dest.metadata().map(|m| m.len() > 0).unwrap_or(false) {
                handle.inc(1);
                continue;
            }

            if let Err(e) = download_file_simple(client, img_url, &dest, REFERER) {
                display.println(format!("  {} p{:04}: {}", chapter_name, pg + 1, e));
                chapter_ok = false;
            }
            handle.inc(1);
        }

        if chapter_ok {
            display.end_ok(handle, page_urls.len() as u64);
            downloaded += 1;
        } else {
            display.end_err(handle, "some pages failed");
        }
        display.advance();
    }

    display.finish(trf("Done. {} chapter(s) downloaded.", &[&downloaded]));
    println!("{}", trf("Done. {} chapter(s) downloaded.", &[&downloaded]));
}

fn chapter_dir_name(chapter_path: &str) -> String {
    let vol = segment_after(chapter_path, "/vol").unwrap_or("0");
    let ch = segment_after(chapter_path, "/ch").unwrap_or("0");
    format!("vol{}_ch{}", vol, ch)
}

fn segment_after<'a>(s: &'a str, pat: &str) -> Option<&'a str> {
    let pos = s.find(pat)? + pat.len();
    let rest = &s[pos..];
    Some(rest.split('/').next().unwrap_or(rest))
}

fn parse_h1(html: &str) -> Option<String> {
    let start = html.find("<h1")? ;
    let inner_start = html[start..].find('>')? + start + 1;
    let inner_end = html[inner_start..].find("</h1>")? + inner_start;
    Some(strip_tags(&html[inner_start..inner_end]).trim().to_string())
}

fn strip_tags(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn confirm_chapters(count: usize) -> bool {
    print!(
        "{}",
        trf(
            "About to download {} chapter(s). Continue? [y/N]: ",
            &[&count]
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

fn download_ranobe(client: &reqwest::blocking::Client, base_dir: &str, input: &str) {
    let (slug, id) = match parse_slug_id(input) {
        Some(v) => v,
        None => {
            eprintln!(
                "{}",
                trf(
                    "Invalid ranobe id format: '{}'. Expected numeric id or 'slug.id'.",
                    &[&input]
                )
            );
            return;
        }
    };

    let mut chapters = match fetch_ranobe_chapters_api(client, id) {
        Ok(chs) => chs,
        Err(e) => {
            eprintln!(
                "{}",
                trf("Failed to fetch ranobe chapters for id {}: {}", &[&id, &e])
            );
            return;
        }
    };

    if chapters.is_empty() {
        println!("{}", trf("No chapters found for ranobe id {}.", &[&id]));
        return;
    }

    let actual_slug = chapters
        .first()
        .and_then(|c| c.view_url.as_deref())
        .and_then(parse_slug_from_view_url)
        .unwrap_or_else(|| {
            if slug != id.to_string() {
                format!("{}.{}", slug, id)
            } else {
                id.to_string()
            }
        });

    let ranobe_url = format!("{}/ranobe/{}/", BASE, actual_slug);
    let ranobe_name = match plain_get_with_url(client, &ranobe_url) {
        Ok((html, _)) => parse_h1(&html).unwrap_or_else(|| actual_slug.clone()),
        Err(_) => actual_slug.clone(),
    };

    chapters.sort_by(|a, b| {
        ranobe_sort_key(&a.volume_str(), &a.number_str())
            .cmp(&ranobe_sort_key(&b.volume_str(), &b.number_str()))
    });

    println!(
        "{}",
        trf(
            "Downloading ranobe '{}': {} chapter(s)",
            &[&ranobe_name, &chapters.len()]
        )
    );

    let root_dir = PathBuf::from(base_dir);
    if let Err(e) = fs::create_dir_all(&root_dir) {
        eprintln!(
            "{}",
            trf(
                "Failed to create directory '{}': {}",
                &[&root_dir.display(), &e]
            )
        );
        return;
    }

    if !confirm_chapters(chapters.len()) {
        println!("{}", tr("Cancelled."));
        return;
    }

    let cover_url = format!("https://static.desu.uno/data/ranobe/covers/preview/{}.jpg", id);
    let cover_path = root_dir.join("cover.jpg");
    if !cover_path.exists() {
        let _ = download_file_simple(client, &cover_url, &cover_path, REFERER);
    }

    let display = crate::cli::DownloadDisplay::new(
        chapters.len() as u64,
        ranobe_name.chars().take(22).collect::<String>(),
    );

    let total = chapters.len();
    let mut downloaded = 0usize;

    for (idx, chapter) in chapters.iter().enumerate() {
        display.set_msg(trf("Chapter {}/{}", &[&(idx + 1), &total]));

        let vol_str = chapter.volume_str();
        let num_str = chapter.number_str();
        let vol = if vol_str.is_empty() { "0" } else { &vol_str };
        let num = if num_str.is_empty() { "0" } else { &num_str };
        let file_stem = format!("vol{}_ch{}", vol, num);

        let epub_path = root_dir.join(format!("{}.epub", file_stem));
        let txt_path = root_dir.join(format!("{}.txt", file_stem));

        if epub_path.exists()
            && epub_path.metadata().map(|m| m.len() > 0).unwrap_or(false)
            && txt_path.exists()
            && txt_path.metadata().map(|m| m.len() > 0).unwrap_or(false)
        {
            downloaded += 1;
            display.advance();
            continue;
        }

        let handle = display.begin(crate::cli::ContentUnit::bytes(&file_stem));

        let detail = match fetch_ranobe_chapter_detail_api(client, id, chapter.id) {
            Ok(d) => d,
            Err(e) => {
                display.end_err(handle, &e);
                display.advance();
                continue;
            }
        };

        let ch_title = detail
            .title
            .as_deref()
            .or_else(|| chapter.title.as_deref())
            .unwrap_or("")
            .trim();

        let display_title = if vol == "0" {
            if ch_title.is_empty() {
                format!("Глава {}", num)
            } else {
                format!("Глава {}. {}", num, ch_title)
            }
        } else {
            if ch_title.is_empty() {
                format!("Том {} Глава {}", vol, num)
            } else {
                format!("Том {} Глава {}. {}", vol, num, ch_title)
            }
        };

        let mut content_html = String::new();
        let mut plain_text_parts = Vec::new();
        let mut images_data: Vec<(String, String, Vec<u8>)> = Vec::new();

        let mut items = detail.content.unwrap_or_default();
        items.sort_by_key(|c| c.position.unwrap_or(0));

        let mut img_counter = 0usize;
        let mut has_error = false;

        for item in items {
            match item.item_type.as_str() {
                "text" => {
                    if let Some(html) = item.html {
                        let cleaned = epub::clean_xhtml(&html);
                        content_html.push_str(&cleaned);
                        content_html.push('\n');

                        let plain = epub::html_to_plain_text(&html);
                        if !plain.is_empty() {
                            plain_text_parts.push(plain);
                        }
                    }
                }
                "image" => {
                    if let Some(img_url) = item.url {
                        if !img_url.is_empty() {
                            img_counter += 1;
                            let clean_url = img_url.split('?').next().unwrap_or(&img_url);
                            let ext = clean_url
                                .rsplit('.')
                                .next()
                                .filter(|e| e.len() <= 5 && !e.contains('/'))
                                .unwrap_or("jpg");

                            let img_rel_path = format!("images/img_{:03}.{}", img_counter, ext);
                            let img_id = format!("img_{:03}", img_counter);

                            match download_bytes(client, &img_url) {
                                Ok(bytes) => {
                                    content_html.push_str(&format!(
                                        r#"<div class="chapter-image"><img src="{}" alt="" /></div>"#,
                                        img_rel_path
                                    ));
                                    content_html.push('\n');
                                    images_data.push((img_id, img_rel_path, bytes));
                                }
                                Err(e) => {
                                    display.println(format!("  {} img_{:03}: {}", file_stem, img_counter, e));
                                    has_error = true;
                                }
                            }
                            plain_text_parts.push(format!("[Иллюстрация: img_{:03}.{}]", img_counter, ext));
                        }
                    }
                }
                _ => {}
            }
        }

        let image_refs: Vec<(&str, &str, &[u8])> = images_data
            .iter()
            .map(|(id, path, bytes)| (id.as_str(), path.as_str(), bytes.as_slice()))
            .collect();

        let epub_bytes = epub::build_epub(
            &display_title,
            &format!("urn:desu:ranobe:{}:{}", id, chapter.id),
            &content_html,
            &image_refs,
        );

        let plain_text = format!("{}\n\n{}", display_title, plain_text_parts.join("\n\n"));

        let write_ok = fs::write(&epub_path, &epub_bytes).is_ok()
            && fs::write(&txt_path, plain_text.as_bytes()).is_ok();

        let total_size = (epub_bytes.len() + plain_text.len()) as u64;

        if write_ok && !has_error {
            display.end_ok(handle, total_size);
            downloaded += 1;
        } else if write_ok {
            display.end_ok(handle, total_size);
            downloaded += 1;
        } else {
            display.end_err(handle, "write error");
        }
        display.advance();
    }

    display.finish(trf("Done. {} chapter(s) downloaded.", &[&downloaded]));
    println!("{}", trf("Done. {} chapter(s) downloaded.", &[&downloaded]));
}

fn ranobe_sort_key(volume: &str, number: &str) -> (u32, u32) {
    let vol = volume.parse::<u32>().unwrap_or(0);
    let ch = (number.parse::<f64>().unwrap_or(0.0) * 10.0) as u32;
    (vol, ch)
}

fn parse_slug_from_view_url(view_url: &str) -> Option<String> {
    let pos = view_url.find("/ranobe/")? + "/ranobe/".len();
    let rest = &view_url[pos..];
    let end = rest.find('/')?;
    Some(rest[..end].to_string())
}

fn download_bytes(client: &reqwest::blocking::Client, url: &str) -> Result<Vec<u8>, String> {
    let mut req = client.get(url).header(reqwest::header::REFERER, REFERER);
    if let Some(cookie) = session_cookie() {
        req = req.header(reqwest::header::COOKIE, cookie);
    }
    let resp = req.send().map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.bytes().map(|b| b.to_vec()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ranobe_sort_key() {
        assert_eq!(ranobe_sort_key("1", "2"), (1, 20));
        assert_eq!(ranobe_sort_key("1", "2.5"), (1, 25));
        assert_eq!(ranobe_sort_key("2", "1"), (2, 10));
        assert!(ranobe_sort_key("1", "10") < ranobe_sort_key("2", "1"));
        assert!(ranobe_sort_key("1", "2") < ranobe_sort_key("1", "2.5"));
    }

    #[test]
    fn test_parse_slug_from_view_url() {
        let url = "https://desu.uno/ranobe/sss-class-suicide-hunter.56/vol2/ch401/rus";
        assert_eq!(
            parse_slug_from_view_url(url),
            Some("sss-class-suicide-hunter.56".to_string())
        );
    }

    #[test]
    fn test_parse_ranobe_chapters_json() {
        let json = r#"{
            "chapters": [
                {
                    "id": 100,
                    "volume": "1",
                    "number": "5",
                    "title": "Test Chapter",
                    "type": "text",
                    "view_url": "https://desu.uno/ranobe/test.1/vol1/ch5/rus"
                },
                {
                    "id": 101,
                    "volume": 2,
                    "number": 10,
                    "title": null,
                    "type": "hybrid",
                    "view_url": "https://desu.uno/ranobe/test.1/vol2/ch10/rus"
                }
            ]
        }"#;

        let resp: RanobeChaptersResponse = serde_json::from_str(json).unwrap();
        let chapters = resp.chapters.unwrap();
        assert_eq!(chapters.len(), 2);
        assert_eq!(chapters[0].volume_str(), "1");
        assert_eq!(chapters[0].number_str(), "5");
        assert_eq!(chapters[1].volume_str(), "2");
        assert_eq!(chapters[1].number_str(), "10");
    }
}

