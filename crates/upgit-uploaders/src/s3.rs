use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use upgit_core::{Artifact, Locator, ObjectKey, UploadError, Uploader};

use crate::util::{
    amz_date, collapse_slash_runs, content_type_for, could_not_reach, hex_lower, host_of, hostname,
    join_host_path, read_bytes, remote_http_error, xml_code_and_message,
};

type HmacSha256 = Hmac<Sha256>;

const DEFAULT_URL_FORMAT: &str = "{endpoint}/{bucket}/{path}";

#[derive(Debug, Clone)]
pub struct S3Config {
    pub region: String,
    pub bucket_name: String,
    pub access_key: String,
    pub secret_key: String,
    pub endpoint: String,
    pub url_format: String,
    /// Public URL prefix (CDN). Empty: `locator_for` uses `url_format`. PUT still uses `endpoint`.
    pub host: String,
}

#[derive(Debug, Clone)]
pub struct S3Uploader {
    config: S3Config,
}

impl S3Uploader {
    pub fn new(mut config: S3Config) -> Self {
        if config.url_format.trim().is_empty() {
            config.url_format = DEFAULT_URL_FORMAT.to_string();
        }
        Self { config }
    }

    pub fn locator_for(&self, key: &ObjectKey) -> Locator {
        let host = self.config.host.trim();
        if !host.is_empty() {
            return Locator::new(join_host_path(host, key.as_str()));
        }
        let endpoint = self.config.endpoint.trim().trim_end_matches('/');
        let raw = self
            .config
            .url_format
            .replace("{endpoint}", endpoint)
            .replace("{bucket}", self.config.bucket_name.trim_matches('/'))
            .replace("{path}", key.as_str().trim_start_matches('/'));
        Locator::new(collapse_slash_runs(&raw))
    }

    fn put_url(&self, key: &ObjectKey) -> String {
        let endpoint = self.config.endpoint.trim().trim_end_matches('/');
        let endpoint = if endpoint.contains("://") {
            endpoint.to_string()
        } else {
            format!("https://{endpoint}")
        };
        collapse_slash_runs(&format!(
            "{}/{}/{}",
            endpoint,
            self.config.bucket_name.trim_matches('/'),
            uri_encode(key.as_str(), false)
        ))
    }

    fn canonical_uri(&self, key: &ObjectKey) -> String {
        format!(
            "/{}/{}",
            uri_encode(self.config.bucket_name.trim_matches('/'), false),
            uri_encode(key.as_str(), false)
        )
    }

    /// AWS SigV4 Authorization header for a frozen request (used by upload and tests).
    pub fn sign_request(
        &self,
        method: &str,
        canonical_uri: &str,
        headers: &[(&str, &str)],
        payload_hash: &str,
        amz_date: &str,
    ) -> String {
        sign_v4(SignV4 {
            method,
            canonical_uri,
            query: "",
            headers,
            payload_hash,
            amz_date,
            region: &self.config.region,
            service: "s3",
            access_key: &self.config.access_key,
            secret_key: &self.config.secret_key,
        })
    }

    /// Status, bucket, and S3's XML Code/Message if present. Does not guess a cause.
    pub fn explain(&self, status: u16, body: &str) -> UploadError {
        let bucket = self.config.bucket_name.trim_matches('/');
        remote_http_error(
            "S3",
            status,
            &format!("bucket `{bucket}`"),
            xml_code_and_message(body).as_deref(),
            "Check [uploaders.s3] region, bucket_name, access_key, secret_key, and endpoint.",
        )
    }
}

