//! The species designer prompt (C9, upstream).
//!
//! Sends exactly: the player's sentence, the `species.*` field docs, one
//! example `[[species]]` entry, and the current roster's names, kinds and
//! glyphs. Never the full parameter set. The reply is JSON for one species;
//! `overlay_from_reply` turns it into the TOML the roster loader already
//! accepts, and `validated` runs it through `Params::apply_overlay` (R8).

use std::fmt::Write as _;

use crate::ai::Message;
use crate::sim::params::Roster;
use crate::sim::{Kind, Params};

/// JSON schema for one species, sent as `response_format`. Mirrors
/// `SpeciesParams`; every field is optional because the loader defaults them.
pub const SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "name": {"type": "string"},
    "plural": {"type": "string"},
    "glyph": {"type": "string", "minLength": 1, "maxLength": 1},
    "color": {"type": "array", "items": {"type": "integer", "minimum": 0, "maximum": 255}, "minItems": 3, "maxItems": 3},
    "kind": {"type": "string", "enum": ["prey", "predator"]},
    "diet": {"type": "string"},
    "initial_count": {"type": "integer", "minimum": 0},
    "adult_age_days": {"type": "integer", "minimum": 1},
    "gestation_days": {"type": "integer", "minimum": 1},
    "litter_max": {"type": "number", "minimum": 0},
    "mate_cooldown_days": {"type": "integer", "minimum": 0},
    "nocturnal": {"type": "boolean"},
    "prey_preference": {"type": "object", "additionalProperties": {"type": "number"}},
    "names": {"type": "array", "items": {"type": "string"}},
    "base_genome": {
      "type": "object",
      "properties": {
        "speed": {"type": "number"}, "size": {"type": "number"}, "sense": {"type": "number"},
        "metabolism": {"type": "number"}, "aggression": {"type": "number"}, "camouflage": {"type": "number"},
        "fertility": {"type": "number"}, "longevity": {"type": "number"}, "resistance": {"type": "number"},
        "sociality": {"type": "number"}, "maturity": {"type": "number"}, "mutability": {"type": "number"}
      },
      "additionalProperties": false
    }
  },
  "required": ["name", "plural", "glyph", "kind"],
  "additionalProperties": false
}"#;

const EXAMPLE: &str = r#"{"name":"boar","plural":"Boars","glyph":"b","color":[120,90,60],"kind":"prey","diet":"roots, acorns","initial_count":30,"adult_age_days":70,"gestation_days":40,"litter_max":5.0,"mate_cooldown_days":30,"nocturnal":false,"base_genome":{"size":0.75,"aggression":0.55,"speed":0.4}}"#;

/// The `species.*` lines of the field docs, the schema's semantics.
fn species_docs() -> String {
    let mut out = String::new();
    for (path, doc) in Params::field_docs() {
        if let Some(field) = path.strip_prefix("species.") {
            let _ = writeln!(out, "- {field}: {doc}");
        }
    }
    out
}

/// The messages for one design request. `previous` is `(reply, error)` from a
/// rejected attempt, for the single correction round the rules allow.
pub fn messages(request: &str, roster: &Roster, previous: Option<(&str, &str)>) -> Vec<Message> {
    let mut system = String::from(
        "You design ONE species for a terminal ecology simulation. Reply with only a JSON object for \
that species, no prose and no code fences, following this schema and these field rules.\n\
Rules: `name` is a new lowercase ASCII word not in the roster; `glyph` is one lowercase ASCII letter \
not used by the roster; every base_genome value is between 0.02 and 0.98 and 0.5 is average; a \
predator needs `prey_preference` over existing prey names with shares that sum to about 1, and a \
prey species must not have one. Omit fields you have no opinion on.\nFields:\n",
    );
    system.push_str(&species_docs());
    let _ = writeln!(system, "Example: {EXAMPLE}");
    let _ = writeln!(system, "Current roster (name, kind, glyph):");
    for id in roster.ids() {
        let kind = if roster.kind(id) == Kind::Predator { "predator" } else { "prey" };
        let glyph = roster.0.get(id.index()).map_or('?', |s| s.glyph);
        let _ = writeln!(system, "- {} ({kind}, glyph {glyph})", roster.name(id));
    }
    let mut user = format!("Design this species: {request}");
    if let Some((reply, error)) = previous {
        let _ = write!(user, "\n\nYour previous reply was rejected by the loader with: {error}\nThe reply was: {reply}\nReply again with corrected JSON only.");
    }
    vec![Message::system(system), Message::user(user)]
}

