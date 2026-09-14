//! Tests for the params module.

use super::Params;
use super::world::Rainfall;
use super::predation::Difficulty;
use super::genetics::GeneticsParams;
use super::social::SocialParams;
use super::presets::PRESETS;
use std::collections::BTreeMap;
use crate::sim::species::SpeciesId;

#[test]
fn toml_round_trip() {
    let p = Params::default();
    let text = p.to_toml().unwrap();
    let q = Params::from_toml(&text).unwrap();
    assert_eq!(p, q);
}

#[test]
fn unknown_key_is_error() {
    let err = Params::from_toml("[world]\nrainfall = \"dry\"\nbogus = 1\n").unwrap_err();
    assert!(err.to_string().contains("bogus"), "error should name the key: {err}");
}

#[test]
fn partial_table_deep_merges() {
    let p = Params::from_toml("[world]\nrainfall = \"dry\"\n").unwrap();
    assert_eq!(p.world.rainfall, Rainfall::Dry);
    // Everything else falls back to default.
    assert_eq!(p.world.width, 150);
    assert_eq!(p.time.season_days, 90);
    assert_eq!(p.events.capacity, 5000);
    assert_eq!(p.ecology.growth_k, 0.18);
}

#[test]
fn unknown_key_errors() {
    let err = Params::from_toml("[world]\nrainfall = \"dry\"\nbogus = 1\n").unwrap_err();
    assert!(err.to_string().contains("bogus"), "error should name the key: {err}");
    let mut p = Params::default();
    let e = p.apply_overlay("[predation]\nkill_bogus = 1\n").unwrap_err();
    assert!(e.contains("kill_bogus"), "overlay error should name the key: {e}");
}

#[test]
fn dump_params_round_trip() {
    let dump = Params::dump_toml();
    assert!(dump.contains("# "), "dump should carry comments");
    let parsed = Params::from_toml(&dump).unwrap();
    assert_eq!(parsed, Params::default());
}

#[test]
fn preset_overlay_merge() {
    let mut p = Params::default();
    p.apply_overlay(PRESETS[1].overlay).unwrap(); // Harsh winter
    assert_eq!(p.time.season_days, 180);
    assert_eq!(p.ecology.regrowth_rate, 0.6);

    let mut p = Params::default();
    p.apply_overlay(PRESETS[2].overlay).unwrap(); // Lush
    assert_eq!(p.world.forest_pct, 30);
    assert_eq!(p.ecology.regrowth_rate, 1.4);
    assert_eq!(p.predation.difficulty, Difficulty::Hard);

    let mut p = Params::default();
    p.apply_overlay(PRESETS[3].overlay).unwrap(); // Archipelago
    assert_eq!(p.world.water_pct, 55);

    let mut p = Params::default();
    p.apply_overlay(PRESETS[4].overlay).unwrap(); // Fast evolution
    assert_eq!(p.genetics.mutation_rate, 0.10);
    assert_eq!(p.genetics.mutation_strength, 0.12);

    assert_eq!(PRESETS[0].overlay, "", "Balanced is the defaults");
}

#[test]
fn maturity_factor_is_neutral_at_half() {
    let p = GeneticsParams::default();
    assert_eq!(GeneticsParams::maturity_factor(0.5, p.maturity_age_span), 1.0);
    assert_eq!(GeneticsParams::maturity_factor(0.5, p.maturity_lifespan_span), 1.0);
    // Slow (high maturity) means later, larger, longer; fast means the reverse.
    assert!(GeneticsParams::maturity_factor(0.98, p.maturity_age_span) > 1.0);
    assert!(GeneticsParams::maturity_factor(0.02, p.maturity_age_span) < 1.0);
    assert!(GeneticsParams::maturity_factor(0.98, p.maturity_litter_span) > 1.0);
    assert!(GeneticsParams::maturity_factor(0.98, p.maturity_lifespan_span) > 1.0);
    // Litter never drops below one, however fast the life history.
    assert_eq!(p.litter_size(SpeciesId::Deer, 0.0, 0.02), 1);
}

#[test]
fn social_defaults_documented() {
    let s = SocialParams::default();
    assert!(s.group_size_max > 0.0 && s.cohesion_min > 0.0);
    assert!(s.pack_share > 0.0 && s.pack_share <= 1.0);
    assert!((1..=3).contains(&(s.pack_share_cheb.min(3))));
}

#[test]
fn field_docs_complete() {
    let value = toml::Value::try_from(Params::default()).unwrap();
    let docs: BTreeMap<&str, &str> = Params::field_docs().iter().copied().collect();
    let map_fields: &[&str] = &[
        "creatures.initial_counts",
        "creatures.adult_age_days",
        "genetics.gestation_days",
        "genetics.litter_max",
        "genetics.mate_cooldown_days",
        "predation.cover_by_terrain",
        "predation.prey_preference",
        "ecology.max_vegetation",
        "ecology.season_cap",
        "ecology.season_regrowth",
        "ecology.season_evaporation",
        "ecology.season_metabolism",
        "ecology.rain_chance_per_day",
    ];
    let mut leaves: Vec<String> = Vec::new();
    collect_leaves(&value, "", map_fields, &mut leaves);
    for leaf in &leaves {
        assert!(docs.contains_key(leaf.as_str()), "missing doc for {leaf}");
    }
    for path in docs.keys() {
        assert!(leaves.iter().any(|l| l == path), "stale doc for {path}");
    }
}

fn collect_leaves(v: &toml::Value, prefix: &str, map_fields: &[&str], out: &mut Vec<String>) {
    let join = |p: &str, k: &str| if p.is_empty() { k.to_string() } else { format!("{p}.{k}") };
    match v {
        toml::Value::Table(t) => {
            if !prefix.is_empty() && map_fields.contains(&prefix) {
                out.push(prefix.to_string());
                return;
            }
            for (k, val) in t {
                collect_leaves(val, &join(prefix, k), map_fields, out);
            }
        }
        _ => out.push(prefix.to_string()),
    }
}
