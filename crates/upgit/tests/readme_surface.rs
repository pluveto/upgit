//! Structural check of the shipped READMEs (0.2 outline, 0.3 CLI).

const EN: &str = include_str!("../../../README.md");
const ZH: &str = include_str!("../../../docs/README.zh-CN.md");

fn assert_contains(haystack: &str, needle: &str, label: &str) {
    assert!(haystack.contains(needle), "{label} must contain {needle:?}");
}

fn assert_absent(haystack: &str, needle: &str, label: &str) {
    assert!(
        !haystack.contains(needle),
        "{label} must not contain {needle:?}"
    );
}

#[test]
fn english_readme_has_0_2_outline_and_0_3_cli() {
    for needle in [
        "## Feature",
        "### Supported Upload Extensions",
        "## Get started",
        "### Download",
        "### Config",
        "### Use it",
        "Use it for Typora",
        "Upload Clipboard",
        "Config Instructions",
        "{unix_tsms}",
        "{fname_hash}",
        "UPGIT_TOKEN",
        "upgit_20220128_1643373863.png",
        "chmod +x",
        "upgit_win_amd64.zip",
        "upgit_linux_386.zip",
        "upgit uploaders",
        "Upload anything to github repo",
        "--wait",
        "--application-path",
        "history.log",
        "UPGIT_RENAME",
        "MinIO",
        "Ceph",
    ] {
        assert_contains(EN, needle, "README.md");
    }

    for needle in ["v0.3.0-alpha", "JSONC", "blob/next"] {
        assert_absent(EN, needle, "README.md");
    }
}

#[test]
fn chinese_readme_has_0_2_outline() {
    for needle in [
        "## 特点",
        "### 上传扩展",
        "## 开始使用",
        "Typora",
        "配置文件说明",
        "{unix_tsms}",
        "UPGIT_TOKEN",
        "upgit_win_amd64.zip",
        "upgit uploaders",
        "配合 Typora",
        "history.log",
        "--wait",
    ] {
        assert_contains(ZH, needle, "docs/README.zh-CN.md");
    }
}
