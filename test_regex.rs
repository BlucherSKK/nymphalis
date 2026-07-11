fn main() {
    let re = regex::Regex::new(r#"(?:https?:)?//(?:kodikplayer\.com|kodik\.cc|kodik\.info|aniqit\.com)/(?:video|serial)/\d+/[a-z0-9]+[^\s"'<>`]*"#).unwrap();
    let text = "some text //kodik.cc/serial/123/abc456?episode=1 and https://aniqit.com/video/999/def789/720p?season=1&episode=2\"";
    for cap in re.captures_iter(text) {
        println!("{}", &cap[0]);
    }
}
