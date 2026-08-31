use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use upgit::Cli;
use upgit_core::{Publisher, Registry};
use upgit_uploaders::AppConfig;

use crate::emitter::Emitter;
use crate::source::Intake;

pub struct App {
    registry: Registry,
    publisher: Publisher,
    emitter: Emitter,
    uploader_id: String,
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
            .ok_or("no uploader configured; pass --uploader ID or set `default` in config.toml")?
            .to_string();

        Ok(Self {
            registry,
            publisher: Publisher::new(config.namer(), config.linker()),
            emitter: Emitter::new(cli.output),
            uploader_id,
        })
    }

    pub fn run(&self, cli: &Cli) -> Result<(), Box<dyn Error>> {
        let mut intake = Intake::from_cli(cli)?;
        let artifacts = intake.collect()?;
        let uploader = self.registry.get(&self.uploader_id).map_err(|err| {
            format!(
                "{err} For GitHub:\n[uploaders.github]\npat = \"...\"\nusername = \"...\"\nrepo = \"...\"\n\nFor Qiniu (no JSONC, no static token):\n[uploaders.qiniu]\naccess_key = \"...\"\nsecret_key = \"...\"\nbucket = \"...\"\npublic_base = \"https://your-cdn.example/\"\n"
            )
        })?;
        let now = SystemTime::now();
        let mut urls = Vec::new();
        for artifact in &artifacts {
            let url = self.publisher.publish(uploader, artifact, now)?;
            urls.push(url.as_str().to_string());
        }
        self.emitter.send(&urls)
    }

    fn load_config(cli: &Cli) -> Result<AppConfig, Box<dyn Error>> {
        if let Some(path) = cli.config.as_deref() {
            return Self::read_config(Path::new(path));
        }
        for path in Self::config_candidates() {
            if path.is_file() {
                return Self::read_config(&path);
            }
        }
        Ok(AppConfig::default())
    }

    fn read_config(path: &Path) -> Result<AppConfig, Box<dyn Error>> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read config {}: {e}", path.display()))?;
        Ok(AppConfig::from_toml(&text)?)
    }

    fn config_candidates() -> Vec<PathBuf> {
        let mut out = vec![PathBuf::from("config.toml")];
        if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            out.push(PathBuf::from(xdg).join("upgit").join("config.toml"));
        } else if let Ok(home) = std::env::var("HOME") {
            out.push(
                PathBuf::from(home)
                    .join(".config")
                    .join("upgit")
                    .join("config.toml"),
            );
        }
        if let Some(exe_dir) = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(Path::to_path_buf))
        {
            out.push(exe_dir.join("config.toml"));
        }
        out
    }
}
