use std::error::Error;
fn main() {
    let client = reqwest::blocking::Client::builder()
        .user_agent("test/1.0")
        .build().unwrap();
    let url = "https://gelbooru.com/index.php?page=dapi&s=tag&q=index&json=1&name_pattern=%25kay%25&limit=5";
    match client.get(url).send() {
        Ok(r) => println!("Status: {}", r.status()),
        Err(e) => {
            println!("Error: {}", e);
            let mut src: Option<&dyn Error> = e.source();
            while let Some(s) = src {
                println!("  caused by: {}", s);
                src = s.source();
            }
        }
    }
}
