use crate::http::build_client;
use crate::cli::{ContentUnit, DownloadDisplay};
use regex::Regex;
use serde::Deserialize;
use base64::{Engine as _, engine::general_purpose};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::Duration;
use std::thread;
use std::sync::atomic::{AtomicUsize, Ordering};
use crate::config;

#[derive(Deserialize)]
struct KodikResponse {
    links: KodikLinks,
}

#[derive(Deserialize)]
struct KodikLinks {
    #[serde(rename = "720")]
    p720: Option<Vec<KodikLink>>,
    #[serde(rename = "480")]
    p480: Option<Vec<KodikLink>>,
    #[serde(rename = "360")]
    p360: Option<Vec<KodikLink>>,
}

#[derive(Deserialize)]
struct KodikLink {
    src: String,
}

fn caesar_cipher(text: &str, shift: u8) -> String {
    let max_shift = 26;
    text.chars().map(|c| {
        if c.is_ascii_alphabetic() {
            let base = if c.is_ascii_lowercase() { b'a' } else { b'A' };
            let pos = c as u8 - base;
            let new_pos = (pos + max_shift - shift) % max_shift;
            (base + new_pos) as char
        } else {
            c
        }
    }).collect()
}

fn decode_src(encoded: &str) -> Option<String> {
    for shift in 0..=26 {
        let mut decoded_caesar = caesar_cipher(encoded, shift);
        while decoded_caesar.len() % 4 != 0 {
            decoded_caesar.push('=');
        }
        if let Ok(decoded_bytes) = general_purpose::STANDARD.decode(&decoded_caesar) {
            if let Ok(mut decoded_str) = String::from_utf8(decoded_bytes) {
                if !decoded_str.starts_with("https:") {
                    decoded_str.insert_str(0, "https:");
                }
                if decoded_str.contains(".mp4") {
                    return Some(decoded_str);
                }
            }
        }
    }
    None
}

pub fn dispatch(args: &[String]) {
    if args.is_empty() {
        eprintln!("Missing kodik command");
        std::process::exit(1);
    }
    match args[0].as_str() {
        "download" => {
            if args.len() < 3 {
                eprintln!("Usage: nymphalis kodik download <dir> <url1> [url2] ...");
                eprintln!("       nymphalis kodik download <dir> --try-parse-page <url>");
                std::process::exit(1);
            }
            let dir = &args[1];
            if args[2] == "--try-parse-page" || args[2] == "-tpp" {
                if args.len() < 4 {
                    eprintln!("Missing url for --try-parse-page");
                    std::process::exit(1);
                }
                try_parse_page(dir, &args[3]);
            } else {
                download(dir, &args[2..]);
            }
        }
        _ => {
            eprintln!("Unknown kodik command");
            std::process::exit(1);
        }
    }
}

