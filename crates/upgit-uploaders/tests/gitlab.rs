//! Locator, encoding, explain, and POST-then-PUT for GitlabUploader.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;
use std::time::Duration;

use upgit_core::{Artifact, ObjectKey, Uploader};
use upgit_uploaders::gitlab::{GitlabConfig, GitlabUploader};
use upgit_uploaders::HostCatalog;

fn uploader(public_base: Option<&str>) -> GitlabUploader {
    GitlabUploader::new(GitlabConfig {
        url: "https://gitlab.example.com".into(),
        project: "group/name".into(),
        token: "tok".into(),
        branch: String::new(),
        public_base: public_base.map(str::to_string),
    })
}

fn read_http_request(stream: &mut std::net::TcpStream) -> Vec<u8> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    loop {
        match stream.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => buf.extend_from_slice(&tmp[..n]),
            Err(_) => break,
        }
        let Some(header_end) = buf.windows(4).position(|w| w == b"\r\n\r\n") else {
            continue;
        };
        let headers = String::from_utf8_lossy(&buf[..header_end]);
        let content_len = headers
            .lines()
            .find(|line| line.to_ascii_lowercase().starts_with("content-length:"))
            .and_then(|line| line.split(':').nth(1))
            .and_then(|v| v.trim().parse::<usize>().ok())
            .unwrap_or(0);
        let body_start = header_end + 4;
        if buf.len() >= body_start + content_len {
            break;
        }
    }
    buf
}

#[test]
fn catalog_lists_gitlab_after_github() {
    let hosts = HostCatalog::all();
    assert_eq!(hosts[0].id, "github");
    assert_eq!(hosts[1].id, "gitlab");
    assert_eq!(hosts[1].title, "GitLab");
}

#[test]
fn locator_uses_instance_raw_url_without_public_base() {
    let key = ObjectKey::parse("2024/01/a.png").expect("key");
    assert_eq!(
        uploader(None).locator_for(&key).as_str(),
        "https://gitlab.example.com/group/name/-/raw/main/2024/01/a.png"
    );
}

#[test]
fn locator_uses_public_base_when_set() {
    let key = ObjectKey::parse("2024/01/a.png").expect("key");
    assert_eq!(
        uploader(Some("https://cdn.example.com/"))
            .locator_for(&key)
            .as_str(),
        "https://cdn.example.com/2024/01/a.png"
    );
}

#[test]
fn files_url_percent_encodes_project_and_path() {
    let key = ObjectKey::parse("2024/01/a.png").expect("key");
    assert_eq!(
        uploader(None).files_url(&key),
        "https://gitlab.example.com/api/v4/projects/group%2Fname/repository/files/2024%2F01%2Fa.png"
    );
}

#[test]
fn explain_401_has_no_documentation_url_or_raw_json() {
    let err = uploader(None).explain(
        401,
        r#"{"message":"401 Unauthorized","documentation_url":"https://docs.gitlab.com/ee/api/index.html"}"#,
    );
    let text = err.to_string();
    assert!(text.contains("401"), "got {text}");
    assert!(text.contains("token"), "got {text}");
    assert!(
        !text.contains("documentation_url"),
        "dumped GitLab JSON: {text}"
    );
    assert!(!text.contains('{'), "dumped GitLab JSON: {text}");
    assert!(
        !text.contains("401 Unauthorized"),
        "leaked GitLab body: {text}"
    );
}

fn serve_create_then_update() -> (String, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    listener.set_nonblocking(false).expect("blocking listener");
    let addr = listener.local_addr().expect("addr");
    let handle = thread::spawn(move || {
        let mut requests = Vec::new();
        for i in 0..2 {
            let (mut stream, _) = listener.accept().expect("accept");
            stream.set_read_timeout(Some(Duration::from_secs(2))).ok();
            let buf = read_http_request(&mut stream);
            requests.push(String::from_utf8_lossy(&buf).into_owned());
            let (status, body) = if i == 0 {
                (
                    "400 Bad Request",
                    r#"{"message":"A file with this name already exists"}"#,
                )
            } else {
                ("200 OK", r#"{"file_path":"2024/01/a.png","branch":"main"}"#)
            };
            let resp = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(resp.as_bytes()).expect("write");
            stream.flush().ok();
        }
        requests
    });
    (format!("http://{addr}"), handle)
}

#[test]
fn upload_posts_then_puts_when_file_exists() {
    let (origin, server) = serve_create_then_update();
    let uploader = GitlabUploader::new(GitlabConfig {
        url: origin,
        project: "group/name".into(),
        token: "tok".into(),
        branch: "main".into(),
        public_base: Some("https://cdn.example.com/".into()),
    });

    let dir = std::env::temp_dir();
    let file = dir.join(format!("upgit-gitlab-upload-{}.png", std::process::id()));
    std::fs::write(&file, b"\x89PNG-fake").expect("write fixture");
    let artifact = Artifact::from_path(&file, None).expect("artifact");
    let key = ObjectKey::parse("2024/01/a.png").expect("key");

    let locator = uploader.upload(&artifact, &key).expect("upload");
    assert_eq!(locator.as_str(), "https://cdn.example.com/2024/01/a.png");

    let _ = std::fs::remove_file(&file);
    let requests = server.join().expect("server");
    assert_eq!(requests.len(), 2, "expected POST then PUT");
    assert!(
        requests[0].starts_with("POST "),
        "first request should be create, got:\n{}",
        requests[0]
    );
    assert!(
        requests[1].starts_with("PUT "),
        "second request should be update, got:\n{}",
        requests[1]
    );
    for req in &requests {
        assert!(
            req.contains("PRIVATE-TOKEN: tok"),
            "missing PRIVATE-TOKEN:\n{req}"
        );
        assert!(
            req.contains("/api/v4/projects/group%2Fname/repository/files/2024%2F01%2Fa.png"),
            "project/path must stay percent-encoded:\n{req}"
        );
        assert!(
            req.contains("\"encoding\":\"base64\""),
            "body must send base64 encoding:\n{req}"
        );
    }
}
