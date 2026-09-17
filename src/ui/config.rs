//! Persistent UI options (C6 FR5): `ui.toml` under the XDG config dir, default
//! `~/.config/sim-fortress/ui.toml` on every Unix including macOS (no `dirs`
//! crate).

use std::path::{Path, PathBuf};

use crate::sim::params::{AiConfig, UiParams};

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

/// The `[ai]` table from `ui.toml` only (C9).
///
/// Params overlays never enable AI (ai-requirements R1): the live app and the
/// headless flags build their `Ai` handle from this and nothing else. A missing
/// or unreadable file means off.
pub fn load_ai() -> AiConfig {
    load_ui().map(|u| u.ai).unwrap_or_default()
}

/// Tests that set `XDG_CONFIG_HOME` hold this: the variable is process-global
/// and the unit tests run in parallel.
#[cfg(test)]
pub(crate) fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
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

#[cfg(test)]
mod tests {
    use super::*;

    /// R6: nothing in the `[ai]` table is a cloud credential. The serialised
    /// field names are checked against a deny list, so adding one fails here.
    #[test]
    fn ai_config_has_no_credential_fields() {
        let text = toml::to_string(&AiConfig::default()).unwrap_or_default();
        let deny = ["access_key", "secret", "aws", "region", "arn", "api_key", "password", "profile"];
        for line in text.lines() {
            let key = line.split('=').next().unwrap_or("").trim().to_ascii_lowercase();
            for d in deny {
                assert!(!key.contains(d), "credential-like field {key:?} in [ai]");
            }
        }
        assert!(text.contains("base_url"), "sanity: {text}");
    }

    /// The gateway token is the one field a player types that must not travel
    /// with a world: it is written blank to binary saves.
    #[test]
    fn gateway_token_blank_in_binary_saves() {
        let mut ui = UiParams::default();
        ui.ai.token = crate::sim::params::GatewayToken("s3cret".into());
        let toml_text = toml::to_string(&ui).unwrap_or_default();
        assert!(toml_text.contains("s3cret"));
        let bytes = postcard::to_allocvec(&ui).unwrap_or_default();
        let back: UiParams = postcard::from_bytes(&bytes).unwrap_or_default();
        assert_eq!(back.ai.token.0, "");
        assert_eq!(back.ai.timeout_secs, ui.ai.timeout_secs);
    }
}
