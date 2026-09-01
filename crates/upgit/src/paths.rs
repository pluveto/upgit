use std::path::{Path, PathBuf};

/// Directory that owns `config.toml` / `upgit.toml`, `history.log`, and `upgit.log`.
///
/// `explicit` is `--application-path`. When unset, this is the directory of
/// `current_exe`.
pub fn application_dir(explicit: Option<&Path>) -> PathBuf {
    if let Some(path) = explicit {
        return path.to_path_buf();
    }
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Config files tried when `--config` / `--config-file` is unset.
///
/// Search order (unique paths; first existing file wins at runtime):
/// 1. `./config.toml`
/// 2. `{xdg_config_home}/upgit/config.toml`
/// 3. Windows `{appdata}/upgit/config.toml`
/// 4. `{home}/.config/upgit/config.toml`
/// 5. `{home}/.upgit.config.toml` (0.2)
/// 6. `{home}/.config/upgitrc` (0.2)
/// 7. USERPROFILE variants: `{userprofile}/.config/upgit/config.toml`,
///    the 0.2 names under `userprofile`, and on Windows
///    `{userprofile}/AppData/Roaming/upgit/config.toml`
/// 8. `{application_path}/config.toml`
/// 9. `{application_path}/upgit.toml` (0.2)
/// 10. `{exe_dir}/config.toml` and `{exe_dir}/upgit.toml` when that directory
///     is not already `application_path`
///
/// `--config` / `--config-file` still wins and is required if given.
pub fn config_search_paths(
    home: Option<&Path>,
    application_path: Option<&Path>,
    xdg_config_home: Option<&Path>,
    appdata: Option<&Path>,
    userprofile: Option<&Path>,
    exe_dir: Option<&Path>,
) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut push_unique = |p: PathBuf| {
        if !out.contains(&p) {
            out.push(p);
        }
    };

    push_unique(PathBuf::from("config.toml"));
    if let Some(xdg) = xdg_config_home {
        push_unique(xdg.join("upgit").join("config.toml"));
    }
    if let Some(appdata) = appdata {
        push_unique(appdata.join("upgit").join("config.toml"));
    }
    if let Some(home) = home {
        push_unique(home.join(".config").join("upgit").join("config.toml"));
        push_unique(home.join(".upgit.config.toml"));
        push_unique(home.join(".config").join("upgitrc"));
    }
    if let Some(profile) = userprofile {
        push_unique(profile.join(".config").join("upgit").join("config.toml"));
        push_unique(profile.join(".upgit.config.toml"));
        push_unique(profile.join(".config").join("upgitrc"));
        if cfg!(windows) {
            push_unique(
                profile
                    .join("AppData")
                    .join("Roaming")
                    .join("upgit")
                    .join("config.toml"),
            );
        }
    }
    if let Some(appdir) = application_path {
        push_unique(appdir.join("config.toml"));
        push_unique(appdir.join("upgit.toml"));
    }
    if let Some(exe) = exe_dir {
        push_unique(exe.join("config.toml"));
        push_unique(exe.join("upgit.toml"));
    }
    out
}

fn nonempty_env(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|s| !s.is_empty())
}

/// Config search paths from the process environment.
///
/// Same order as runtime upload: `--application-path` (or the exe directory)
/// plus XDG / APPDATA / `$HOME` fallbacks.
pub fn env_config_search_paths(application_path: Option<&Path>) -> Vec<PathBuf> {
    let home = nonempty_env("HOME");
    let xdg = nonempty_env("XDG_CONFIG_HOME");
    let appdata = if cfg!(windows) {
        nonempty_env("APPDATA")
    } else {
        None
    };
    let profile = nonempty_env("USERPROFILE");
    let appdir = application_dir(application_path);
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf));
    config_search_paths(
        home.as_deref().map(Path::new),
        Some(appdir.as_path()),
        xdg.as_deref().map(Path::new),
        appdata.as_deref().map(Path::new),
        profile.as_deref().map(Path::new),
        exe_dir.as_deref(),
    )
}

/// Default `config.toml` written by `upgit init` with no dest argument.
pub fn platform_config_file() -> Option<PathBuf> {
    if cfg!(windows) {
        nonempty_env("APPDATA")
            .map(|p| PathBuf::from(p).join("upgit").join("config.toml"))
            .or_else(|| {
                nonempty_env("USERPROFILE").map(|p| {
                    PathBuf::from(p)
                        .join("AppData")
                        .join("Roaming")
                        .join("upgit")
                        .join("config.toml")
                })
            })
    } else {
        nonempty_env("XDG_CONFIG_HOME")
            .map(|p| PathBuf::from(p).join("upgit").join("config.toml"))
            .or_else(|| {
                nonempty_env("HOME").map(|p| {
                    PathBuf::from(p)
                        .join(".config")
                        .join("upgit")
                        .join("config.toml")
                })
            })
            .or_else(|| {
                nonempty_env("USERPROFILE").map(|p| {
                    PathBuf::from(p)
                        .join(".config")
                        .join("upgit")
                        .join("config.toml")
                })
            })
    }
}