fn download(dir: &str, urls: &[String]) {
    let c = build_client();
    fs::create_dir_all(dir).unwrap_or_default();
    let display = DownloadDisplay::new(urls.len() as u64, "Kodik");
    
    let folder_name = match std::fs::canonicalize(dir) {
        Ok(path) => path.file_name().unwrap_or_default().to_string_lossy().into_owned(),
        Err(_) => Path::new(dir).file_name().unwrap_or_default().to_string_lossy().into_owned(),
    };
    let folder_name = if folder_name.is_empty() || folder_name == "." || folder_name == ".." {
        "episode".to_string()
    } else {
        folder_name
    };
    
    let jobs = config::parallel_jobs();
    let index = AtomicUsize::new(0);

    thread::scope(|scope| {
        for _ in 0..jobs {
            let c = &c;
            let display = &display;
            let index = &index;
            let urls = urls;
            let dir = dir;
            let folder_name = &folder_name;

            scope.spawn(move || loop {
                let idx = index.fetch_add(1, Ordering::Relaxed);
                if idx >= urls.len() {
                    break;
                }
                let url = &urls[idx];
                
                display.set_msg(format!("Extracting {}", url));
                
                let (extracted, _video_id, translation_title) = match extract_video_link(c, url) {
                    Ok((src, vid, title)) => (src, vid, title),
                    Err(e) => {
                        let handle = display.begin(ContentUnit::chunks(format!("Video {}", idx + 1), 0));
                        display.end_err(handle, &format!("Extraction failed: {}", e));
                        display.advance();
                        continue;
                    }
                };

                let m3u8_resp = match c.get(&extracted).send() {
                    Ok(r) => r.error_for_status(),
                    Err(e) => Err(e)
                };
                let m3u8_text = match m3u8_resp {
                    Ok(r) => r.text().unwrap_or_default(),
                    Err(e) => {
                        let handle = display.begin(ContentUnit::chunks(format!("Video {}", idx + 1), 0));
                        display.end_err(handle, &format!("Failed to get m3u8: {}", e));
                        display.advance();
                        continue;
                    }
                };

                let mut chunks = Vec::new();
                for line in m3u8_text.lines() {
                    let line = line.trim();
                    if !line.is_empty() && !line.starts_with('#') {
                        if line.starts_with("http") {
                            chunks.push(line.to_string());
                        } else {
                            let base_url = extracted.rsplit_once('/').map(|(b, _)| b).unwrap_or("");
                            chunks.push(format!("{}/{}", base_url, line));
                        }
                    }
                }

                if chunks.is_empty() {
                    let handle = display.begin(ContentUnit::chunks(format!("Video {}", idx + 1), 0));
                    display.end_err(handle, "No chunks found");
                    display.advance();
                    continue;
                }

                let file_base = format!("{}_{}", folder_name, idx + 1);
                let out_path = Path::new(dir).join(format!("{}.mkv", file_base));
                let handle = display.begin(ContentUnit::chunks(format!("{}.mkv", file_base), chunks.len() as u64));

                let mut out_file = match OpenOptions::new().create(true).write(true).truncate(true).open(&out_path) {
                    Ok(f) => f,
                    Err(e) => {
                        display.end_err(handle, &format!("File error: {}", e));
                        display.advance();
                        continue;
                    }
                };

                let mut success = true;
                for chunk_url in chunks.iter() {
                    let mut attempts = 0;
                    let mut chunk_data: Option<Vec<u8>> = None;
                    while attempts < 3 {
                        match c.get(chunk_url).send() {
                            Ok(r) => {
                                if let Ok(r) = r.error_for_status() {
                                    if let Ok(bytes) = r.bytes() {
                                        chunk_data = Some(bytes.to_vec());
                                        break;
                                    }
                                }
                            }
                            Err(_) => {}
                        }
                        attempts += 1;
                        std::thread::sleep(Duration::from_millis(500));
                    }

                    if let Some(data) = chunk_data {
                        if out_file.write_all(&data).is_err() {
                            success = false;
                            break;
                        }
                        handle.inc(1);
                    } else {
                        success = false;
                        break;
                    }
                }

                if success {
                    display.end_ok(handle, chunks.len() as u64);

                    let tmp_path = out_path.with_extension("tmp.mkv");
                    let status = std::process::Command::new("ffmpeg")
                        .arg("-y")
                        .arg("-i")
                        .arg(&out_path)
                        .arg("-c")
                        .arg("copy")
                        .arg("-metadata:s:a:0")
                        .arg(format!("title={}", translation_title))
                        .arg(&tmp_path)
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .status();

                    if let Ok(st) = status {
                        if st.success() {
                            let _ = std::fs::rename(&tmp_path, &out_path);
                        } else {
                            let _ = std::fs::remove_file(&tmp_path);
                        }
                    }
                } else {
                    display.end_err(handle, "Failed downloading chunks");
                }
                display.advance();
            });
        }
    });

    display.finish("Done");
}

