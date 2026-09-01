use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use upgit::Cli;
use upgit_core::{KeyPolicy, LinkPolicy, Publisher, Registry, RegistryError};
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
        let linker = if cli.raw {
            LinkPolicy::identity()
        } else {
            config.linker()
        };
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
        let mut urls = Vec::new();
        for artifact in &artifacts {
            let url = self.publisher.publish(uploader, artifact, now)?;
            if self.verbose {
                let key = self.namer.apply(artifact, now)?;
                eprintln!("key: {} url: {}", key.as_str(), url.as_str());
            }
            urls.push(url.as_str().to_string());
        }
        self.emitter.send(&urls)?;
        if self.clean {
            for path in &cli.files {
                std::fs::remove_file(path).map_err(|e| format!("cannot delete {path}: {e}"))?;
            }
        }
        Ok(())
    }

    fn load_config(cli: &Cli) -> Result<AppConfig, Box<dyn Error>> {
        let mut config = if let Some(path) = cli.config.as_deref() {
            Self::read_config(Path::new(path))?
        } else {
            Self::config_candidates()
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
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read config {}: {e}", path.display()))?;
        Ok(AppConfig::from_toml(&text)?)
    }

    fn config_candidates() -> Vec<PathBuf> {
        let mut out = Vec::new();
        let mut push_unique = |p: PathBuf| {
            if !out.contains(&p) {
                out.push(p);
            }
        };
        push_unique(PathBuf::from("config.toml"));
        if let Some(xdg) = nonempty_env("XDG_CONFIG_HOME") {
            push_unique(PathBuf::from(xdg).join("upgit").join("config.toml"));
        }
        if cfg!(windows) {
            if let Some(appdata) = nonempty_env("APPDATA") {
                push_unique(PathBuf::from(appdata).join("upgit").join("config.toml"));
            }
        }
        if let Some(home) = nonempty_env("HOME") {
            push_unique(
                PathBuf::from(home)
                    .join(".config")
                    .join("upgit")
                    .join("config.toml"),
            );
        }
        if let Some(profile) = nonempty_env("USERPROFILE") {
            push_unique(
                PathBuf::from(&profile)
                    .join(".config")
                    .join("upgit")
                    .join("config.toml"),
            );
            if cfg!(windows) {
                push_unique(
                    PathBuf::from(&profile)
                        .join("AppData")
                        .join("Roaming")
                        .join("upgit")
                        .join("config.toml"),
                );
            }
        }
        if let Some(exe_dir) = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(Path::to_path_buf))
        {
            push_unique(exe_dir.join("config.toml"));
        }
        out
    }
}

fn nonempty_env(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|s| !s.is_empty())
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
