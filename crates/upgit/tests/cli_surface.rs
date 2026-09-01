//! Shipped CLI: `upgit [FILE]...` plus flags. `:clipboard` is not the interface.

use std::path::PathBuf;
use std::process::Command;

use clap::Parser;
use upgit::{Cli, Command as CliCommand};

fn help_text() -> String {
    let mut cmd = Cli::command_with_hosts();
    let mut buf = Vec::new();
    cmd.write_long_help(&mut buf).expect("help");
    String::from_utf8(buf).expect("utf8")
}

fn combined_output(output: &std::process::Output) -> String {
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    text
}

#[test]
fn help_documents_files_clipboard_uploader_and_output() {
    let help = help_text();
    let lower = help.to_lowercase();
    assert!(
        lower.contains("file") || help.contains("[FILE]"),
        "help must mention file operands:\n{help}"
    );
    assert!(
        help.contains("--clipboard") && !help.contains("--clipboard-files-only-placeholder"),
        "help must document --clipboard:\n{help}"
    );
    assert!(
        help.contains("--clipboard-files"),
        "help must document --clipboard-files:\n{help}"
    );
    assert!(
        help.contains("--uploader"),
        "help must document --uploader:\n{help}"
    );
    assert!(
        help.contains("--output"),
        "help must document --output:\n{help}"
    );
    assert!(
        help.contains("--format"),
        "help must document --format:\n{help}"
    );
    assert!(
        help.contains("--target-dir"),
        "help must document --target-dir:\n{help}"
    );
    assert!(
        help.contains("--size-limit"),
        "help must document --size-limit:\n{help}"
    );
    assert!(
        help.contains("upgit init") || help.contains("init"),
        "help must mention init:\n{help}"
    );
    assert!(
        help.contains("uploaders"),
        "help must mention uploaders:\n{help}"
    );
    assert!(
        help.contains("github.com/pluveto/upgit"),
        "help must mention the GitHub repo:\n{help}"
    );
    assert!(
        help.contains("--version"),
        "help must document --version:\n{help}"
    );
    assert!(
        help.contains("Uploaders") && help.contains("github"),
        "help must list uploaders:\n{help}"
    );
    assert!(
        help.contains(
            "Upload anything to github repo or other remote storages and then get its link."
        ),
        "help about must match 0.2:\n{help}"
    );
    assert!(
        help.contains("--wait"),
        "help must document --wait:\n{help}"
    );
    assert!(
        help.contains("--no-log"),
        "help must document --no-log:\n{help}"
    );
    assert!(
        help.contains("--application-path"),
        "help must document --application-path:\n{help}"
    );
    assert!(
        help.contains("--config-file"),
        "help must document --config-file:\n{help}"
    );
    assert!(
        help.contains("--output-type"),
        "help must document --output-type:\n{help}"
    );
}

#[test]
fn help_does_not_present_colon_clipboard_as_the_interface() {
    let help = help_text();
    assert!(
        !help.contains(":clipboard"),
        "colon placeholder must not be the documented interface:\n{help}"
    );
}

#[test]
fn help_does_not_mention_registry_jsonc_or_extensions() {
    let help = help_text();
    assert!(
        !help.contains("registry"),
        "help must not mention registry:\n{help}"
    );
    assert!(
        !help.contains("JSONC"),
        "help must not mention JSONC:\n{help}"
    );
    assert!(
        !help.contains("extensions/"),
        "help must not mention extensions/:\n{help}"
    );
}

#[test]
fn parses_files_uploader_and_output_flags() {
    let cli = Cli::try_parse_from([
        "upgit",
        "logo.png",
        "shot.jpg",
        "--uploader",
        "qiniu",
        "--output",
        "clipboard",
    ])
    .expect("parse");
    assert_eq!(cli.files, vec!["logo.png", "shot.jpg"]);
    assert_eq!(cli.uploader.as_deref(), Some("qiniu"));
    assert_eq!(cli.output, upgit::Output::Clipboard);
    assert!(!cli.clipboard);
    assert!(!cli.clipboard_files);
    assert!(cli.command.is_none());
}

#[test]
fn parses_format_target_dir_and_size_limit() {
    let cli = Cli::try_parse_from([
        "upgit", "logo.png", "-f", "markdown", "-t", "dir", "-s", "0",
    ])
    .expect("parse");
    assert_eq!(cli.files, vec!["logo.png"]);
    assert_eq!(cli.format.as_deref(), Some("markdown"));
    assert_eq!(cli.target_dir.as_deref(), Some("dir"));
    assert_eq!(cli.size_limit, Some(0));
}

#[test]
fn parses_clipboard_image_flag() {
    let cli = Cli::try_parse_from(["upgit", "--clipboard"]).expect("parse");
    assert!(cli.clipboard);
    assert!(cli.files.is_empty());
}

