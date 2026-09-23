pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFFFFFFu32;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            if (crc & 1) != 0 {
                crc = (crc >> 1) ^ 0xEDB88320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

pub fn create_stored_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut local_parts = Vec::new();
    let mut central_parts = Vec::new();
    let mut offset = 0u32;
    let mut central_size = 0u32;

    for (name, data) in files {
        let name_bytes = name.as_bytes();
        let name_len = name_bytes.len() as u16;
        let size = data.len() as u32;
        let checksum = crc32(data);

        // Local header (30 bytes + name + data)
        let mut local_header = Vec::with_capacity(30 + name_bytes.len() + data.len());
        local_header.extend_from_slice(&0x04034b50u32.to_le_bytes());
        local_header.extend_from_slice(&20u16.to_le_bytes());
        local_header.extend_from_slice(&0x0800u16.to_le_bytes()); // UTF-8
        local_header.extend_from_slice(&0u16.to_le_bytes());      // Store
        local_header.extend_from_slice(&0u16.to_le_bytes());      // Time
        local_header.extend_from_slice(&0u16.to_le_bytes());      // Date
        local_header.extend_from_slice(&checksum.to_le_bytes());
        local_header.extend_from_slice(&size.to_le_bytes());
        local_header.extend_from_slice(&size.to_le_bytes());
        local_header.extend_from_slice(&name_len.to_le_bytes());
        local_header.extend_from_slice(&0u16.to_le_bytes());
        local_header.extend_from_slice(name_bytes);
        local_header.extend_from_slice(data);

        // Central directory header (46 bytes + name)
        let mut central_header = Vec::with_capacity(46 + name_bytes.len());
        central_header.extend_from_slice(&0x02014b50u32.to_le_bytes());
        central_header.extend_from_slice(&20u16.to_le_bytes());
        central_header.extend_from_slice(&20u16.to_le_bytes());
        central_header.extend_from_slice(&0x0800u16.to_le_bytes());
        central_header.extend_from_slice(&0u16.to_le_bytes());
        central_header.extend_from_slice(&0u16.to_le_bytes());
        central_header.extend_from_slice(&0u16.to_le_bytes());
        central_header.extend_from_slice(&checksum.to_le_bytes());
        central_header.extend_from_slice(&size.to_le_bytes());
        central_header.extend_from_slice(&size.to_le_bytes());
        central_header.extend_from_slice(&name_len.to_le_bytes());
        central_header.extend_from_slice(&0u16.to_le_bytes());
        central_header.extend_from_slice(&0u16.to_le_bytes());
        central_header.extend_from_slice(&0u16.to_le_bytes());
        central_header.extend_from_slice(&0u16.to_le_bytes());
        central_header.extend_from_slice(&0u32.to_le_bytes());
        central_header.extend_from_slice(&offset.to_le_bytes());
        central_header.extend_from_slice(name_bytes);

        offset += local_header.len() as u32;
        central_size += central_header.len() as u32;

        local_parts.push(local_header);
        central_parts.push(central_header);
    }

    let mut out = Vec::new();
    for part in local_parts {
        out.extend(part);
    }
    let central_offset = out.len() as u32;
    for part in central_parts {
        out.extend(part);
    }

    out.extend_from_slice(&0x06054b50u32.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&central_size.to_le_bytes());
    out.extend_from_slice(&central_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());

    out
}

pub fn escape_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

pub fn clean_xhtml(html: &str) -> String {
    let mut out = html.to_string();
    out = out.replace("<br>", "<br />");
    out = out.replace("<br/>", "<br />");
    out = out.replace("<hr>", "<hr />");
    out = out.replace("<hr/>", "<hr />");

    out = out
        .replace("&nbsp;", "\u{00A0}")
        .replace("&mdash;", "—")
        .replace("&ndash;", "–")
        .replace("&laquo;", "«")
        .replace("&raquo;", "»")
        .replace("&hellip;", "…")
        .replace("&bull;", "•")
        .replace("&ldquo;", "“")
        .replace("&rdquo;", "”")
        .replace("&lsquo;", "‘")
        .replace("&rsquo;", "’")
        .replace("&copy;", "©")
        .replace("&reg;", "®")
        .replace("&trade;", "™");

    let mut result = String::with_capacity(out.len());
    let mut pos = 0;
    while let Some(img_start) = out[pos..].find("<img") {
        let abs_start = pos + img_start;
        result.push_str(&out[pos..abs_start]);
        if let Some(tag_end) = out[abs_start..].find('>') {
            let tag = &out[abs_start..abs_start + tag_end + 1];
            if tag.ends_with("/>") {
                result.push_str(tag);
            } else {
                result.push_str(&tag[..tag.len() - 1]);
                result.push_str(" />");
            }
            pos = abs_start + tag_end + 1;
        } else {
            result.push_str(&out[abs_start..]);
            pos = out.len();
            break;
        }
    }
    result.push_str(&out[pos..]);

    result
}

pub fn html_to_plain_text(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    let mut current_tag = String::new();

    let mut chars = html.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '<' {
            in_tag = true;
            current_tag.clear();
        } else if c == '>' {
            in_tag = false;
            let tag_lower = current_tag.to_lowercase();
            if tag_lower == "/p" || tag_lower.starts_with("/div") {
                out.push_str("\n\n");
            } else if tag_lower == "br" || tag_lower == "br/" || tag_lower == "br /" {
                out.push('\n');
            }
        } else if in_tag {
            current_tag.push(c);
        } else {
            out.push(c);
        }
    }

    unescape_html_entities(&out).trim().to_string()
}

