use std::process::Command;

#[test]
fn init_writes_config_and_recipes_without_hand_copying() {
    let dir = tempfile::tempdir().expect("tempdir");
    let status = Command::new(env!("CARGO_BIN_EXE_upgit"))
        .current_dir(dir.path())
        .arg("init")
        .status()
        .expect("run init");
    assert!(status.success(), "upgit init failed");
    let config = dir.path().join("config.toml");
    let smms = dir.path().join("recipes").join("smms.toml");
    assert!(config.is_file(), "missing {}", config.display());
    assert!(smms.is_file(), "missing {}", smms.display());
    let text = std::fs::read_to_string(&config).expect("read config");
    assert!(text.contains("default = \"github\""));
    assert!(text.contains("[uploaders.github]"));
    assert!(text.contains("[uploaders.qiniu]"));
    assert!(text.contains("access_key"));
    assert!(!text.contains("extensions/"));
    assert!(dir.path().join("recipes").join("gitee.toml").is_file());
}
