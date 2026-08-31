use std::time::{SystemTime, UNIX_EPOCH};

use upgit_core::{Artifact, UploadError};

pub(crate) fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0xf) as usize] as char);
    }
    out
}

pub(crate) struct UtcParts {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub min: u32,
    pub sec: u32,
    pub unix_days: i64,
}

pub(crate) fn utc_parts(t: SystemTime) -> UtcParts {
    let secs = t
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let unix_days = (secs / 86400) as i64;
    let tod = secs % 86400;
    let (year, month, day) = civil_from_unix_days(unix_days);
    UtcParts {
        year,
        month,
        day,
        hour: (tod / 3600) as u32,
        min: ((tod % 3600) / 60) as u32,
        sec: (tod % 60) as u32,
        unix_days,
    }
}

/// HTTP-date (RFC 1123 / IMF-fixdate) in GMT.
pub(crate) fn http_date_gmt(t: SystemTime) -> String {
    const WEEKDAYS: [&str; 7] = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let p = utc_parts(t);
    format!(
        "{}, {:02} {} {:04} {:02}:{:02}:{:02} GMT",
        WEEKDAYS[p.unix_days.rem_euclid(7) as usize],
        p.day,
        MONTHS[(p.month - 1) as usize],
        p.year,
        p.hour,
        p.min,
        p.sec
    )
}

pub(crate) fn amz_date(t: SystemTime) -> String {
    let p = utc_parts(t);
    format!(
        "{:04}{:02}{:02}T{:02}{:02}{:02}Z",
        p.year, p.month, p.day, p.hour, p.min, p.sec
    )
}

fn civil_from_unix_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m, d)
}

pub(crate) fn content_type_for(name: &str) -> &'static str {
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "tif" | "tiff" => "image/tiff",
        "pdf" => "application/pdf",
        "txt" => "text/plain",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" => "application/javascript",
        "json" => "application/json",
        "xml" => "application/xml",
        "mp4" => "video/mp4",
        "mp3" => "audio/mpeg",
        "zip" => "application/zip",
        _ => "application/octet-stream",
    }
}

pub(crate) fn read_bytes(artifact: &Artifact) -> Result<Vec<u8>, UploadError> {
    let path = artifact
        .path()
        .ok_or_else(|| UploadError::message("artifact has no local path; cannot upload bytes"))?;
    std::fs::read(path).map_err(|e| UploadError::message(e.to_string()))
}

pub(crate) fn join_host_path(host: &str, key: &str) -> String {
    let host = host.trim().trim_end_matches('/');
    let key = key.trim_start_matches('/');
    format!("{host}/{key}")
}

pub(crate) fn hostname<'a>(host: &'a str) -> &'a str {
    host.trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/')
}

pub(crate) fn collapse_slash_runs(url: &str) -> String {
    let (prefix, rest) = match url.find("://") {
        Some(i) => (&url[..i + 3], &url[i + 3..]),
        None => ("", url),
    };
    let mut out = String::from(prefix);
    let mut prev_slash = false;
    for c in rest.chars() {
        if c == '/' {
            if !prev_slash {
                out.push('/');
            }
            prev_slash = true;
        } else {
            prev_slash = false;
            out.push(c);
        }
    }
    out
}

/// Host part of an HTTP URL (or a bare host), without scheme, path, or trailing slash.
pub(crate) fn host_of(url: &str) -> &str {
    let rest = url
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    let rest = rest.split_once('/').map(|(h, _)| h).unwrap_or(rest);
    rest.trim_end_matches('/')
}

pub(crate) fn one_line(s: &str, max: usize) -> String {
    let line = s.lines().next().unwrap_or("").trim();
    let n = line.chars().count();
    if n <= max {
        line.to_string()
    } else {
        let mut out: String = line.chars().take(max).collect();
        out.push('…');
        out
    }
}

/// First line of a JSON string field, truncated. Never returns the whole body.
pub(crate) fn json_string_field(body: &str, field: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(body.trim()).ok()?;
    value.get(field)?.as_str().map(|s| one_line(s, 120))
}

