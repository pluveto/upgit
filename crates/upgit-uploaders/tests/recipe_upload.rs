//! Drives the shipped HttpRecipeUploader::upload against a local HTTP server.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;
use std::time::Duration;

use upgit_core::{Artifact, ObjectKey, Uploader};
use upgit_uploaders::recipe::{HttpRecipe, HttpRecipeUploader};

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

fn serve_one_json(json: &'static str) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    listener.set_nonblocking(false).expect("blocking listener");
    let addr = listener.local_addr().expect("addr");
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        stream.set_read_timeout(Some(Duration::from_secs(2))).ok();
        let buf = read_http_request(&mut stream);
        let req = String::from_utf8_lossy(&buf);
        assert!(
            req.starts_with("POST "),
            "expected POST from HttpRecipeUploader, got:\n{req}"
        );
        let resp_body = json.as_bytes();
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            resp_body.len()
        );
        stream.write_all(resp.as_bytes()).expect("headers");
        stream.write_all(resp_body).expect("body");
        stream.flush().ok();
    });
    (format!("http://{addr}/upload"), handle)
}

#[test]
fn upload_posts_file_and_returns_json_locator() {
    let (url, server) = serve_one_json(r#"{"data":{"url":"https://cdn.example.com/ok.png"}}"#);
    let recipe = HttpRecipe::from_toml(&format!(
        r#"
[meta]
id = "mock"

[request]
method = "POST"
url = "{url}"

[request.body]
smfile = {{ type = "file" }}

[response]
url = {{ from = "json", path = "data.url" }}
"#
    ))
    .expect("recipe");

    let dir = std::env::temp_dir();
    let file = dir.join(format!("upgit-recipe-upload-{}.png", std::process::id()));
    std::fs::write(&file, b"\x89PNG-fake").expect("write fixture");
    let artifact = Artifact::from_path(&file, None).expect("artifact");
    let key = ObjectKey::parse("ok.png").expect("key");
    let uploader = HttpRecipeUploader::new(recipe, HashMap::new());

    let locator = uploader.upload(&artifact, &key).expect("upload");
    assert_eq!(locator.as_str(), "https://cdn.example.com/ok.png");

    let _ = std::fs::remove_file(&file);
    server.join().expect("server");
}
