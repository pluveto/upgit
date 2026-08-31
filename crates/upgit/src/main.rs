use std::path::Path;

use clap::Parser;
use upgit::Cli;

mod app;
mod emitter;
mod init;
mod source;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("init") {
        let dest = args.get(2).map(Path::new);
        if let Err(err) = init::run(dest) {
            eprintln!("error: {err}");
            std::process::exit(1);
        }
        return;
    }

    let cli = Cli::parse();
    if let Err(err) = app::App::from_cli(&cli).and_then(|app| app.run(&cli)) {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}
