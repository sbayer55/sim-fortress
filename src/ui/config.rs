//! Persistent UI options (C6 FR5): `ui.toml` under the XDG config dir, default
//! `~/.config/sim-fortress/ui.toml` on every Unix including macOS (no `dirs`
//! crate).

use std::path::{Path, PathBuf};

use crate::sim::params::UiParams;

/// `$XDG_CONFIG_HOME/sim-fortress/ui.toml`, or `~/.config/…` when unset.
pub fn ui_config_path() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME").filter(|s| !s.is_empty()) {
        PathBuf::from(xdg).join("sim-fortress").join("ui.toml")
    } else {
        let home = std::env::var_os("HOME").filter(|s| !s.is_empty()).unwrap_or_else(|| ".".into());
        PathBuf::from(home).join(".config").join("sim-fortress").join("ui.toml")
    }
}

pub fn load_ui() -> Option<UiParams> {
    load_ui_from(&ui_config_path())
}

pub fn load_ui_from(path: &Path) -> Option<UiParams> {
    let text = std::fs::read_to_string(path).ok()?;
    toml::from_str(&text).ok()
}

pub fn save_ui(ui: &UiParams) -> std::io::Result<()> {
    save_ui_to(&ui_config_path(), ui)
}

pub fn save_ui_to(path: &Path, ui: &UiParams) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let text = toml::to_string_pretty(ui).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e.to_string()))?;
    std::fs::write(path, text)
}
