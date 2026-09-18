//! S09 key handling: typed entry, text fields, arrow adjustment and presets.

use ratatui::crossterm::event::KeyCode;

use crate::sim::params::{Difficulty, Rainfall};
use crate::sim::{Params, PRESETS};
use crate::ui::app::AppState;
use crate::ui::screens::Action;
use super::{generate, WorldGenForm, F_AGE, F_HEIGHT, F_RAINFALL, F_ROCK, F_SEASON, F_SPECIES, F_WATER, F_WIDTH, F_FOREST, T_DIFFICULTY, T_MUTATION_RATE, T_MUTATION_STRENGTH, T_REGROWTH};

/// Apply a preset (C6 FR7): write its values into the form fields. Balanced
/// (index 0) resets those fields to the defaults; the name, seed and species
/// counts are never touched by presets.
pub(super) fn apply_preset(form: &mut WorldGenForm, idx: usize) {
    let preset = &PRESETS[idx];
    if idx == 0 {
        let d = Params::default();
        form.world = d.world;
        form.season_days = d.time.season_days;
        form.genetics = d.genetics;
        form.predation = d.predation;
        form.regrowth_rate = d.ecology.regrowth_rate;
    } else {
        let mut p = form.build_params();
        if let Err(e) = p.apply_overlay(preset.overlay) {
            eprintln!("preset error: {e}");
            return;
        }
        form.world = p.world;
        form.season_days = p.time.season_days;
        form.genetics = p.genetics;
        form.predation = p.predation;
        form.regrowth_rate = p.ecology.regrowth_rate;
    }
    form.dirty = true;
}

pub(super) fn adjust(form: &mut WorldGenForm, focus: usize, dir: i32) {
    let d = i64::from(dir);
    let w = &mut form.world;
    match focus {
        // Left/Right adjust the width and height fields, like every other field.
        F_WIDTH => w.width = crate::cast!(clamp_i64(crate::cast!(w.width => i64) + d * 10, 30, 1000) => usize),
        F_HEIGHT => w.height = crate::cast!(clamp_i64(crate::cast!(w.height => i64) + d * 10, 30, 1000) => usize),
        F_WATER => w.water_pct = crate::cast!(clamp_i64(i64::from(w.water_pct) + d, 0, 100) => u8),
        F_FOREST => w.forest_pct = crate::cast!(clamp_i64(i64::from(w.forest_pct) + d, 0, 100) => u8),
        F_ROCK => w.rock_pct = crate::cast!(clamp_i64(i64::from(w.rock_pct) + d, 0, 100) => u8),
        F_AGE => w.age = crate::cast!(clamp_i64(i64::from(w.age) + d, 0, 200) => u8),
        F_RAINFALL => cycle_rainfall(&mut w.rainfall, dir),
        F_SEASON => form.season_days = crate::cast!(clamp_i64(i64::from(form.season_days) + d * 10, 30, 360) => u32),
        F_SPECIES.. if focus < form.tail() => {
            let i = focus - F_SPECIES;
            form.counts[i] = crate::cast!(clamp_i64(i64::from(form.counts[i]) + d, 0, 999) => u32);
        }
        _ => match focus - form.tail() {
            T_MUTATION_RATE => form.genetics.mutation_rate = clamp_f32(form.genetics.mutation_rate + crate::cast!(dir => f32) * 0.01, 0.0, 1.0),
            T_MUTATION_STRENGTH => form.genetics.mutation_strength = clamp_f32(form.genetics.mutation_strength + crate::cast!(dir => f32) * 0.01, 0.0, 1.0),
            T_DIFFICULTY => cycle_difficulty(&mut form.predation.difficulty, dir),
            T_REGROWTH => form.regrowth_rate = clamp_f32(form.regrowth_rate + crate::cast!(dir => f32) * 0.1, 0.0, 10.0),
            _ => {}
        },
    }
}

/// Fields whose value can be typed in directly (Space opens the entry).
pub(super) fn is_numeric_field(form: &WorldGenForm, focus: usize) -> bool {
    let tail = form.tail();
    matches!(focus, F_WIDTH..=F_AGE) || (F_SEASON..=tail + T_MUTATION_STRENGTH).contains(&focus) || focus == tail + T_REGROWTH
}