pub(crate) fn xml_error_summary(uploader: &str, status: u16, body: &str) -> String {
    if let Some(code) = xml_text(body, "Code").filter(|s| !s.is_empty()) {
        return format!(
            "{uploader} upload failed (HTTP {status}): {}",
            one_line(code, 80)
        );
    }
    if let Some(msg) = xml_text(body, "Message").filter(|s| !s.is_empty()) {
        return format!(
            "{uploader} upload failed (HTTP {status}): {}",
            one_line(msg, 120)
        );
    }
    format!("{uploader} upload failed (HTTP {status}).")
}

pub(crate) fn xml_text<'a>(body: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let lower = body.to_ascii_lowercase();
    let start = lower.find(&open.to_ascii_lowercase())? + open.len();
    let end_rel = lower.get(start..)?.find(&close.to_ascii_lowercase())?;
    Some(body.get(start..start + end_rel)?.trim())
}

pub(crate) fn looks_like_rate_limit(body: &str) -> bool {
    let b = body.to_ascii_lowercase();
    b.contains("rate limit") || b.contains("ratelimit") || b.contains("too many requests")
}

pub(crate) fn looks_like_signature_error(body: &str) -> bool {
    let b = body.to_ascii_lowercase();
    b.contains("signaturedoesnotmatch")
        || b.contains("invalidaccesskey")
        || b.contains("signature does not match")
        || b.contains("invalid signature")
        || b.contains("incorrect signature")
        || b.contains("authorizationheader")
}

pub(crate) fn looks_like_missing_bucket(body: &str) -> bool {
    let b = body.to_ascii_lowercase();
    b.contains("nosuchbucket")
        || b.contains("no such bucket")
        || b.contains("the specified bucket does not exist")
}

fn transport_io(err: &ureq::Error) -> String {
    match err {
        ureq::Error::Status(code, _) => format!("HTTP {code}"),
        ureq::Error::Transport(t) => {
            let mut detail = t
                .message()
                .map(str::to_string)
                .unwrap_or_else(|| t.kind().to_string());
            let mut src = std::error::Error::source(t);
            while let Some(s) = src {
                detail = s.to_string();
                src = s.source();
            }
            detail
        }
    }
}

pub(crate) fn could_not_reach(uploader: &str, host: &str, err: ureq::Error) -> UploadError {
    UploadError::new(
        uploader,
        format!("could not reach {host}: {}", transport_io(&err)),
        String::new(),
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn http_date_known_unix() {
        let t = UNIX_EPOCH + Duration::from_secs(1_643_630_400);
        assert_eq!(http_date_gmt(t), "Mon, 31 Jan 2022 12:00:00 GMT");
        assert_eq!(amz_date(t), "20220131T120000Z");
    }

    #[test]
    fn http_date_aws_example() {
        let t = UNIX_EPOCH + Duration::from_secs(1_369_353_600);
        assert_eq!(http_date_gmt(t), "Fri, 24 May 2013 00:00:00 GMT");
        assert_eq!(amz_date(t), "20130524T000000Z");
    }

    #[test]
    fn host_of_strips_scheme_and_path() {
        assert_eq!(
            host_of("https://api.github.com/repos/a/b"),
            "api.github.com"
        );
        assert_eq!(
            host_of("s3.us-west-2.amazonaws.com"),
            "s3.us-west-2.amazonaws.com"
        );
        assert_eq!(host_of("https://upload.qiniup.com"), "upload.qiniup.com");
    }

    #[test]
    fn json_string_field_is_one_line_not_the_body() {
        let body = r#"{"message":"Not Found","documentation_url":"https://docs.github.com/rest"}"#;
        assert_eq!(
            json_string_field(body, "message").as_deref(),
            Some("Not Found")
        );
        assert!(json_string_field(body, "documentation_url")
            .unwrap()
            .starts_with("https://"));
    }
}
