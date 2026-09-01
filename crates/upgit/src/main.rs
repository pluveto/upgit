use std::error::Error;

use clap::FromArgMatches;
use upgit::{Cli, Command};
use upgit_uploaders::HostCatalog;

mod app;
mod emitter;
mod init;
mod source;

fn main() {
    let cli = parse_cli();
    if let Err(err) = dispatch(cli) {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn parse_cli() -> Cli {
    let matches = Cli::command_with_hosts().get_matches();
    match Cli::from_arg_matches(&matches) {
        Ok(cli) => cli,
        Err(err) => err.exit(),
    }
}

fn dispatch(cli: Cli) -> Result<(), Box<dyn Error>> {
    match &cli.command {
        Some(Command::Init { dest }) => init::run(dest.as_deref()),
        Some(Command::Uploaders) => {
            let width = HostCatalog::id_width();
            for host in HostCatalog::all() {
                println!("{:width$}  {}", host.id, host.title, width = width);
            }
            Ok(())
        }
        Some(Command::Update {
            beta,
            alpha,
            dry_run,
            force,
            apply_migrations,
        }) => upgit::update::run(upgit::update::Opts {
            channel: upgit::update::Channel::from_flags(*beta, *alpha),
            dry_run: *dry_run,
            force: *force,
            apply_migrations: *apply_migrations,
        }),
        None if cli.files.is_empty() && !cli.clipboard && !cli.clipboard_files => {
            let mut cmd = Cli::command_with_hosts();
            cmd.print_help()?;
            std::process::exit(2);
        }
        None => app::App::from_cli(&cli)?.run(&cli),
    }
}
