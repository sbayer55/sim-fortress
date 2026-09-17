//! S09 form drawing: the world, species, evolution and preset sections as a
//! stack of components, with the action buttons pinned to the bottom.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::Frame;

use crate::sim::params::{Difficulty, Rainfall};
use crate::sim::Kind;
use crate::ui::screens::common::sp;
use crate::ui::style::SpeciesStyle;
use crate::widgets::Constraint::{Fill, Fixed};
use crate::widgets::{Bar, Block, ButtonRow, Columns, Component, Divider, Panel, Row, Rows, Spacer, Stepper, Text, TextField, VStack};
use crate::{glyphs, theme};
use super::{shown, WorldGenForm, F_AGE, F_HEIGHT, F_SEASON, F_SPECIES, F_WATER, F_WIDTH, F_FOREST, F_ROCK, T_BACK, T_GENERATE, T_MUTATION_RATE, T_MUTATION_STRENGTH, T_PRESETS, T_RANDOMIZE, T_REGROWTH};

pub(super) fn form_panel(f: &mut Frame<'_>, area: Rect, form: &WorldGenForm) {
    let hint = format!("field {} of {}", form.focus + 1, form.field_count());
    let buf = f.buffer_mut();
    let inner = Panel::new("New World").info(hint).render(buf, area);
    let mut rows = world_section(form);
    rows.extend(species_section(form));
    rows.extend(evolution_section(form));
    rows.extend(presets_section(form));
    let tail = form.tail();
    let button_focus = [T_GENERATE, T_RANDOMIZE, T_BACK].iter().position(|&t| form.focus == tail + t);
    let (rest, buttons) = (Spacer::rows(0), ButtonRow::new(&["[ Generate ]", "[ Randomize seed ]", "[ Back ]"]).focused(button_focus).primary(0).left(4).gap(3));
    VStack::from_boxes(&rows).child_with(Fill(1), &rest).child(&buttons).render(buf, inner);
}

/// A full-width Stepper or Text Field row.
fn field(label: &'static str, value: String, adjustable: bool, hint: &'static str, focused: bool) -> Box<dyn Component + 'static> {
    if adjustable {
        Box::new(Stepper::new(label, value).hint(hint).focused(focused))
    } else {
        Box::new(TextField::new(label, value).hint(hint).focused(focused))
    }
}

/// World shape and season fields.
fn world_section(form: &WorldGenForm) -> Rows<'static> {
    let wf = [
        ("World name", form.name.clone(), false, "text"),
        ("Seed", form.seed_text.clone(), false, "hex/decimal"),
        ("Map width", shown(form, F_WIDTH, form.world.width.to_string()), true, "100 - 1000 cells"),
        ("Map height", shown(form, F_HEIGHT, form.world.height.to_string()), true, "30 - 1000 cells"),
        ("Water %", shown(form, F_WATER, form.world.water_pct.to_string()), true, "lakes + rivers"),
        ("Forest %", shown(form, F_FOREST, form.world.forest_pct.to_string()), true, "predator cover"),
        ("Rock %", shown(form, F_ROCK, form.world.rock_pct.to_string()), true, "impassable"),
        ("World age", shown(form, F_AGE, form.world.age.to_string()), true, "erosion 0 - 30"),
        ("Rainfall", rainfall_name(form.world.rainfall).to_string(), true, "dry/normal/wet"),
        ("Season length", shown(form, F_SEASON, format!("{} days", form.season_days)), true, "30 - 180 days"),
    ];
    let mut rows: Rows<'static> = vec![Box::new(Divider::new("World"))];
    // The world rows are in Tab order starting at focus 0, so the array index is the focus index.
    for (i, (label, value, adjustable, hint)) in wf.into_iter().enumerate() {
        rows.push(field(label, value, adjustable, hint, i == form.focus));
    }
    rows.push(Box::new(Spacer::rows(1)));
    rows
}

/// The species table columns: margin, glyph, name, kind, count stepper, gap, share bar, gap, percent.
fn species_columns() -> Columns {
    Columns::new(&[Fixed(1), Fixed(2), Fixed(10), Fixed(9), Fixed(8), Fixed(3), Fixed(22), Fixed(1), Fixed(4), Fill(1)])
}

