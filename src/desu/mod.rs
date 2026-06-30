use crate::config::{load_config, save_config};
use crate::gelbooru::download_file_simple;
use crate::language::{tr, trf};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
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

// логин

enum BrowserKind {
    Firefox,
    Chromium(&'static str),
    Falkon,
}

struct Browser {
    bin:  &'static str,
    kind: BrowserKind,
}

fn resolve_browser(name: &str) -> Option<Browser> {
    match name.to_lowercase().as_str() {
        "firefox" | "ff" =>
            Some(Browser { bin: "firefox",       kind: BrowserKind::Firefox }),
        "chromium" =>
            Some(Browser { bin: "chromium",       kind: BrowserKind::Chromium("chromium") }),
        "chrome" | "google-chrome" =>
            Some(Browser { bin: "google-chrome",  kind: BrowserKind::Chromium("google-chrome") }),
        "brave" | "brave-browser" =>
            Some(Browser { bin: "brave-browser",  kind: BrowserKind::Chromium("brave-browser") }),
        "falkon" =>
            Some(Browser { bin: "falkon",         kind: BrowserKind::Falkon }),
        _ => None,
    }
}

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

    open_browser(LOGIN_URL, browser.as_ref().map(|b| b.bin));

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

fn open_browser(url: &str, bin: Option<&str>) {
    let cmd = bin.unwrap_or("xdg-open");
    let launched = std::process::Command::new(cmd)
        .arg(url)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .is_ok();
    if !launched {
        println!("{}", trf("Could not launch '{}' automatically.", &[&cmd]));
        println!("{}", trf("Please open this URL manually: {}", &[&url]));
    }
}


fn read_firefox_desu_session() -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    let ff_dir = PathBuf::from(&home).join(".mozilla").join("firefox");
    for entry in fs::read_dir(&ff_dir).ok()?.flatten() {
        let db = entry.path().join("cookies.sqlite");
        if !db.exists() { continue; }
        let sql = "SELECT GROUP_CONCAT(name || '=' || value, '; ') \
                   FROM (SELECT name, value FROM moz_cookies \
                         WHERE (host = 'desu.uno' OR host = '.desu.uno') \
                         AND name LIKE 'xf_%' AND length(value) > 0 \
                         GROUP BY name ORDER BY lastAccessed DESC)";
        if let Some(v) = sqlite3_query(&db, sql) { return Some(v); }
    }
    None
}

fn read_chromium_desu_session(browser: &str) -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    let db = PathBuf::from(&home)
        .join(".config")
        .join(browser)
        .join("Default")
        .join("Cookies");
    if !db.exists() { return None; }
    let sql = "SELECT GROUP_CONCAT(name || '=' || value, '; ') \
               FROM (SELECT name, value FROM cookies \
                     WHERE (host_key = 'desu.uno' OR host_key = '.desu.uno') \
                     AND name LIKE 'xf_%' AND length(value) > 0 \
                     GROUP BY name ORDER BY last_access_utc DESC)";
    sqlite3_query(&db, sql)
}

fn read_falkon_desu_session() -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    let profiles_dir = PathBuf::from(&home)
        .join(".config")
        .join("falkon")
        .join("profiles");
    for entry in fs::read_dir(&profiles_dir).ok()?.flatten() {
        let db = entry.path().join("Cookies");
        if !db.exists() { continue; }
        let sql = "SELECT GROUP_CONCAT(name || '=' || value, '; ') \
                   FROM (SELECT name, value FROM cookies \
                         WHERE (host_key = 'desu.uno' OR host_key = '.desu.uno') \
                         AND name LIKE 'xf_%' AND length(value) > 0 \
                         GROUP BY name ORDER BY last_access_utc DESC)";
        if let Some(v) = sqlite3_query(&db, sql) { return Some(v); }
    }
    None
}

