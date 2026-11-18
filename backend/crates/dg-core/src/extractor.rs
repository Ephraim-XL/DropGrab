//! Extractor host (section 5.5), reduced to what this build can honestly provide.
//!
//! The design calls for signed WebAssembly plugins with fetch/log/now host functions. No plugin
//! runtime is in this build, so resolution is a generic HTML and manifest scan:
//!   - a direct media URL (by content type or extension) resolves to one format;
//!   - an HTML page is scanned for `<video>`/`<audio>`/`<source>` URLs and OpenGraph video tags;
//!   - anything else returns EXTRACT_FAILED.
//! Results are cached for 10 minutes (section 5.5). DRM markers refuse the job (DRM_PROTECTED).

use dg_types::ErrorCode;
use serde_json::{json, Value};

use crate::engine::{err, lock, Engine, Res};
use crate::util::now_ms;

const CACHE_MS: i64 = 10 * 60 * 1000;
const MAX_HTML: usize = 2 * 1024 * 1024;

pub struct Resolved {
    pub title: String,
    pub uploader: String,
    pub duration: String,
    pub formats: Vec<Value>,
}

pub async fn resolve(engine: &Engine, page_url: &str) -> Res<Resolved> {
    if let Some((at, v)) = lock(&engine.extractor_cache).get(page_url).cloned() {
        if now_ms() - at < CACHE_MS {
            return Ok(from_json(&v));
        }
    }
    let resp = engine
        .http
        .get(page_url)
        .send()
        .await
        .map_err(|_| err(ErrorCode::NetUnreachable, "could not reach the page"))?;
    let status = resp.status().as_u16();
    if status == 404 || status == 410 {
        return Err(err(ErrorCode::NotFound, "the page is gone"));
    }
    if status == 401 || status == 403 {
        return Err(err(ErrorCode::AuthRequired, "the page needs a login"));
    }
    let ctype = resp.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    let final_url = resp.url().to_string();
    if !ctype.starts_with("text/html") {
        // A direct media URL: one format, the URL itself.
        if ctype.starts_with("video/") || ctype.starts_with("audio/") || is_media_ext(&final_url) {
            let size = resp.content_length().unwrap_or(0);
            let r = Resolved {
                title: crate::downloads::filename_from_url(&final_url).unwrap_or_else(|| "Media".into()),
                uploader: crate::util::host_of(&final_url).unwrap_or_default(),
                duration: String::new(),
                formats: vec![json!({
                    "id": "direct",
                    "label": "Original",
                    "container": ext_of(&final_url),
                    "isAudioOnly": ctype.starts_with("audio/"),
                    "url": final_url,
                    "size": size,
                    "height": 0,
                })],
            };
            cache(engine, page_url, &r);
            return Ok(r);
        }
        return Err(err(ErrorCode::ExtractFailed, "no downloadable media on this link"));
    }
    let body = read_html_body(resp).await.map_err(|_| err(ErrorCode::NetUnreachable, "page body unreadable"))?;
    let body = truncate_on_char_boundary(body, MAX_HTML);
    let lower = body.to_ascii_lowercase();
    if lower.contains("encrypted media") || lower.contains("widevine") || lower.contains("playready") {
        return Err(err(ErrorCode::DrmProtected, "this content is DRM-protected and cannot be saved"));
    }
    let title = og_meta(&body, "og:title").or_else(|| title_tag(&body)).unwrap_or_else(|| "Media".into());
    let mut urls: Vec<String> = Vec::new();
    for tag in ["video", "audio", "source"] {
        for src in attr_values(&body, tag, "src") {
            urls.push(src);
        }
    }
    if let Some(v) = og_meta(&body, "og:video").or_else(|| og_meta(&body, "og:video:url")) {
        urls.push(v);
    }
    let mut formats = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for raw in urls {
        let abs = match absolute(&final_url, &raw) {
            Some(a) => a,
            None => continue,
        };
        if !is_media_ext(&abs) || !seen.insert(abs.clone()) {
            continue;
        }
        formats.push(json!({
            "id": format!("f{}", formats.len() + 1),
            "label": if formats.is_empty() { "Original".to_string() } else { format!("Option {}", formats.len() + 1) },
            "container": ext_of(&abs),
            "isAudioOnly": is_audio_ext(&abs),
            "url": abs,
            "size": 0,
            "height": 0,
        }));
    }
    if formats.is_empty() {
        return Err(err(ErrorCode::ExtractFailed, "no downloadable media found on this page"));
    }
    let r = Resolved {
        title: crate::util::sanitize_filename(&title),
        uploader: crate::util::host_of(&final_url).unwrap_or_default(),
        duration: String::new(),
        formats,
    };
    cache(engine, page_url, &r);
    Ok(r)
}