/// The initial-species roster with counts and population shares.
fn species_section(form: &WorldGenForm) -> Rows<'static> {
    let total: u32 = form.counts.iter().sum();
    let mut table = Vec::new();
    let mut selected = None;
    for id in form.species.ids() {
        let i = id.index();
        let focused = form.focus == F_SPECIES + i;
        if focused {
            selected = Some(i);
        }
        let bg = if focused { theme::SELECT_BG } else { theme::PANEL_BG };
        let base = if focused { theme::selected() } else { theme::text() };
        let kind = if form.species.kind(id) == Kind::Prey { "prey" } else { "predator" };
        let share = crate::cast!(form.counts[i] => f32) / crate::cast!(total.max(1) => f32);
        table.push(
            Row::new()
                .cell(Spacer::cols(1))
                .cell(Text::new(format!("{} ", form.species.adult_glyph(id))).style(Style::default().fg(form.species.color(id)).bg(bg).add_modifier(Modifier::BOLD)))
                .cell(Text::new(format!("{:<10}", form.species.plural(id))).style(base))
                .cell(Text::new(format!("{kind:<9}")).style(Style::default().fg(theme::DIM).bg(bg)))
                .cell(Stepper::compact(shown(form, F_SPECIES + i, form.counts[i].to_string())).focused(focused))
                .cell(Spacer::cols(3))
                .cell(Bar::new(share).color(form.species.color(id)))
                .cell(Spacer::cols(1))
                .cell(Text::new(format!("{:>3}%", crate::cast!((share * 100.0).round() => u32))).style(Style::default().fg(theme::TEXT).bg(bg))),
        );
    }
    vec![
        Box::new(Divider::new("Initial species")),
        Box::new(Text::spans(vec![sp("   species    kind       count   ", theme::dim_text()), sp("share of starting population", theme::dim_text())])),
        Box::new(Block::new(species_columns()).rows(table).selected(selected)),
        Box::new(Text::new(format!("   total {}   prey {}   predators {}", total, kind_total(form, Kind::Prey), kind_total(form, Kind::Predator))).style(theme::dim_text())),
        Box::new(Spacer::rows(1)),
    ]
}

/// The evolution-tuning fields.
fn evolution_section(form: &WorldGenForm) -> Rows<'static> {
    let tail = form.tail();
    let evo = [
        ("Mutation rate", shown(form, tail + T_MUTATION_RATE, format!("{:.2}", form.genetics.mutation_rate)), "per trait/birth"),
        ("Mutation strength", shown(form, tail + T_MUTATION_STRENGTH, format!("{:.2}", form.genetics.mutation_strength)), "mutation sd"),
        ("Predation difficulty", difficulty_name(form.predation.difficulty).to_string(), "easy/norm/hard"),
        ("Regrowth rate", shown(form, tail + T_REGROWTH, format!("{:.1}", form.regrowth_rate)), "veg multiplier"),
    ];
    let mut rows: Rows<'static> = vec![Box::new(Divider::new("Evolution"))];
    for (i, (label, value, hint)) in evo.into_iter().enumerate() {
        rows.push(field(label, value, true, hint, form.focus == tail + T_MUTATION_RATE + i));
    }
    rows.push(Box::new(Text::spans(vec![
        sp(format!(" {} ", glyphs::NOTE), theme::dim_text()),
        sp("Higher mutation strength speeds adaptation but raises the", theme::dim_text()),
    ])));
    rows.push(Box::new(Text::new("   chance of unviable offspring.").style(theme::dim_text())));
    rows.push(Box::new(Spacer::rows(1)));
    rows
}

/// The preset list.
fn presets_section(form: &WorldGenForm) -> Rows<'static> {
    let presets: [(&str, &str); 5] = [
        ("Balanced", "default values, gentle seasons"),
        ("Harsh winter", "180-day seasons, regrowth 0.6"),
        ("Lush", "forest 30%, regrowth 1.4, predation hard"),
        ("Archipelago", "water 55%, islands isolate lineages"),
        ("Fast evolution", "mutation rate 0.10, strength 0.12"),
    ];
    let mut rows: Rows<'static> = vec![Box::new(Divider::new("Presets"))];
    for (i, (name, desc)) in presets.iter().enumerate() {
        let focused = form.focus == form.tail() + T_PRESETS + i;
        rows.push(Box::new(Text::spans(vec![
            sp(format!(" {} ", glyphs::DOT), if focused { theme::label() } else { theme::dim_text() }),
            sp(format!("{name:<16}"), if focused { theme::title() } else { theme::text() }),
            sp(*desc, theme::dim_text()),
        ])));
    }
    rows
}

fn kind_total(form: &WorldGenForm, kind: Kind) -> u32 {
    form.species.ids().filter(|&id| form.species.kind(id) == kind).map(|id| form.counts[id.index()]).sum()
}

const fn rainfall_name(r: Rainfall) -> &'static str {
    match r {
        Rainfall::Dry => "dry",
        Rainfall::Normal => "normal",
        Rainfall::Wet => "wet",
    }
}

const fn difficulty_name(d: Difficulty) -> &'static str {
    match d {
        Difficulty::Easy => "easy",
        Difficulty::Normal => "normal",
        Difficulty::Hard => "hard",
    }
}