fn sqlite3_query(db: &Path, sql: &str) -> Option<String> {
    let tmp     = std::env::temp_dir().join("booru_desu_cookie.sqlite");
    let tmp_wal = std::env::temp_dir().join("booru_desu_cookie.sqlite-wal");
    let tmp_shm = std::env::temp_dir().join("booru_desu_cookie.sqlite-shm");

    fs::copy(db, &tmp).ok()?;
    let wal = PathBuf::from(format!("{}-wal", db.display()));
    let shm = PathBuf::from(format!("{}-shm", db.display()));
    if wal.exists() { let _ = fs::copy(&wal, &tmp_wal); }
    if shm.exists() { let _ = fs::copy(&shm, &tmp_shm); }

    let out = std::process::Command::new("sqlite3")
        .arg(&tmp)
        .arg(sql)
        .output()
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
        tr("  nymphalis desu.uno download <dir> <slug.id> [slug.id2] ...")
    );
    eprintln!(
        "{}",
        tr("      Download all chapters of the given manga title(s) into <dir>.")
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

fn xhr_get(client: &reqwest::blocking::Client, url: &str) -> Result<String, String> {
    xhr_get_with_url(client, url).map(|(body, _)| body)
}

// то же что xhr_get, но ещё возвращает финальный URL
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

// собирает ссылки на главы из HTML
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

// скачивание

fn run_download(rest: &[String]) {
    if rest.len() < 2 {
        eprintln!("{}", tr("download requires at least one manga title.\n"));
        print_service_usage();
        exit(1);
    }

    let out_dir = &rest[0];
    let titles = &rest[1..];

    if let Err(e) = fs::create_dir_all(out_dir) {
        eprintln!(
            "{}",
            trf("Failed to create directory '{}': {}", &[&out_dir, &e])
        );
        exit(1);
    }

    let client = build_client();

    for title in titles {
        download_manga(&client, out_dir, title);
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

    let chapters = parse_chapter_urls(&html, &actual_slug_id);

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

    let display = crate::cli::DownloadDisplay::new(
        chapters.len() as u64,
        manga_name.chars().take(22).collect::<String>(),
    );

    let total = chapters.len();
    let mut downloaded = 0usize;

    for (idx, chapter_path) in chapters.iter().enumerate() {
        display.set_msg(trf("Chapter {}/{}", &[&(idx + 1), &total]));

        let chapter_name = chapter_dir_name(chapter_path);
        let chapter_dir = root_dir.join(&chapter_name);

        if let Err(e) = fs::create_dir_all(&chapter_dir) {
            display.println(trf(
                "Failed to create directory '{}': {}",
                &[&chapter_dir.display(), &e],
            ));
            display.advance();
            continue;
        }

        let chapter_url = format!("{}/{}", BASE, chapter_path);
        let chapter_html = match plain_get(client, &chapter_url) {
            Ok(h) => h,
            Err(e) => {
                display.println(trf(
                    "Failed to fetch pages for chapter {}: {}",
                    &[&chapter_name, &e],
                ));
                display.advance();
                continue;
            }
        };

        let (dir, filenames) = match parse_reader_init(&chapter_html) {
            Some(v) => v,
            None => {
                display.println(trf("No pages in chapter {}.", &[&chapter_name]));
                display.advance();
                continue;
            }
        };

        if filenames.is_empty() {
            display.println(trf("No pages in chapter {}.", &[&chapter_name]));
            display.advance();
            continue;
        }

        let handle = display.begin(crate::cli::ContentUnit::pages(
            &chapter_name,
            filenames.len() as u64,
        ));

        let mut chapter_ok = true;
        for (pg, filename) in filenames.iter().enumerate() {
            let ext = filename
                .rsplit('.')
                .next()
                .filter(|e| e.len() <= 5 && !e.contains('/'))
                .unwrap_or("jpg");
            let dest = chapter_dir.join(format!("{:04}.{}", pg + 1, ext));

            let img_url = format!("{}{}", dir, filename);
            if let Err(e) = download_file_simple(client, &img_url, &dest, REFERER) {
                display.println(format!("  {} p{:04}: {}", chapter_name, pg + 1, e));
                chapter_ok = false;
            }
            handle.inc(1);
        }

        if chapter_ok {
            display.end_ok(handle, filenames.len() as u64);
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
