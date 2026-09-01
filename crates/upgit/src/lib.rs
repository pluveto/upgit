use std::fmt::Write as _;
use std::path::PathBuf;

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use upgit_uploaders::HostCatalog;

mod history;
mod output;
mod paths;

pub use history::{record_history, record_upload_log};
pub use output::render_output;
pub use paths::{application_dir, config_search_paths};

/// Upload anything to github repo or other remote storages and then get its link.
#[derive(Parser, Debug)]
#[command(
    name = "upgit",
    version,
    disable_version_flag = true,
    about = "Upload anything to github repo or other remote storages and then get its link.",
    after_help = "Create a config with `upgit init`. List ids with `upgit uploaders`.\nhttps://github.com/pluveto/upgit",
    args_conflicts_with_subcommands = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Local files to upload
    #[arg(value_name = "FILE")]
    pub files: Vec<String>,

    /// Upload the image currently on the clipboard
    #[arg(long)]
    pub clipboard: bool,

    /// Upload files copied on the clipboard (file list)
    #[arg(long = "clipboard-files")]
    pub clipboard_files: bool,

    /// Uploader id (see `upgit uploaders`)
    #[arg(short, long)]
    pub uploader: Option<String>,

    /// stdout or clipboard (clipboard copies the URL)
    #[arg(
        short,
        long,
        value_enum,
        default_value = "stdout",
        visible_alias = "output-type"
    )]
    pub output: Output,

    /// url | markdown | named
    #[arg(short = 'f', long = "format", visible_alias = "output-format")]
    pub format: Option<String>,

    /// Keep the original filename under this remote directory
    #[arg(short = 't', long = "target-dir")]
    pub target_dir: Option<String>,

    /// Maximum file size in bytes (0 = unlimited)
    #[arg(short = 's', long = "size-limit")]
    pub size_limit: Option<u64>,

    /// Path to a TOML config file
    #[arg(short, long, visible_alias = "config-file")]
    pub config: Option<String>,

    /// Skip [link] replacements
    #[arg(short = 'r', long)]
    pub raw: bool,

    /// Delete local files after a successful upload
    #[arg(short = 'C', long)]
    pub clean: bool,

    /// Print uploader, object key, and URL to stderr
    #[arg(short = 'V', long)]
    pub verbose: bool,

    /// Do not exit after upload until the user presses a key
    #[arg(short = 'w', long)]
    pub wait: bool,

    /// Disable writing upgit.log (history.log is still written)
    #[arg(short = 'n', long = "no-log")]
    pub no_log: bool,

    /// Directory that owns config.toml / upgit.toml, history.log, and upgit.log
    #[arg(long = "application-path", value_name = "PATH")]
    pub application_path: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Write a GitHub config.toml
    Init {
        /// Path to write. Default: platform config directory
        dest: Option<PathBuf>,
    },
    /// List built-in uploaders
    Uploaders,
}

/// Destination for the public URL after upload.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum Output {
    #[default]
    Stdout,
    Clipboard,
}

/// Help footer: every host id + title, plus init / uploaders / repo URL.
pub fn after_help() -> String {
    let mut text = String::from("Uploaders (pass --uploader ID, or set default in config.toml):\n");
    let width = HostCatalog::id_width();
    for host in HostCatalog::all() {
        let _ = writeln!(text, "  {:width$}  {}", host.id, host.title, width = width);
    }
    text.push('\n');
    text.push_str("Create a config with `upgit init`. List ids with `upgit uploaders`.\n");
    text.push_str("https://github.com/pluveto/upgit\n");
    text
}

impl Cli {
    pub fn command_with_hosts() -> clap::Command {
        Self::command().after_help(after_help()).arg(
            clap::Arg::new("print_version")
                .long("version")
                .action(clap::ArgAction::Version)
                .help("Print version"),
        )
    }
}