/// A reply turned into the `[[species]]` overlay text the loader accepts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Overlay {
    pub name: String,
    pub toml: String,
}

/// Parse the model's JSON (code fences tolerated) into overlay TOML. No
/// validation happens here; `validated` does that with the real loader.
#[cfg(feature = "ai")]
pub fn overlay_from_reply(content: &str) -> Result<Overlay, String> {
    let text = content.trim();
    let text = text.strip_prefix("```json").or_else(|| text.strip_prefix("```")).unwrap_or(text);
    let text = text.strip_suffix("```").unwrap_or(text).trim();
    let json: serde_json::Value = serde_json::from_str(text).map_err(|e| format!("reply is not JSON: {e}"))?;
    let name = json.get("name").and_then(serde_json::Value::as_str).unwrap_or("").to_string();
    if name.is_empty() {
        return Err("reply has no \"name\"".to_string());
    }
    let species = toml::Value::try_from(json).map_err(|e| format!("reply is not a species table: {e}"))?;
    let mut root = toml::map::Map::new();
    root.insert("species".to_string(), toml::Value::Array(vec![species]));
    let toml = toml::to_string(&toml::Value::Table(root)).map_err(|e| e.to_string())?;
    Ok(Overlay { name, toml })
}

/// `base` with the overlay applied by the ordinary loader (deep-merge by name,
/// then `Params::validate`), or the loader's message.
pub fn validated(base: &Params, overlay_toml: &str) -> Result<Params, String> {
    let mut p = base.clone();
    p.apply_overlay(overlay_toml)?;
    Ok(p)
}

/// The lines S09b shows before the player accepts a design.
pub fn summary(params: &Params, name: &str) -> Vec<String> {
    let Some(s) = params.species.0.iter().find(|s| s.name == name) else {
        return vec![format!("{name}: not in the roster")];
    };
    let kind = if s.kind == Kind::Predator { "predator" } else { "prey" };
    let mut out = vec![
        format!("{} / {}  glyph {}  {kind}  {}", s.name, s.plural, s.glyph, if s.diet.is_empty() { "-" } else { &s.diet }),
        format!("founders {}  adult at {} days  gestation {} days  litter up to {:.0}", s.initial_count, s.adult_age_days, s.gestation_days, 1.0 + s.litter_max),
    ];
    let g = s.base_genome.genome();
    let traits: Vec<String> = crate::sim::TRAIT_NAMES
        .iter()
        .zip(g.0.iter())
        .filter(|(_, v)| (**v - 0.5).abs() > 0.001)
        .map(|(n, v)| format!("{n} {v:.2}"))
        .collect();
    out.push(if traits.is_empty() { "genome: all average".to_string() } else { format!("genome: {}", traits.join("  ")) });
    if !s.prey_preference.is_empty() {
        let prey: Vec<String> = s.prey_preference.iter().map(|(k, v)| format!("{k} {v:.2}")).collect();
        out.push(format!("prey: {}", prey.join("  ")));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// R9: the message holds the documented list and nothing else.
    #[test]
    fn designer_prompt_sends_exactly_the_documented_data() {
        let p = Params::default();
        let msgs = messages("a boar: omnivore, big litters", &p.species, None);
        let system = &msgs[0].content;
        let user = &msgs[1].content;
        assert!(user.contains("a boar: omnivore, big litters"));
        assert!(system.contains("- glyph:"), "field docs missing");
        assert!(system.contains("\"name\":\"boar\""), "example missing");
        for id in p.species.ids() {
            assert!(system.contains(&format!("- {} (", p.species.name(id))));
        }
        for forbidden in ["hunger_base", "base_url", "regrowth_rate", "ui.toml"] {
            assert!(!system.contains(forbidden) && !user.contains(forbidden), "{forbidden} leaked");
        }
        let again = messages("a boar", &p.species, Some(("{}", "species '' has no name")));
        assert!(again[1].content.contains("rejected by the loader with: species '' has no name"));
    }

    #[test]
    fn summary_describes_a_roster_entry() {
        let p = Params::default();
        let lines = summary(&p, "wolf");
        assert!(lines[0].starts_with("wolf / Wolves"), "{lines:?}");
        assert!(lines.iter().any(|l| l.starts_with("prey: ")), "{lines:?}");
        assert_eq!(summary(&p, "boar")[0], "boar: not in the roster");
    }
}
