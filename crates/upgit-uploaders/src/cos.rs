use std::collections::BTreeMap;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use hmac::{Hmac, Mac};
use md5::{Digest, Md5};
use sha1::Sha1;
use upgit_core::{Artifact, Locator, ObjectKey, UploadError, Uploader};

use crate::util::{
    content_type_for, hex_lower, hostname, http_date_gmt, join_host_path, read_bytes, status_error,
};

type HmacSha1 = Hmac<Sha1>;

#[derive(Debug, Clone)]
pub struct CosConfig {
    pub host: String,
    pub secret_id: String,
    pub secret_key: String,
}

#[derive(Debug, Clone)]
pub struct CosUploader {
    config: CosConfig,
}

impl CosUploader {
    pub fn new(config: CosConfig) -> Self {
        Self { config }
    }

    pub fn locator_for(&self, key: &ObjectKey) -> Locator {
        let host = hostname(&self.config.host);
        Locator::new(format!("https://{}", join_host_path(host, key.as_str())))
    }

    fn put_url(&self, key: &ObjectKey) -> String {
        let host = hostname(&self.config.host);
        format!("https://{}/{}", host, key.as_str())
    }

    /// Tencent COS v5 Authorization, with frozen sign/key times for tests.
    pub fn authorization_for(
        &self,
        method: &str,
        uri_path: &str,
        headers: &[(&str, &str)],
        sign_start: i64,
        sign_end: i64,
    ) -> String {
        let sign_time = format!("{sign_start};{sign_end}");
        let key_time = sign_time.clone();
        let sign_key = hex_lower(&hmac_sha1(
            self.config.secret_key.as_bytes(),
            key_time.as_bytes(),
        ));
        let (format_headers, signed_header_list) = gen_format_headers(headers);
        let (format_parameters, signed_parameter_list) = (String::new(), Vec::<String>::new());
        let format_string = format!(
            "{}\n{}\n{}\n{}\n",
            method.to_ascii_lowercase(),
            uri_path,
            format_parameters,
            format_headers
        );
        let hashed = hex_lower(&Sha1::digest(format_string.as_bytes()));
        let string_to_sign = format!("sha1\n{key_time}\n{hashed}\n");
        let signature = hex_lower(&hmac_sha1(sign_key.as_bytes(), string_to_sign.as_bytes()));
        format!(
            "q-sign-algorithm=sha1&q-ak={}&q-sign-time={sign_time}&q-key-time={key_time}&q-header-list={}&q-url-param-list={}&q-signature={signature}",
            self.config.secret_id,
            signed_header_list.join(";"),
            signed_parameter_list.join(";"),
        )
    }
}

impl Uploader for CosUploader {
    fn upload(&self, artifact: &Artifact, key: &ObjectKey) -> Result<Locator, UploadError> {
        let data = read_bytes(artifact)?;
        let content_type = content_type_for(artifact.file_name());
        let content_md5 = STANDARD.encode(Md5::digest(&data));
        let date = http_date_gmt(std::time::SystemTime::now());
        let host = hostname(&self.config.host).to_string();
        let uri_path = format!("/{}", key.as_str());
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let headers = [
            ("Host", host.as_str()),
            ("Content-MD5", content_md5.as_str()),
            ("Content-Type", content_type),
        ];
        let authorization = self.authorization_for("PUT", &uri_path, &headers, now, now + 3600);
        let url = self.put_url(key);
        match ureq::put(&url)
            .set("Date", &date)
            .set("Content-MD5", &content_md5)
            .set("Content-Type", content_type)
            .set("User-Agent", "upgit")
            .set("Authorization", &authorization)
            .send_bytes(&data)
        {
            Ok(_) => Ok(self.locator_for(key)),
            Err(ureq::Error::Status(code, resp)) => {
                let text = resp.into_string().unwrap_or_default();
                Err(status_error("qcloudcos", code, &text))
            }
            Err(e) => Err(UploadError::message(e.to_string())),
        }
    }
}

fn hmac_sha1(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha1::new_from_slice(key).expect("hmac-sha1 key");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

fn gen_format_headers(headers: &[(&str, &str)]) -> (String, Vec<String>) {
    let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut signed = Vec::new();
    for (key, value) in headers {
        if !is_sign_header(key) {
            continue;
        }
        let encoded_key = safe_url_encode(key).to_ascii_lowercase();
        map.entry(encoded_key.clone())
            .or_default()
            .push((*value).to_string());
        signed.push(encoded_key);
    }
    signed.sort();
    let mut pairs = Vec::new();
    for (k, mut vals) in map {
        vals.sort();
        for val in vals {
            pairs.push(format!("{k}={}", safe_url_encode(&val)));
        }
    }
    (pairs.join("&"), signed)
}

fn is_sign_header(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    if key.starts_with("x-cos-") {
        return true;
    }
    matches!(
        key.as_str(),
        "host"
            | "range"
            | "cache-control"
            | "content-disposition"
            | "content-encoding"
            | "content-type"
            | "content-length"
            | "content-md5"
            | "transfer-encoding"
            | "versionid"
            | "expect"
            | "expires"
            | "if-match"
            | "if-modified-since"
            | "if-none-match"
            | "if-unmodified-since"
            | "origin"
            | "access-control-request-method"
            | "access-control-request-headers"
            | "response-content-type"
            | "response-content-language"
            | "response-expires"
            | "response-cache-control"
            | "response-content-disposition"
            | "response-content-encoding"
    )
}

fn encode_uri_component(s: &str) -> String {
    let mut out = String::new();
    for &b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push_str("%20"),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn safe_url_encode(s: &str) -> String {
    encode_uri_component(s)
        .replace('!', "%21")
        .replace('\'', "%27")
        .replace('(', "%28")
        .replace(')', "%29")
        .replace('*', "%2A")
}