/// Reads at most `MAX_HTML` bytes of the page body. The read stops as soon as the cap is reached
/// instead of letting `Response::text` buffer the whole response, so an oversized page cannot
/// exhaust memory.
async fn read_html_body(mut resp: reqwest::Response) -> Result<String, reqwest::Error> {
    let mut buf: Vec<u8> = Vec::with_capacity(64 * 1024);
    while buf.len() < MAX_HTML {
        match resp.chunk().await? {
            Some(chunk) => {
                let room = MAX_HTML - buf.len();
                buf.extend_from_slice(&chunk[..chunk.len().min(room)]);
            }
            None => break,
        }
    }
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

/// Cuts `body` to at most `max` bytes, flooring the cut to a character boundary. Indexing a
/// `String` at a byte position inside a multi-byte character panics, so the straddling character
/// is dropped instead of being split.
fn truncate_on_char_boundary(mut body: String, max: usize) -> String {
    if body.len() > max {
        let mut end = max;
        while end > 0 && !body.is_char_boundary(end) {
            end -= 1;
        }
        body.truncate(end);
    }
    body
}

fn cache(engine: &Engine, page_url: &str, r: &Resolved) {
    lock(&engine.extractor_cache).insert(page_url.to_string(), (now_ms(), to_json(r)));
}

fn to_json(r: &Resolved) -> Value {
    json!({ "title": r.title, "uploader": r.uploader, "duration": r.duration, "formats": r.formats })
}

fn from_json(v: &Value) -> Resolved {
    Resolved {
        title: v["title"].as_str().unwrap_or("Media").to_string(),
        uploader: v["uploader"].as_str().unwrap_or("").to_string(),
        duration: v["duration"].as_str().unwrap_or("").to_string(),
        formats: v["formats"].as_array().cloned().unwrap_or_default(),
    }
}

/// Picks a format for a preset (section 5.5: Best, Balanced, Data saver, Audio only).
pub fn pick_format<'a>(formats: &'a [Value], preset: &str) -> Option<&'a Value> {
    if formats.is_empty() {
        return None;
    }
    if preset == "audio" {
        return formats.iter().find(|f| f["isAudioOnly"].as_bool() == Some(true)).or_else(|| formats.first());
    }
    let mut video: Vec<&Value> = formats.iter().filter(|f| f["isAudioOnly"].as_bool() != Some(true)).collect();
    if video.is_empty() {
        return formats.first();
    }
    video.sort_by_key(|f| std::cmp::Reverse(f["height"].as_u64().unwrap_or(0)));
    match preset {
        "data-saver" => video.last().copied(),
        "balanced" => video.iter().find(|f| f["height"].as_u64().map(|h| h <= 1080).unwrap_or(true)).copied(),
        _ => video.first().copied(),
    }
}

pub fn list_sites() -> Value {
    json!({ "sites": [], "mode": "generic-html" })
}

pub fn update_status() -> Value {
    json!({
        "updated": false,
        "version": env!("CARGO_PKG_VERSION"),
        "reason": "signed extractor plugins are not part of this build; the generic resolver is built in",
    })
}

const MEDIA_EXT: &[&str] = &["mp4", "webm", "mkv", "mov", "m4v", "mp3", "m4a", "aac", "ogg", "opus", "flac", "wav"];
const AUDIO_EXT: &[&str] = &["mp3", "m4a", "aac", "ogg", "opus", "flac", "wav"];

fn ext_of(url: &str) -> String {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    path.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default()
}

fn is_media_ext(url: &str) -> bool {
    MEDIA_EXT.contains(&ext_of(url).as_str())
}

fn is_audio_ext(url: &str) -> bool {
    AUDIO_EXT.contains(&ext_of(url).as_str())
}

fn absolute(base: &str, raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.starts_with("https://") || raw.starts_with("http://") {
        return Some(raw.to_string());
    }
    if raw.starts_with("data:") || raw.starts_with("javascript:") || raw.is_empty() {
        return None;
    }
    let base_url = reqwest::Url::parse(base).ok()?;
    base_url.join(raw).ok().map(|u| u.to_string())
}

fn title_tag(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let start = lower.find("<title")?;
    let open_end = lower[start..].find('>')? + start + 1;
    let close = lower[open_end..].find("</title>")? + open_end;
    let t = html[open_end..close].trim();
    if t.is_empty() { None } else { Some(t.to_string()) }
}

/// Returns the content of `<meta property="NAME" content="...">` (either attribute order).
fn og_meta(html: &str, name: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let mut from = 0;
    while let Some(pos) = lower[from..].find("<meta") {
        let start = from + pos;
        let end = lower[start..].find('>').map(|e| start + e)?;
        let tag = &html[start..end];
        let tl = tag.to_ascii_lowercase();
        if tl.contains(&format!("\"{name}\"")) || tl.contains(&format!("'{name}'")) {
            if let Some(c) = attr_in(tag, "content") {
                return Some(c);
            }
        }
        from = end;
    }
    None
}

fn attr_values(html: &str, tag: &str, attr: &str) -> Vec<String> {
    let lower = html.to_ascii_lowercase();
    let mut out = Vec::new();
    let mut from = 0;
    let open = format!("<{tag}");
    while let Some(pos) = lower[from..].find(&open) {
        let start = from + pos;
        let end = match lower[start..].find('>') {
            Some(e) => start + e,
            None => break,
        };
        if let Some(v) = attr_in(&html[start..end], attr) {
            out.push(v);
        }
        from = end;
    }
    out
}