/// Apply a typed value to a numeric field, clamped to the same range the
/// arrow keys use. Empty or unparseable input leaves the field unchanged.
pub(super) fn commit_edit(form: &mut WorldGenForm, focus: usize, text: &str) {
    let text = text.trim();
    if text.is_empty() {
        return;
    }
    let int = |t: &str| t.parse::<i64>().ok();
    let flt = |t: &str| t.parse::<f32>().ok().filter(|v| v.is_finite());
    let w = &mut form.world;
    let changed = match focus {
        F_WIDTH => int(text).map(|v| w.width = crate::cast!(clamp_i64(v, 100, 1000) => usize)).is_some(),
        F_HEIGHT => int(text).map(|v| w.height = crate::cast!(clamp_i64(v, 30, 1000) => usize)).is_some(),
        F_WATER => int(text).map(|v| w.water_pct = crate::cast!(clamp_i64(v, 0, 60) => u8)).is_some(),
        F_FOREST => int(text).map(|v| w.forest_pct = crate::cast!(clamp_i64(v, 0, 50) => u8)).is_some(),
        F_ROCK => int(text).map(|v| w.rock_pct = crate::cast!(clamp_i64(v, 0, 30) => u8)).is_some(),
        F_AGE => int(text).map(|v| w.age = crate::cast!(clamp_i64(v, 0, 30) => u8)).is_some(),
        F_SEASON => int(text).map(|v| form.season_days = crate::cast!(clamp_i64(v, 30, 180) => u32)).is_some(),
        F_SPECIES.. if focus < form.tail() => int(text).map(|v| form.counts[focus - F_SPECIES] = crate::cast!(clamp_i64(v, 0, 999) => u32)).is_some(),
        _ => match focus - form.tail() {
            T_MUTATION_RATE => flt(text).map(|v| form.genetics.mutation_rate = clamp_f32(v, 0.0, 0.2)).is_some(),
            T_MUTATION_STRENGTH => flt(text).map(|v| form.genetics.mutation_strength = clamp_f32(v, 0.0, 0.2)).is_some(),
            T_REGROWTH => flt(text).map(|v| form.regrowth_rate = clamp_f32(v, 0.2, 2.0)).is_some(),
            _ => false,
        },
    };
    if changed {
        form.dirty = true;
    }
}

pub(super) fn clamp_i64(v: i64, lo: i64, hi: i64) -> i64 {
    v.clamp(lo, hi)
}

pub(super) const fn clamp_f32(v: f32, lo: f32, hi: f32) -> f32 {
    v.clamp(lo, hi)
}

pub(super) fn cycle_rainfall(r: &mut Rainfall, dir: i32) {
    let all = [Rainfall::Dry, Rainfall::Normal, Rainfall::Wet];
    let i = crate::cast!(all.iter().position(|x| x == r).unwrap_or(1) => i32);
    *r = all[crate::cast!(((i + dir).rem_euclid(3)) => usize)];
}

pub(super) fn cycle_difficulty(d: &mut Difficulty, dir: i32) {
    let all = [Difficulty::Easy, Difficulty::Normal, Difficulty::Hard];
    let i = crate::cast!(all.iter().position(|x| x == d).unwrap_or(1) => i32);
    *d = all[crate::cast!(((i + dir).rem_euclid(3)) => usize)];
}

/// How far Tab moves the focus (`BackTab` wraps backwards).
pub(super) fn field_step(form: &WorldGenForm, code: KeyCode) -> usize {
    if code == KeyCode::BackTab {
        form.field_count() - 1
    } else {
        1
    }
}

/// Consume a key while a numeric field is in typed-entry mode.
pub(super) fn handle_typed_entry(form: &mut WorldGenForm, code: KeyCode) {
    match code {
        KeyCode::Char(c) if c.is_ascii_digit() || c == '.' => {
            if let Some(buf) = form.edit.as_mut().filter(|b| b.len() < 9) {
                buf.push(c);
            }
        }
        KeyCode::Backspace => {
            if let Some(buf) = form.edit.as_mut() {
                buf.pop();
            }
        }
        KeyCode::Enter => {
            let focus = form.focus;
            if let Some(text) = form.edit.take() {
                commit_edit(form, focus, &text);
            }
        }
        KeyCode::Tab | KeyCode::BackTab => {
            let focus = form.focus;
            if let Some(text) = form.edit.take() {
                commit_edit(form, focus, &text);
            }
            form.focus = (form.focus + field_step(form, code)) % form.field_count();
        }
        KeyCode::Esc => form.edit = None,
        _ => {}
    }
}

/// The name (0) and seed (1) text fields.
pub(super) fn handle_text_field(form: &mut WorldGenForm, focus: usize, code: KeyCode) -> Action {
    let is_name = focus == 0;
    match code {
        KeyCode::Char(c) if !c.is_control() => {
            let max = if is_name { 23 } else { 20 };
            if !form.text_edited {
                form.text_edited = true;
                form.field_mut(is_name).clear();
            }
            let full = if is_name { form.name.chars().count() >= max } else { form.seed_text.chars().count() >= max };
            if !full {
                form.field_mut(is_name).push(c);
            }
            if !is_name {
                form.dirty = true;
            }
            Action::None
        }
        KeyCode::Backspace => {
            if is_name {
                form.name.pop();
            } else {
                form.seed_text.pop();
            }
            form.text_edited = true;
            if !is_name {
                form.dirty = true;
            }
            Action::None
        }
        _ => Action::None,
    }
}

/// Left/Right adjust the focused field; Enter generates.
pub(super) fn handle_adjust(form: &mut WorldGenForm, focus: usize, code: KeyCode, app: &mut AppState) -> Action {
    match code {
        KeyCode::Left | KeyCode::Right => {
            let dir: i32 = if code == KeyCode::Right { 1 } else { -1 };
            adjust(form, focus, dir);
            form.dirty = true;
            Action::None
        }
        KeyCode::Enter => generate(form, app),
        _ => Action::None,
    }
}
