use clap::Parser;
use upgit::Cli;

mod app;
mod emitter;
mod source;

fn main() {
    let cli = Cli::parse();
    if let Err(err) = app::App::from_cli(&cli).and_then(|app| app.run(&cli)) {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}