fn attr_in(tag: &str, attr: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let key = format!("{attr}=");
    let mut from = 0;
    while let Some(pos) = lower[from..].find(&key) {
        let idx = from + pos;
        // Must be a whole attribute name: preceded by whitespace.
        let prev = lower[..idx].chars().last();
        if matches!(prev, Some(c) if c.is_whitespace()) {
            let rest = &tag[idx + key.len()..];
            let q = rest.chars().next()?;
            if q == '"' || q == '\'' {
                let inner = &rest[1..];
                let close = inner.find(q)?;
                return Some(inner[..close].replace("&amp;", "&"));
            }
            let end = rest.find(|c: char| c.is_whitespace() || c == '>').unwrap_or(rest.len());
            return Some(rest[..end].to_string());
        }
        from = idx + key.len();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_video_source_and_og() {
        let html = r#"<html><head><title>Lecture 07</title>
            <meta property="og:video" content="/media/l7.mp4"></head>
            <body><video><source src="/media/l7-720.mp4"></video>
            <audio src="https://cdn.test/ep.mp3"></audio></body></html>"#;
        assert_eq!(title_tag(html).as_deref(), Some("Lecture 07"));
        assert_eq!(og_meta(html, "og:video").as_deref(), Some("/media/l7.mp4"));
        assert_eq!(attr_values(html, "source", "src"), vec!["/media/l7-720.mp4"]);
        assert_eq!(absolute("https://ex.test/p/x", "/media/l7.mp4").as_deref(), Some("https://ex.test/media/l7.mp4"));
        assert_eq!(absolute("https://ex.test/", "data:text/plain,x"), None);
    }

    #[test]
    fn preset_selection() {
        let f = vec![
            json!({"url":"a","height":1080,"isAudioOnly":false}),
            json!({"url":"b","height":720,"isAudioOnly":false}),
            json!({"url":"c","height":480,"isAudioOnly":false}),
            json!({"url":"d","height":0,"isAudioOnly":true}),
        ];
        assert_eq!(pick_format(&f, "best").unwrap()["url"], "a");
        assert_eq!(pick_format(&f, "balanced").unwrap()["url"], "a");
        assert_eq!(pick_format(&f, "data-saver").unwrap()["url"], "c");
        assert_eq!(pick_format(&f, "audio").unwrap()["url"], "d");
    }

    #[test]
    fn attr_requires_whole_name() {
        assert_eq!(attr_in(r#"<a data-src="x" src="y">"#, "src").as_deref(), Some("y"));
    }

    /// Regression: `body[..MAX_HTML]` panicked when the byte index landed inside a multi-byte
    /// character. The cut must floor to the previous character boundary.
    #[test]
    fn truncation_never_splits_a_multibyte_char() {
        // "€" is three bytes, so MAX_HTML (2 MiB = 3n + 2) falls two bytes into a character.
        let body = "a".repeat(MAX_HTML - 1) + &"€".repeat(8);
        assert!(body.len() > MAX_HTML);
        assert!(!body.is_char_boundary(MAX_HTML), "test setup must straddle a char");

        let cut = truncate_on_char_boundary(body.clone(), MAX_HTML);
        assert_eq!(cut.len(), MAX_HTML - 1, "cut must end before the straddling char");
        assert!(cut.chars().all(|c| c == 'a'));

        // A body that is already small enough (or ends exactly on the cap) is untouched.
        assert_eq!(truncate_on_char_boundary("abc".into(), MAX_HTML), "abc");
        let exact = "a".repeat(MAX_HTML);
        assert_eq!(truncate_on_char_boundary(exact.clone(), MAX_HTML).len(), MAX_HTML);
    }

    /// Regression: the page body is read with a hard cap, not buffered in full.
    #[tokio::test]
    async fn body_read_stops_at_the_cap() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let port = listener.local_addr().unwrap().port();
        let total = MAX_HTML * 3;
        tokio::spawn(async move {
            let (mut sock, _) = match listener.accept().await {
                Ok(v) => v,
                Err(_) => return,
            };
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let mut req = [0u8; 1024];
            let _ = sock.read(&mut req).await;
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {total}\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\n\r\n"
            );
            if sock.write_all(head.as_bytes()).await.is_err() {
                return;
            }
            // More than the cap; the client must stop reading once the cap is reached.
            let chunk = vec![b'x'; 64 * 1024];
            let mut sent = 0;
            while sent < total {
                if sock.write_all(&chunk).await.is_err() {
                    return;
                }
                sent += chunk.len();
            }
        });

        let url = format!("http://127.0.0.1:{port}/big.html");
        let resp = reqwest::Client::new().get(&url).send().await.expect("request");
        let body = tokio::time::timeout(std::time::Duration::from_secs(10), read_html_body(resp))
            .await
            .expect("reader must return at the cap, not wait for the whole body")
            .expect("read");
        assert_eq!(body.len(), MAX_HTML, "body must be capped at MAX_HTML bytes");
        assert!(body.chars().all(|c| c == 'x'));
    }
}
