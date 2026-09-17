//! The `[ai]` table (C9): plain configuration for the optional language-model
//! layer. It is data only, so it may live beside the other `UiParams` fields
//! (ai-requirements R3); the code that acts on it lives in `crate::ai`, never
//! under `sim`.
//!
//! The table is persisted in `ui.toml` (it serialises at the root of
//! `UiParams`), and is *read* only from there: a `--params` overlay carrying
//! `[ui.ai]` parses but never switches AI on (R1), because the live app and the
//! headless flags build their handle from `ui::config::load_ai()` alone.

use serde::{Deserialize, Serialize};

/// The optional bearer token for the gateway itself (never a cloud key, R6).
///
/// Serialised as-is to self-describing formats (`ui.toml`); written as an empty
/// string to binary saves, because `Params` is stored whole inside every
/// `.simf` and a secret must not travel with a world.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(transparent)]
pub struct GatewayToken(pub String);

impl Serialize for GatewayToken {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        if s.is_human_readable() {
            s.serialize_str(&self.0)
        } else {
            s.serialize_str("")
        }
    }
}

/// Which model each feature uses. A feature is on only when it names a model
/// (`"<provider>/<model>"`, Bifrost's form); an empty string is off.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AiFeatures {
    /// Season-by-season narration of the event log (downstream).
    pub chronicle: String,
    /// Models Bifrost tries, in order, when `chronicle` fails.
    pub chronicle_fallbacks: Vec<String>,
    /// Species overlays written from a sentence (upstream).
    pub designer: String,
    /// Models Bifrost tries, in order, when `designer` fails.
    pub designer_fallbacks: Vec<String>,
}

impl AiFeatures {
    /// The model string for a feature key, or `None` when the feature is off.
    pub fn model(&self, feature: &str) -> Option<&str> {
        let m = match feature {
            "chronicle" => self.chronicle.as_str(),
            "designer" => self.designer.as_str(),
            _ => "",
        };
        (!m.is_empty()).then_some(m)
    }

    /// The fallback list for a feature key.
    pub fn fallbacks(&self, feature: &str) -> &[String] {
        match feature {
            "chronicle" => &self.chronicle_fallbacks,
            "designer" => &self.designer_fallbacks,
            _ => &[],
        }
    }
}

/// The `[ai]` table. Off unless `enabled` *and* a feature names a model (R1).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AiConfig {
    /// Master switch. Nothing runs, probes or listens while false.
    pub enabled: bool,
    /// Bifrost's OpenAI-compatible root.
    pub base_url: String,
    /// Optional bearer token for the gateway itself.
    pub token: GatewayToken,
    /// Per-request timeout; a timeout is an error reply, never a retry (R7).
    pub timeout_secs: u32,
    pub features: AiFeatures,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            base_url: "http://localhost:8080/v1".to_string(),
            token: GatewayToken::default(),
            timeout_secs: 30,
            features: AiFeatures::default(),
        }
    }
}

impl AiConfig {
    /// `enabled` and the feature names a model.
    pub fn feature_on(&self, feature: &str) -> bool {
        self.enabled && self.features.model(feature).is_some()
    }
}
