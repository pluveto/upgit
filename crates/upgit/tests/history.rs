use upgit::record_history;

#[test]
fn record_history_writes_json_line_with_url() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("history.log");
    record_history(
        &path,
        "https://raw.githubusercontent.com/user/repo/main/a.png",
        "https://cdn.jsdelivr.net/gh/user/repo@main/a.png",
    )
    .expect("write history");
    let text = std::fs::read_to_string(&path).expect("read history");
    assert!(
        text.contains("\"url\":"),
        "history.json line must contain url field:\n{text}"
    );
    assert!(
        text.contains("\"rawUrl\":"),
        "history.json line must contain rawUrl field:\n{text}"
    );
}