#[test]
fn parses_clipboard_files_flag() {
    let cli = Cli::try_parse_from(["upgit", "--clipboard-files"]).expect("parse");
    assert!(cli.clipboard_files);
}

#[test]
fn parses_wait_no_log_application_path_and_aliases() {
    let cli = Cli::try_parse_from([
        "upgit",
        "logo.png",
        "--wait",
        "--no-log",
        "--application-path",
        "/tmp/app",
        "--config-file",
        "my.toml",
        "--output-type",
        "clipboard",
    ])
    .expect("parse");
    assert!(cli.wait);
    assert!(cli.no_log);
    assert_eq!(
        cli.application_path.as_deref(),
        Some(std::path::Path::new("/tmp/app"))
    );
    assert_eq!(cli.config.as_deref(), Some("my.toml"));
    assert_eq!(cli.output, upgit::Output::Clipboard);
}

#[test]
fn parses_short_wait_and_no_log() {
    let cli = Cli::try_parse_from(["upgit", "logo.png", "-w", "-n"]).expect("parse");
    assert!(cli.wait);
    assert!(cli.no_log);
}

#[test]
fn parses_init_subcommand() {
    let cli = Cli::try_parse_from(["upgit", "init"]).expect("parse");
    assert!(matches!(cli.command, Some(CliCommand::Init { dest: None })));

    let cli = Cli::try_parse_from(["upgit", "init", "/tmp/config.toml"]).expect("parse");
    match cli.command {
        Some(CliCommand::Init { dest: Some(path) }) => {
            assert_eq!(path, PathBuf::from("/tmp/config.toml"));
        }
        other => panic!("expected init with dest, got {other:?}"),
    }
}

#[test]
fn parses_uploaders_subcommand() {
    let cli = Cli::try_parse_from(["upgit", "uploaders"]).expect("parse");
    assert!(matches!(cli.command, Some(CliCommand::Uploaders)));
}

#[test]
fn init_help_is_clap_help_not_a_write() {
    let err = Cli::try_parse_from(["upgit", "init", "--help"]).expect_err("help");
    let text = err.to_string();
    assert!(
        text.contains("Usage") || text.contains("init"),
        "init --help should be clap help, got:\n{text}"
    );
}

#[test]
fn binary_no_args_prints_usage_and_fails() {
    let output = Command::new(env!("CARGO_BIN_EXE_upgit"))
        .output()
        .expect("run upgit");
    assert!(!output.status.success(), "upgit with no args must fail");
    let text = combined_output(&output);
    assert!(
        text.contains("Usage"),
        "no-args output must contain Usage:\n{text}"
    );
    assert!(
        text.contains("--version"),
        "no-args help must include --version:\n{text}"
    );
    assert!(
        text.contains("Uploaders") && text.contains("github"),
        "no-args help must list uploaders:\n{text}"
    );
}

#[test]
fn binary_init_help_does_not_create_a_file_named_help() {
    let dir = tempfile::tempdir().expect("tempdir");
    let output = Command::new(env!("CARGO_BIN_EXE_upgit"))
        .current_dir(dir.path())
        .args(["init", "--help"])
        .output()
        .expect("run init --help");
    assert!(
        output.status.success(),
        "init --help should succeed, stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = combined_output(&output);
    assert!(
        text.contains("Usage") || text.to_lowercase().contains("init"),
        "init --help output:\n{text}"
    );
    assert!(
        !dir.path().join("--help").exists(),
        "init --help must not write a file named --help"
    );
}

#[test]
fn binary_help_contains_wait_no_log_and_aliases() {
    let output = Command::new(env!("CARGO_BIN_EXE_upgit"))
        .arg("-h")
        .output()
        .expect("run upgit -h");
    assert!(
        output.status.success(),
        "upgit -h should succeed, stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = combined_output(&output);
    for flag in [
        "--wait",
        "--no-log",
        "--application-path",
        "--config-file",
        "--output-type",
    ] {
        assert!(text.contains(flag), "upgit -h must contain {flag}:\n{text}");
    }
    assert!(
        text.contains(
            "Upload anything to github repo or other remote storages and then get its link."
        ),
        "upgit -h about must match 0.2:\n{text}"
    );
}

#[test]
fn binary_uploaders_lists_github_and_smms() {
    let output = Command::new(env!("CARGO_BIN_EXE_upgit"))
        .arg("uploaders")
        .output()
        .expect("run uploaders");
    assert!(
        output.status.success(),
        "uploaders failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("github"), "uploaders stdout:\n{stdout}");
    assert!(stdout.contains("smms"), "uploaders stdout:\n{stdout}");
    let first = stdout.lines().next().expect("non-empty");
    assert!(
        first.starts_with("github") && first.contains("GitHub") && first.contains("  "),
        "first line should be aligned columns, got {first:?}"
    );
    assert!(
        !first.contains('\t'),
        "uploaders must not use tabs: {first:?}"
    );
}