fn unescape_html_entities(s: &str) -> String {
    s.replace("&quot;", "\"")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
        .replace("&mdash;", "—")
        .replace("&ndash;", "–")
        .replace("&laquo;", "«")
        .replace("&raquo;", "»")
        .replace("&hellip;", "…")
}

pub fn build_epub(
    title: &str,
    identifier: &str,
    content_html: &str,
    images: &[(&str, &str, &[u8])],
) -> Vec<u8> {
    let title_escaped = escape_xml(title);
    let id_escaped = escape_xml(identifier);

    let container_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
<rootfiles><rootfile full-path="OEBPS/package.opf" media-type="application/oebps-package+xml" />
</rootfiles></container>"#;

    let mut manifest_images = String::new();
    for (img_id, img_path, _) in images {
        let media_type = if img_path.ends_with(".png") {
            "image/png"
        } else if img_path.ends_with(".webp") {
            "image/webp"
        } else if img_path.ends_with(".gif") {
            "image/gif"
        } else {
            "image/jpeg"
        };
        manifest_images.push_str(&format!(
            r#"<item id="{}" href="{}" media-type="{}" />"#,
            escape_xml(img_id),
            escape_xml(img_path),
            media_type
        ));
        manifest_images.push('\n');
    }

    let package_opf = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="book-id" xml:lang="ru">
<metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
<dc:identifier id="book-id">{}</dc:identifier>
<dc:title>{}</dc:title>
<dc:language>ru</dc:language>
<dc:publisher>Desu.Me</dc:publisher>
</metadata>
<manifest>
<item id="chapter" href="content.xhtml" media-type="application/xhtml+xml" />
<item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav" />
<item id="style" href="style.css" media-type="text/css" />
{}
</manifest>
<spine>
<itemref idref="chapter" />
</spine>
</package>"#,
        id_escaped, title_escaped, manifest_images
    );

    let nav_xhtml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" xml:lang="ru">
<head><title>Оглавление</title></head>
<body>
<nav epub:type="toc" id="toc">
<h1>Оглавление</h1>
<ol><li><a href="content.xhtml">{}</a></li></ol>
</nav>
</body>
</html>"#,
        title_escaped
    );

    let content_xhtml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" xml:lang="ru">
<head><title>{}</title><link rel="stylesheet" type="text/css" href="style.css" /></head>
<body>
<section epub:type="chapter">
<h1>{}</h1>
{}
</section>
</body>
</html>"#,
        title_escaped, title_escaped, content_html
    );

    let style_css = r#"body{margin:5%;font-family:serif;line-height:1.6;color:#222}
h1{margin:0 0 1.5em;font-size:1.45em;line-height:1.3}
p{margin:.75em 0;text-indent:1.25em}
a{color:#315b8a}
.chapter-image{margin:1.5em 0;text-align:center}
.chapter-image img{max-width:100%;height:auto}
blockquote{margin:1em 1.5em}
hr{border:0;border-top:1px solid #aaa}"#;

    let mut files: Vec<(&str, &[u8])> = Vec::new();
    files.push(("mimetype", b"application/epub+zip"));
    files.push(("META-INF/container.xml", container_xml.as_bytes()));
    files.push(("OEBPS/package.opf", package_opf.as_bytes()));
    files.push(("OEBPS/nav.xhtml", nav_xhtml.as_bytes()));
    files.push(("OEBPS/content.xhtml", content_xhtml.as_bytes()));
    files.push(("OEBPS/style.css", style_css.as_bytes()));

    let mut owned_paths = Vec::new();
    for (_, path, _) in images {
        owned_paths.push(format!("OEBPS/{}", path));
    }
    for (i, (_, _, bytes)) in images.iter().enumerate() {
        files.push((owned_paths[i].as_str(), bytes));
    }

    create_stored_zip(&files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crc32() {
        assert_eq!(crc32(b"123456789"), 0xCBF43926);
    }

    #[test]
    fn test_create_stored_zip() {
        let zip = create_stored_zip(&[
            ("hello.txt", b"Hello, World!"),
            ("sub/dir.txt", b"Subdir text"),
        ]);
        assert!(zip.len() > 100);
        assert_eq!(&zip[0..4], &[0x50, 0x4b, 0x03, 0x04]);
    }

    #[test]
    fn test_html_to_plain_text() {
        let html = "<p><i>Hello</i> &amp; <b>World!</b></p><p>Second &laquo;paragraph&raquo;</p>";
        let plain = html_to_plain_text(html);
        assert_eq!(plain, "Hello & World!\n\nSecond «paragraph»");
    }

    #[test]
    fn test_build_epub() {
        let epub = build_epub("Test Title", "urn:desu:test", "<p>Hello</p>", &[]);
        assert!(epub.len() > 500);
        assert_eq!(&epub[0..4], &[0x50, 0x4b, 0x03, 0x04]);
    }

    #[test]
    fn test_clean_xhtml() {
        let html = "<p>Text&nbsp;with&mdash;dashes<br>and <img src=\"foo.jpg\">.</p>";
        let cleaned = clean_xhtml(html);
        assert_eq!(
            cleaned,
            "<p>Text\u{00A0}with—dashes<br />and <img src=\"foo.jpg\" />.</p>"
        );
    }
}
