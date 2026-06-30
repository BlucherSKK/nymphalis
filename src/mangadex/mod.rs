use crate::gelbooru::download_file_simple;
use crate::http::build_client;
use crate::language::{tr, trf};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::process::exit;

const API: &str = "https://api.mangadex.org";
const REFERER: &str = "https://mangadex.org/";

pub fn dispatch(rest: &[String]) {
    if rest.is_empty() {
        print_usage();
        exit(1);
    }
    match rest[0].as_str() {
        "download"      => run_download(&rest[1..]),
        "search"        => run_search(&rest[1..]),
        "-h" | "--help" => { print_usage(); exit(0); }
        other => {
            eprintln!("{}", trf("Unknown command for mangadex.org: {}", &[&other]));
            print_usage();
            exit(1);
        }
    }
}

fn print_usage() {
    eprintln!("{}", tr("Commands (mangadex.org):"));
    eprintln!("{}", tr("  nymphalis md search <keyword>"));
    eprintln!("{}", tr("      Search manga with Russian translation available."));
    eprintln!("{}", tr("  nymphalis md download <dir> <manga-uuid> [uuid2 ...]"));
    eprintln!("{}", tr("      Download all Russian chapters into <dir>/<uuid>/."));
}

// HTTP

fn api_get(client: &reqwest::blocking::Client, url: &str) -> Result<Value, String> {
    let resp = client
        .get(url)
        .header(reqwest::header::REFERER, REFERER)
        .send()
        .map_err(|e| format!("request error: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("status {}", resp.status().as_u16()));
    }

    resp.json::<Value>().map_err(|e| format!("JSON error: {}", e))
}

// поиск

fn run_search(rest: &[String]) {
    if rest.is_empty() {
        eprintln!("{}", tr("search requires a keyword."));
        exit(1);
    }
    let query = rest.join(" ");
    let client = build_client();

    let url = format!(
        "{}/manga?title={}\
         &contentRating[]=safe&contentRating[]=suggestive\
         &contentRating[]=erotica&contentRating[]=pornographic\
         &limit=20&includes[]=author",
        API,
        percent_encode(&query)
    );

    let json = match api_get(&client, &url) {
        Ok(j) => j,
        Err(e) => { eprintln!("{}", trf("Search failed: {}", &[&e])); exit(1); }
    };

    let data = match json["data"].as_array() {
        Some(d) if !d.is_empty() => d,
        _ => { println!("{}", trf("No manga found for '{}'.", &[&query])); return; }
    };

    println!("{}", trf("\nManga search results for '{}':\n", &[&query]));
    println!("{:<55} {}", tr("TITLE"), tr("UUID"));
    println!("{}", "-".repeat(95));

    for item in data {
        let id    = item["id"].as_str().unwrap_or("?");
        let attrs = &item["attributes"];
        let title = best_title(attrs);
        let ru    = ru_title(attrs);
        println!("{:<55} {}", truncate(&title, 54), id);
        if let Some(r) = ru {
            if r != title {
                println!("  {}", truncate(&r, 72));
            }
        }
    }

    println!("{}", trf("Found: {}.", &[&data.len()]));
    println!("{}", tr("Use uuid with: nymphalis md download <dir> <uuid>"));
}

// берёт лучший доступный тайтл: ru > en > романизация
fn best_title(attrs: &Value) -> String {
    let titles = &attrs["title"];
    for lang in ["ru", "en", "ja-ro"] {
        if let Some(t) = titles[lang].as_str() {
            return t.to_string();
        }
    }
    if let Some(alts) = attrs["altTitles"].as_array() {
        for prefer in ["ru", "en"] {
            for alt in alts {
                if let Some(t) = alt[prefer].as_str() {
                    return t.to_string();
                }
            }
        }
    }
    if let Some(obj) = titles.as_object() {
        if let Some(v) = obj.values().next().and_then(|v| v.as_str()) {
            return v.to_string();
        }
    }
    "?".to_string()
}

// возвращает русский тайтл или None
fn ru_title(attrs: &Value) -> Option<String> {
    if let Some(t) = attrs["title"]["ru"].as_str() {
        return Some(t.to_string());
    }
    if let Some(alts) = attrs["altTitles"].as_array() {
        for alt in alts {
            if let Some(t) = alt["ru"].as_str() {
                return Some(t.to_string());
            }
        }
    }
    None
}

// скачивание

fn run_download(rest: &[String]) {
    if rest.len() < 2 {
        eprintln!("{}", tr("download requires <dir> <manga-uuid> [uuid2 ...]"));
        exit(1);
    }
    let out_dir = &rest[0];
    let client  = build_client();
    for id in &rest[1..] {
        download_manga(&client, out_dir, id);
    }
}

fn download_manga(client: &reqwest::blocking::Client, base_dir: &str, manga_id: &str) {
    let info_url = format!("{}/manga/{}", API, manga_id);
    let title = match api_get(client, &info_url) {
        Ok(j) => best_title(&j["data"]["attributes"]),
        Err(e) => { eprintln!("{}", trf("Failed to fetch manga '{}': {}", &[&manga_id, &e])); return; }
    };

    let agg_url = format!("{}/manga/{}/aggregate?translatedLanguage[]=ru", API, manga_id);
    let agg = match api_get(client, &agg_url) {
        Ok(j) => j,
        Err(e) => { eprintln!("{}", trf("Failed to get chapters for '{}': {}", &[&title, &e])); return; }
    };

    let chapters = collect_chapters(&agg);
    if chapters.is_empty() {
        println!("{}", trf("No Russian chapters for '{}'.", &[&title]));
        return;
    }

    let root_dir = PathBuf::from(base_dir);
    if let Err(e) = fs::create_dir_all(&root_dir) {
        eprintln!("{}", trf("Cannot create {}: {}", &[&root_dir.display(), &e]));
        return;
    }

    println!("{}", trf("Downloading '{}': {} chapter(s)", &[&title, &chapters.len()]));

    let display = crate::cli::DownloadDisplay::new(
        chapters.len() as u64,
        title.chars().take(22).collect::<String>(),
    );

    for (idx, (vol, ch, ch_id)) in chapters.iter().enumerate() {
        display.set_msg(format!("vol{} ch{}", vol, ch));

        let server_url = format!("{}/at-home/server/{}", API, ch_id);
        let server = match api_get(client, &server_url) {
            Ok(j) => j,
            Err(e) => {
                display.println(trf("  Chapter {}/{} server error: {}", &[&(idx + 1), &chapters.len(), &e]));
                display.advance();
                continue;
            }
        };

        let base_url = server["baseUrl"].as_str().unwrap_or("");
        let hash     = server["chapter"]["hash"].as_str().unwrap_or("");
        let pages: Vec<&str> = server["chapter"]["data"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
            .unwrap_or_default();

        if pages.is_empty() {
            display.advance();
            continue;
        }

        let dir_name = format!("vol{}_ch{}", vol, ch);
        let ch_dir   = root_dir.join(&dir_name);
        if let Err(e) = fs::create_dir_all(&ch_dir) {
            display.println(trf("Cannot create {}: {}", &[&ch_dir.display(), &e]));
            display.advance();
            continue;
        }

        let handle = display.begin(crate::cli::ContentUnit::pages(
            &dir_name,
            pages.len() as u64,
        ));

        for (pi, filename) in pages.iter().enumerate() {
            let url  = format!("{}/data/{}/{}", base_url, hash, filename);
            let ext  = filename.rsplit('.').next().unwrap_or("jpg");
            let dest = ch_dir.join(format!("{:04}.{}", pi + 1, ext));
            if !dest.exists() {
                let _ = download_file_simple(client, &url, &dest, REFERER);
            }
            handle.inc(1);
        }

        display.end_ok(handle, pages.len() as u64);
        display.advance();

        // мангадекс просит не спешить
        std::thread::sleep(std::time::Duration::from_millis(1500));
    }

    display.finish(tr("done"));
    println!("{}", trf("Done. {} chapter(s) downloaded.", &[&chapters.len()]));
}

// разворачивает aggregate → список (том, глава, uuid)
fn collect_chapters(agg: &Value) -> Vec<(String, String, String)> {
    let mut out: Vec<(f64, f64, String, String, String)> = Vec::new();

    if let Some(vols) = agg["volumes"].as_object() {
        for (vol_key, vol_val) in vols {
            let vol_n: f64 = vol_key.parse().unwrap_or(f64::MAX);
            if let Some(chs) = vol_val["chapters"].as_object() {
                for (ch_key, ch_val) in chs {
                    let ch_n: f64  = ch_key.parse().unwrap_or(f64::MAX);
                    let ch_id      = ch_val["id"].as_str().unwrap_or("").to_string();
                    if !ch_id.is_empty() {
                        out.push((vol_n, ch_n, vol_key.clone(), ch_key.clone(), ch_id));
                    }
                }
            }
        }
    }

    out.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap().then(a.1.partial_cmp(&b.1).unwrap()));
    out.into_iter().map(|(_, _, v, c, id)| (v, c, id)).collect()
}

// хелперы

fn percent_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9'
            | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

fn truncate(s: &str, max: usize) -> String {
    let mut chars = s.chars();
    let mut out = String::new();
    for _ in 0..max {
        match chars.next() {
            Some(c) => out.push(c),
            None    => break,
        }
    }
    if chars.next().is_some() { out.push_str("…"); }
    out
}
