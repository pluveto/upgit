//! Shipped CLI: `upgit [FILE]...` plus flags. `:clipboard` is not the interface.

use clap::{CommandFactory, Parser};
use upgit::Cli;

fn help_text() -> String {
    let mut cmd = Cli::command();
    let mut buf = Vec::new();
    cmd.write_long_help(&mut buf).expect("help");
    String::from_utf8(buf).expect("utf8")
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
        help.contains("upgit init") || help.contains("init"),
        "help must mention init:\n{help}"
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
