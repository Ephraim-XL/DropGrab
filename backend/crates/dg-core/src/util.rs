//! Small helpers shared across services: time, filename sanitizing, kinds, human-readable times.

use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

pub fn now_s() -> i64 {
    now_ms() / 1000
}

pub fn random_hex(bytes: usize) -> String {
    use rand::RngCore;
    let mut buf = vec![0u8; bytes];
    rand::thread_rng().fill_bytes(&mut buf);
    hex::encode(buf)
}

/// Sanitizes a file name received from or proposed by a peer (section 4.10, 4.4 path rules).
/// Strips directories, replaces illegal characters with `_`, rejects `..`, reserved Windows
/// names, and empty names.
pub fn sanitize_filename(raw: &str) -> String {
    let base = raw.rsplit(['/', '\\']).next().unwrap_or("");
    let mut out: String = base
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    out = out.trim().trim_end_matches(['.', ' ']).to_string();
    if out.is_empty() || out == "." || out == ".." {
        out = "download".into();
    }
    let stem = out.split('.').next().unwrap_or("").to_ascii_uppercase();
    const RESERVED: &[&str] = &[
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "LPT1", "LPT2", "LPT3",
    ];
    if RESERVED.contains(&stem.as_str()) {
        out = format!("_{out}");
    }
    if out.len() > 200 {
        let ext = out.rsplit_once('.').map(|(_, e)| e.to_string()).filter(|e| e.len() <= 10);
        let mut cut: String = out.chars().take(180).collect();
        if let Some(e) = ext {
            cut.push('.');
            cut.push_str(&e);
        }
        out = cut;
    }
    out
}

/// Returns a path inside `dir` for `name` that does not exist yet: `name (1).ext` (section 4.10).
pub fn unique_path(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
    let candidate = dir.join(name);
    if !candidate.exists() {
        return candidate;
    }
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s.to_string(), format!(".{e}")),
        _ => (name.to_string(), String::new()),
    };
    for n in 1..100_000 {
        let c = dir.join(format!("{stem} ({n}){ext}"));
        if !c.exists() {
            return c;
        }
    }
    dir.join(format!("{stem}-{}{ext}", random_hex(4)))
}

/// Item kind from a file name, matching the library `kind` values in section 7.1.
pub fn kind_from_name(name: &str) -> &'static str {
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "mp4" | "mkv" | "mov" | "webm" | "avi" | "m4v" | "ts" => "video",
        "mp3" | "m4a" | "aac" | "flac" | "ogg" | "opus" | "wav" => "audio",
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "heic" | "bmp" | "svg" => "image",
        "pdf" | "doc" | "docx" | "txt" | "md" | "rtf" | "odt" | "xls" | "xlsx" | "ppt" | "pptx" | "csv" => {
            "document"
        }
        "zip" | "rar" | "7z" | "tar" | "gz" | "xz" | "bz2" => "archive",
        "apk" | "ipa" | "exe" | "dmg" | "msi" | "deb" | "rpm" | "appimage" | "iso" => "app",
        _ => "other",
    }
}

pub fn mime_from_name(name: &str) -> &'static str {
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "mp4" => "video/mp4",
        "mkv" => "video/x-matroska",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        "m4a" => "audio/mp4",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        "txt" => "text/plain",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        _ => "application/octet-stream",
    }
}

/// Human "when" string used by the UI lists (`today`, `yesterday`, `3 days ago`).
pub fn when_label(ts_s: i64) -> String {
    let now = now_s();
    let diff = (now - ts_s).max(0);
    if diff < 60 {
        "just now".into()
    } else if diff < 3600 {
        format!("{} min ago", diff / 60)
    } else if diff < 86_400 {
        "today".into()
    } else if diff < 2 * 86_400 {
        "yesterday".into()
    } else {
        format!("{} days ago", diff / 86_400)
    }
}

/// Section 4.1 display form: first 8 groups of 4 hex characters, uppercase.
pub fn display_fingerprint(fp_hex: &str) -> String {
    let upper: String = fp_hex.to_ascii_uppercase().chars().take(32).collect();
    upper
        .as_bytes()
        .chunks(4)
        .map(|c| std::str::from_utf8(c).unwrap_or(""))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Host part of a URL for "Downloaded from example.org" (no scheme, no path, no credentials).
pub fn host_of(url: &str) -> Option<String> {
    let rest = url.split_once("://").map(|(_, r)| r)?;
    let authority = rest.split(['/', '?', '#']).next()?;
    let host = authority.rsplit('@').next()?;
    let host = host.split(':').next()?;
    if host.is_empty() {
        None
    } else {
        Some(host.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_blocks_traversal_and_reserved() {
        assert_eq!(sanitize_filename("../../etc/passwd"), "passwd");
        assert_eq!(sanitize_filename("..\\..\\boot.ini"), "boot.ini");
        assert_eq!(sanitize_filename(".."), "download");
        assert_eq!(sanitize_filename("a:b*c?.txt"), "a_b_c_.txt");
        assert_eq!(sanitize_filename("CON.txt"), "_CON.txt");
        assert_eq!(sanitize_filename(""), "download");
        assert!(!sanitize_filename("/abs/path").contains('/'));
    }

    #[test]
    fn fingerprint_display_is_eight_groups() {
        let fp = "4f3a91c27d08b6e1".repeat(4);
        let shown = display_fingerprint(&fp);
        assert_eq!(shown.split(' ').count(), 8);
        assert!(shown.split(' ').all(|g| g.len() == 4));
    }

    #[test]
    fn host_extraction() {
        assert_eq!(host_of("https://example.org/a/b?c=d").as_deref(), Some("example.org"));
        assert_eq!(host_of("http://user:pw@h.test:8080/x").as_deref(), Some("h.test"));
        assert_eq!(host_of("nope"), None);
    }

    #[test]
    fn kinds() {
        assert_eq!(kind_from_name("Lecture.MP4"), "video");
        assert_eq!(kind_from_name("slides.pdf"), "document");
        assert_eq!(kind_from_name("x.bin"), "other");
    }
}
