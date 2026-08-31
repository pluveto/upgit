use clap::{Parser, ValueEnum};

/// Upload local files, a clipboard image, or clipboard files and print a public URL.
#[derive(Parser, Debug)]
#[command(
    name = "upgit",
    version,
    about = "Upload local files, a clipboard image, or clipboard files and print a public URL.",
    long_about = "Upload local files, a clipboard image, or clipboard files to a remote host and print the public URL.\n\nPass one or more file operands to upload those paths. Use --clipboard for the image currently on the clipboard, or --clipboard-files for a file list copied on the clipboard.\n\nChoose an uploader with --uploader (looked up in the registry) and where to write the resulting URL with --output.\n\nFirst run: upgit init writes config.toml. Qiniu uses access_key/secret_key (tokens expire). There is no extensions/ directory."
)]
pub struct Cli {
    /// Local files to upload
    #[arg(value_name = "FILE")]
    pub files: Vec<String>,

    /// Upload the image currently on the clipboard
    #[arg(long)]
    pub clipboard: bool,

    /// Upload files copied on the clipboard (file list)
    #[arg(long = "clipboard-files")]
    pub clipboard_files: bool,

    /// Uploader id (looked up in the registry)
    #[arg(short, long)]
    pub uploader: Option<String>,

    /// Where to write the resulting URL
    #[arg(short, long, value_enum, default_value = "stdout")]
    pub output: Output,

    /// Path to a TOML config file
    #[arg(short, long)]
    pub config: Option<String>,
}

/// Destination for the public URL after upload.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum Output {
    #[default]
    Stdout,
    Clipboard,
}
