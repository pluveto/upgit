use std::path::Path;

use upgit::config_search_paths;

#[test]
fn config_search_paths_includes_dot_upgit_and_upgit_toml() {
    let home = Path::new("/home/alice");
    let appdir = Path::new("/opt/upgit");
    let exe_dir = Path::new("/usr/bin");
    let paths = config_search_paths(Some(home), Some(appdir), None, None, None, Some(exe_dir));
    assert!(
        paths
            .iter()
            .any(|p| p == Path::new("/home/alice/.upgit.config.toml")),
        "missing .upgit.config.toml in {paths:?}"
    );
    assert!(
        paths
            .iter()
            .any(|p| p == Path::new("/opt/upgit/upgit.toml")),
        "missing application-path upgit.toml in {paths:?}"
    );
    assert!(
        paths.iter().any(|p| p == Path::new("/usr/bin/upgit.toml")),
        "missing exe-dir upgit.toml in {paths:?}"
    );
    assert!(
        paths
            .iter()
            .any(|p| p == Path::new("/home/alice/.config/upgitrc")),
        "missing .config/upgitrc in {paths:?}"
    );
}
