//! All simulation tunables, serializable to/from TOML.
//!
//! `Params::from_toml` accepts a *partial* table: missing fields and sections
//! deep-merge over `Default`. Unknown keys are an error (`deny_unknown_fields`).

use serde::{Deserialize, Serialize};

use docs::FIELD_DOCS;
use toml_util::{attach_comment, deep_merge};
pub use world::{DayNightTint, EventsParams, Rainfall, ScarcityThresholds, StatsParams, TimeParams, UiParams, WorldParams};
pub use creatures::CreaturesParams;
pub use predation::{Difficulty, PredationParams};
pub use genetics::GeneticsParams;
pub use social::SocialParams;
pub use diet::DietParams;
pub use territory::TerritoryParams;
pub use succession::SuccessionParams;
pub use ecology::EcologyParams;
pub use pathogen::{DiseaseParams, PathogenParams};
pub use presets::{PRESETS, Preset};
pub use species::{BaseGenome, Roster, SpeciesParams};
pub use ai::{AiConfig, AiFeatures, GatewayToken};
pub use quirks::{QuirkDef, QuirkKinds, QuirkParams, QuirkSpecial, QuirkTier};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct Params {
    pub world: WorldParams,
    pub time: TimeParams,
    pub ui: UiParams,
    pub events: EventsParams,
    pub stats: StatsParams,
    pub creatures: CreaturesParams,
    pub genetics: GeneticsParams,
    pub ecology: EcologyParams,
    pub predation: PredationParams,
    pub disease: DiseaseParams,
    pub social: SocialParams,
    /// Diet breadth: which terrains a herbivore can graze (genome slot 12).
    pub diet: DietParams,
    /// Territory: the scent grid, scent avoidance and the contest (C5 FR13).
    pub territory: TerritoryParams,
    /// Succession and trampling: how grazing reshapes the terrain (C2 FR12).
    pub succession: SuccessionParams,
    /// The species roster (`[[species]]`); position is the `SpeciesId`.
    pub species: Roster,
    /// Quirks: named birth oddities (`[quirks]`), off unless chosen at world creation.
    pub quirks: QuirkParams,
}

impl Params {
    /// Parse a (possibly partial) TOML table, deep-merged over defaults.
    pub fn from_toml(s: &str) -> Result<Self, toml::de::Error> {
        let p: Self = toml::from_str(s)?;
        p.validate().map_err(serde::de::Error::custom)?;
        Ok(p)
    }

    pub fn to_toml(&self) -> Result<String, toml::ser::Error> {
        toml::to_string_pretty(self)
    }

    /// Deep-merge a partial TOML overlay onto `self` (C6 FR6). Unknown keys are
    /// still rejected by `deny_unknown_fields`; missing keys keep their value.
    pub fn apply_overlay(&mut self, s: &str) -> Result<(), String> {
        let base = toml::Value::try_from(self.clone()).map_err(|e| e.to_string())?;
        let overlay: toml::Value = toml::from_str(s).map_err(|e| e.to_string())?;
        let merged = deep_merge(base, overlay);
        let merged: Self = merged.try_into().map_err(|e| e.to_string())?;
        merged.validate()?;
        *self = merged;
        Ok(())
    }

    /// Dump the defaults as TOML with one `#` comment per leaf (C6 FR6).
    pub fn dump_toml() -> String {
        let default = Self::default();
        let text = toml::to_string_pretty(&default).unwrap_or_default();
        let mut doc: toml_edit::DocumentMut = match text.parse() {
            Ok(d) => d,
            Err(_) => return text,
        };
        for (path, comment) in Self::field_docs() {
            attach_comment(&mut doc, path, comment);
        }
        doc.to_string()
    }

    /// `(path, doc)` for every leaf parameter (C6 FR6). One entry per struct
    /// field that is not itself a nested `*Params` struct.
    pub const fn field_docs() -> &'static [(&'static str, &'static str)] {
        FIELD_DOCS
    }
}

mod world;
mod ai;
mod creatures;
mod predation;
mod genetics;
mod social;
mod diet;
mod territory;
mod succession;
mod ecology;
mod pathogen;
mod quirks;
mod docs;
mod presets;
mod toml_util;
mod species;
mod validate;
#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests;
