use std::process::Command;

fn assignment_lines(text: &str) -> Vec<(usize, String)> {
    text.lines()
        .enumerate()
        .filter_map(|(i, line)| {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('[') {
                return None;
            }
            trimmed.contains('=').then(|| (i, trimmed.to_string()))
        })
        .collect()
}

fn every_field_has_a_comment(text: &str) {
    let lines: Vec<&str> = text.lines().collect();
    for (i, field) in assignment_lines(text) {
        assert!(
            i > 0 && lines[i - 1].trim_start().starts_with('#'),
            "field `{field}` needs a comment immediately above it"
        );
    }
}

fn comment_above(text: &str, field: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let needle = format!("{field} =");
    for (i, line) in lines.iter().enumerate() {
        if line.trim_start().starts_with(&needle) {
            assert!(i > 0, "field `{field}` is missing a comment above it");
            let comment = lines[i - 1].trim();
            assert!(
                comment.starts_with('#'),
                "field `{field}` needs a comment immediately above it, got `{comment}`"
            );
            return comment.to_string();
        }
    }
    panic!("missing field `{field}`");
}

#[test]
fn init_writes_github_config_and_recipes_without_hand_copying() {
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
    let template = include_str!("../../../config.github.toml");
    assert_eq!(text, template);
    assert!(text.contains("default = \"github\""));
    assert!(text.contains("[uploaders.github]"));
    assert!(
        !text.contains("[uploaders.qiniu]"),
        "init config must omit other uploader tables"
    );
    assert!(
        !text.contains("[uploaders.s3]"),
        "init config must omit other uploader tables"
    );
    assert_eq!(
        text.matches("[uploaders.").count(),
        1,
        "init config should contain only [uploaders.github]"
    );
    assert!(text.contains("settings/tokens"));
    assert!(text.to_lowercase().contains("public"));
    assert!(!text.contains("extensions/"));
    assert!(dir.path().join("recipes").join("gitee.toml").is_file());
    every_field_has_a_comment(&text);
    for field in ["pat", "username", "repo", "branch", "default", "naming"] {
        comment_above(&text, field);
    }
}

#[test]
fn packed_github_template_comments_pat_username_repo_branch() {
    let text = include_str!("../../../config.github.toml");
    assert!(text.contains("default = \"github\""));
    assert!(text.contains("[uploaders.github]"));
    assert!(!text.contains("[uploaders.qiniu]"));
    let pat = comment_above(text, "pat");
    assert!(
        pat.contains("settings/tokens"),
        "pat comment should point at GitHub PAT URL, got {pat}"
    );
    let repo = comment_above(text, "repo");
    assert!(
        repo.to_lowercase().contains("public"),
        "repo comment should say the repo must be public, got {repo}"
    );
    comment_above(text, "username");
    comment_above(text, "branch");
    every_field_has_a_comment(text);
}
