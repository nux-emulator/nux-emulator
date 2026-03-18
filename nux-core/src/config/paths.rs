//! XDG-compliant config path resolution.

use std::path::PathBuf;

/// Returns the global config file path.
///
/// Resolves to `$XDG_CONFIG_HOME/nux/config.toml`,
/// falling back to `~/.config/nux/config.toml`.
///
/// # Panics
///
/// Panics if the home directory cannot be determined.
pub fn global_config_path() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| {
        let home = dirs::home_dir().expect("cannot determine home directory");
        home.join(".config")
    });
    base.join("nux").join("config.toml")
}

/// Returns the instance config file path for the given instance name.
///
/// Resolves to `$XDG_DATA_HOME/nux/instances/<name>/config.toml`,
/// falling back to `~/.local/share/nux/instances/<name>/config.toml`.
///
/// # Panics
///
/// Panics if the home directory cannot be determined.
pub fn instance_config_path(name: &str) -> PathBuf {
    let base = dirs::data_dir().unwrap_or_else(|| {
        let home = dirs::home_dir().expect("cannot determine home directory");
        home.join(".local").join("share")
    });
    base.join("nux")
        .join("instances")
        .join(name)
        .join("config.toml")
}
