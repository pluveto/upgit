use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use upgit::{application_dir, Cli, History};
use upgit_core::{BatchPublisher, KeyPolicy, Publisher, Registry, RegistryError};
use upgit_uploaders::{AppConfig, HostCatalog};

use crate::emitter::Emitter;
use crate::source::{Intake, DEFAULT_SIZE_LIMIT};

pub struct App {
    registry: Registry,
    publisher: Publisher,
    namer: KeyPolicy,
    emitter: Emitter,
    uploader_id: String,
    size_limit: Option<u64>,
    verbose: bool,
    clean: bool,
}

impl App {
    pub fn from_cli(cli: &Cli) -> Result<Self, Box<dyn Error>> {
        let config = Self::load_config(cli)?;
        let mut registry = Registry::new();
        config.install_into(&mut registry)?;

        let uploader_id = cli
            .uploader
            .as_deref()
            .filter(|id| !id.is_empty())
            .or_else(|| config.default_uploader())
            .ok_or(
                "no uploader configured; pass --uploader ID or set default in config.toml. Run upgit init to create a config.",
            )?
            .to_string();

        let namer = match cli.target_dir.as_deref() {
            Some(dir) => KeyPolicy::keep_original_in(dir),
            None => config.namer()?,
        };
        let linker = config.linker();
        let formats: Vec<(String, String)> = config
            .output_formats
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        Ok(Self {
            registry,
            publisher: Publisher::new(namer.clone(), linker),
            namer,
            emitter: Emitter::new(cli.output, cli.format.as_deref(), &formats)?,
            uploader_id,
            size_limit: resolve_size_limit(cli.size_limit.or(config.size_limit)),
            verbose: cli.verbose,
            clean: cli.clean,
        })
    }

    pub fn run(&self, cli: &Cli) -> Result<(), Box<dyn Error>> {
        let uploader = self
            .registry
            .get(&self.uploader_id)
            .map_err(unknown_uploader)?;
        if self.verbose {
            eprintln!("uploader: {}", self.uploader_id);
        }
        let mut intake = Intake::from_cli(cli, self.size_limit)?;
        let artifacts = intake.collect()?;
        let now = SystemTime::now();
        let history = if cli.no_log {
            History::silent()
        } else {
            History::files(
                application_dir(cli.application_path.as_deref()),
                &self.uploader_id,
            )
        };
        let published = BatchPublisher::new(&self.publisher)
            .with_concurrency(cli.jobs)
            .run(uploader, &artifacts, now)?;
        let mut items = Vec::with_capacity(published.len());
        for (artifact, (raw, replaced)) in artifacts.iter().zip(published) {
            let shown = if cli.raw {
                raw.as_str().to_string()
            } else {
                replaced.as_str().to_string()
            };
            let key = self
                .namer
                .apply(artifact, now, Some(artifact.stem()))?;
            if self.verbose {
                eprintln!("key: {} url: {shown}", key.as_str());
            }
            history.record(raw.as_str(), replaced.as_str(), key.as_str(), &shown)?;
            items.push((shown, artifact.file_name().to_string()));
        }
        self.emitter.send(&items)?;
        if self.clean {
            for path in &cli.files {
                std::fs::remove_file(path).map_err(|e| format!("cannot delete {path}: {e}"))?;
            }
        }
        if cli.wait {
            let mut line = String::new();
            let _ = std::io::stdin().read_line(&mut line);
        }
        Ok(())
    }

    fn load_config(cli: &Cli) -> Result<AppConfig, Box<dyn Error>> {
        let mut config = if let Some(path) = cli.config.as_deref() {
            Self::read_config(Path::new(path))?
        } else {
            Self::config_candidates(cli)
                .into_iter()
                .find(|path| path.is_file())
                .map(|path| Self::read_config(&path))
                .transpose()?
                .unwrap_or_default()
        };
        config.overlay_env();
        Ok(config)
    }

    fn read_config(path: &Path) -> Result<AppConfig, Box<dyn Error>> {
        let mut config = upgit::migrate::ConfigFile::load(path)?;
        if config.migrate()? {
            config.save()?;
        }
        Ok(AppConfig::from_toml(config.text())?)
    }

    fn config_candidates(cli: &Cli) -> Vec<PathBuf> {
        upgit::env_config_search_paths(cli.application_path.as_deref())
    }
}

/// `Some(0)` from the user means unlimited (`None` to Artifact). Unset → 5MiB.
fn resolve_size_limit(limit: Option<u64>) -> Option<u64> {
    match limit {
        Some(0) => None,
        Some(n) => Some(n),
        None => Some(DEFAULT_SIZE_LIMIT),
    }
}

fn unknown_uploader(err: RegistryError) -> Box<dyn Error> {
    let available = HostCatalog::ids().collect::<Vec<_>>().join(", ");
    format!(
        "{err}\navailable: {available}\nSet `default` in config.toml or pass --uploader ID. Create a config with `upgit init`."
    )
    .into()
}