impl Uploader for S3Uploader {
    fn upload(&self, artifact: &Artifact, key: &ObjectKey) -> Result<Locator, UploadError> {
        let data = read_bytes(artifact)?;
        let content_type = content_type_for(artifact.file_name());
        let payload_hash = hex_lower(&Sha256::digest(&data));
        let now = std::time::SystemTime::now();
        let amz = amz_date(now);
        let host = hostname(&self.config.endpoint).to_string();
        let canonical_uri = self.canonical_uri(key);
        let headers = [
            ("content-type", content_type),
            ("host", host.as_str()),
            ("x-amz-content-sha256", payload_hash.as_str()),
            ("x-amz-date", amz.as_str()),
        ];
        let authorization = self.sign_request("PUT", &canonical_uri, &headers, &payload_hash, &amz);
        let url = self.put_url(key);
        match ureq::put(&url)
            .set("Authorization", &authorization)
            .set("Content-Type", content_type)
            .set("x-amz-content-sha256", &payload_hash)
            .set("x-amz-date", &amz)
            .send_bytes(&data)
        {
            Ok(_) => Ok(self.locator_for(key)),
            Err(ureq::Error::Status(code, resp)) => {
                let text = resp.into_string().unwrap_or_default();
                Err(self.explain(code, &text))
            }
            Err(e) => Err(could_not_reach("S3", host_of(&self.config.endpoint), e)),
        }
    }
}

struct SignV4<'a> {
    method: &'a str,
    canonical_uri: &'a str,
    query: &'a str,
    headers: &'a [(&'a str, &'a str)],
    payload_hash: &'a str,
    amz_date: &'a str,
    region: &'a str,
    service: &'a str,
    access_key: &'a str,
    secret_key: &'a str,
}

fn sign_v4(req: SignV4<'_>) -> String {
    let mut hdrs: Vec<(String, String)> = req
        .headers
        .iter()
        .map(|(k, v)| (k.to_ascii_lowercase(), v.trim().to_string()))
        .collect();
    hdrs.sort_by(|a, b| a.0.cmp(&b.0));
    let mut canonical_headers = String::new();
    let mut signed_names: Vec<String> = Vec::new();
    for (name, value) in &hdrs {
        canonical_headers.push_str(name);
        canonical_headers.push(':');
        canonical_headers.push_str(value);
        canonical_headers.push('\n');
        signed_names.push(name.clone());
    }
    let signed_headers = signed_names.join(";");
    let canonical_request = format!(
        "{}\n{}\n{}\n{canonical_headers}\n{signed_headers}\n{}",
        req.method, req.canonical_uri, req.query, req.payload_hash
    );
    let datestamp = req.amz_date.get(..8).unwrap_or(req.amz_date);
    let credential_scope = format!("{datestamp}/{}/{}/aws4_request", req.region, req.service);
    let hashed_canonical = hex_lower(&Sha256::digest(canonical_request.as_bytes()));
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{}\n{credential_scope}\n{hashed_canonical}",
        req.amz_date
    );
    let signing_key = signing_key(req.secret_key, datestamp, req.region, req.service);
    let signature = hex_lower(&hmac_sha256(&signing_key, string_to_sign.as_bytes()));
    format!(
        "AWS4-HMAC-SHA256 Credential={}/{credential_scope},SignedHeaders={signed_headers},Signature={signature}",
        req.access_key
    )
}

fn signing_key(secret: &str, datestamp: &str, region: &str, service: &str) -> Vec<u8> {
    let k_date = hmac_sha256(format!("AWS4{secret}").as_bytes(), datestamp.as_bytes());
    let k_region = hmac_sha256(&k_date, region.as_bytes());
    let k_service = hmac_sha256(&k_region, service.as_bytes());
    hmac_sha256(&k_service, b"aws4_request")
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("hmac-sha256 key");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

fn uri_encode(s: &str, encode_slash: bool) -> String {
    let mut out = String::new();
    for &b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b'/' if !encode_slash => out.push('/'),
            _ => {
                out.push('%');
                out.push(
                    char::from_digit((b >> 4) as u32, 16)
                        .unwrap()
                        .to_ascii_uppercase(),
                );
                out.push(
                    char::from_digit((b & 0xf) as u32, 16)
                        .unwrap()
                        .to_ascii_uppercase(),
                );
            }
        }
    }
    out
}