fn extract_video_link(client: &reqwest::blocking::Client, url: &str) -> Result<(String, String, String), String> {
    let re_url = Regex::new(r"/([^/]+)/(\d+)/([a-z0-9]+)").unwrap();
    let caps = re_url.captures(url).ok_or("Invalid URL format")?;
    let v_type = caps.get(1).unwrap().as_str();
    let v_id = caps.get(2).unwrap().as_str();
    let v_hash = caps.get(3).unwrap().as_str();

    let domain = kodik_utils_extract_domain(url).unwrap_or("kodikplayer.com");

    let page_html = client.get(url).send().map_err(|e| e.to_string())?.text().map_err(|e| e.to_string())?;

    let video_id_re = Regex::new(r#"var\s+videoId\s*=\s*["']([^"']+)["']"#).unwrap();
    let video_id = video_id_re.captures(&page_html).and_then(|c| c.get(1)).map(|m| m.as_str()).unwrap_or(v_id);

    let translation_re = Regex::new(r#"var\s+translationTitle\s*=\s*["']([^"']+)["']"#).unwrap();
    let translation_title = translation_re.captures(&page_html).and_then(|c| c.get(1)).map(|m| m.as_str()).unwrap_or("unknown");

    let player_path_re = Regex::new(r#"src="/(assets/js/app\.player_single[^"]*)""#).unwrap();
    let player_path = player_path_re.captures(&page_html).ok_or("No player script found")?.get(1).unwrap().as_str();

    let player_js = client.get(&format!("https://{}/{}", domain, player_path)).send().map_err(|e| e.to_string())?.text().map_err(|e| e.to_string())?;

    let endpoint_re = Regex::new(r#"url:\s*atob\(["']([\w=]+)["']\)"#).unwrap();
    let b64_endpoint = endpoint_re.captures(&player_js).ok_or("No endpoint found")?.get(1).unwrap().as_str();
    let endpoint = String::from_utf8(general_purpose::STANDARD.decode(b64_endpoint).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;

    let form_params = [
        ("type", v_type),
        ("id", v_id),
        ("hash", v_hash),
        ("bad_user", "True"),
        ("info", "{}"),
        ("cdn_is_working", "True"),
    ];

    let gvi_url = format!("https://{}{}", domain, endpoint);
    let resp: KodikResponse = client.post(&gvi_url).form(&form_params).send().map_err(|e| e.to_string())?.json().map_err(|e| e.to_string())?;

    let mut links = Vec::new();
    if let Some(mut v) = resp.links.p720 { links.append(&mut v); }
    if let Some(mut v) = resp.links.p480 { links.append(&mut v); }
    if let Some(mut v) = resp.links.p360 { links.append(&mut v); }

    let encoded_src = &links.first().ok_or("No links found in response")?.src;
    let decoded = decode_src(encoded_src).ok_or("Failed to decode link")?;
    
    Ok((decoded, video_id.to_string(), translation_title.to_string()))
}

fn kodik_utils_extract_domain(url: &str) -> Option<&str> {
    let re = Regex::new(r"^https?://([^/]+)").unwrap();
    re.captures(url).map(|c| c.get(1).unwrap().as_str())
}

fn try_parse_page(dir: &str, url: &str) {
    let c = build_client();
    println!("Fetching {}...", url);
    let resp = match c.get(url).send() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to fetch page: {}", e);
            std::process::exit(1);
        }
    };
    
    let text = match resp.text() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Failed to read page text: {}", e);
            std::process::exit(1);
        }
    };
    
    // Clean text to handle JSON escaped slashes and quotes
    let text = text
        .replace("\\/", "/")
        .replace("\\\"", "\"")
        .replace("\\u002F", "/")
        .replace("\\u0026", "&")
        .replace("\\u003F", "?")
        .replace("\\u003D", "=");
    
    // Extract Kodik URLs
    let re = Regex::new(r#"(?:https?:)?//(?:kodikplayer\.com|kodik\.cc|kodik\.info|aniqit\.com)/[a-z]+/[0-9]+/[a-z0-9]+[^ \t\r\n"'<>`\\]*"#).unwrap();
    
    let mut links = Vec::new();
    for cap in re.captures_iter(&text) {
        let mut link = cap[0].to_string();
        if link.starts_with("//") {
            link = format!("https:{}", link);
        }
        if let Some(idx) = link.find('?') {
            link.truncate(idx);
        }
        links.push(link);
    }
    
    let mut unique_links = Vec::new();
    for link in links {
        if !unique_links.contains(&link) {
            unique_links.push(link);
        }
    }
    let links = unique_links;
    
    if links.is_empty() {
        eprintln!("No Kodik links found on the page.");
        std::process::exit(1);
    }
    
    println!("Found {} episodes/links:", links.len());
    for (i, link) in links.iter().enumerate() {
        let ep = if let Some(idx) = link.find("episode=") {
            let num: String = link[idx + 8..].chars().take_while(|c| c.is_ascii_digit()).collect();
            if num.is_empty() { format!("Episode {}", i + 1) } else { format!("Episode {}", num) }
        } else {
            format!("Video {}", i + 1)
        };
        println!("  {}. {} ({})", i + 1, ep, link);
    }
    
    print!("Download these {} items? [y/N]: ", links.len());
    std::io::stdout().flush().unwrap();
    
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();
    let input = input.trim().to_lowercase();
    
    if input == "y" || input == "yes" {
        download(dir, &links);
    } else {
        println!("Aborted.");
    }
}
