//! The Stack section every overlay sidebar ends with, the compact sidebar
//! for two or more layers, the per-layer legend rows the S14 switcher shares,
//! and the generic heatmap sidebar (S02a–c).

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;
use crate::sim::{Sim, World};
use crate::ui::screens::common::sp;
use crate::ui::style::SpeciesStyle;
use crate::widgets::map::{Base, Disease, Layer, OverlayStack};
use crate::widgets::{bars, panel, util, Component, Divider, Legend, Panel, Rows, Spacer, Text, VStack};
use crate::{glyphs, theme};
use super::disease_overlay::immune_to_shown;
use super::WorldMap;

/// The Stack section (S14 item 20): `─ Stack ─`, one row per active layer in
/// composition order, then the dim hint.
pub fn stack_section(stack: &OverlayStack) -> Rows<'static> {
    let mut rows: Rows<'static> = vec![Box::new(Divider::new("Stack"))];
    for (i, layer) in stack.layers().enumerate() {
        rows.push(Box::new(Text::new(format!(" {}. {:<9} ({})", i + 1, layer.name(), layer.kind()))));
    }
    rows.push(Box::new(Text::new(" o edits the stack   Esc clears all").style(theme::dim_text())));
    rows
}

/// The sub-pick shown after the layer name (` · deer`), empty for layers
/// without one.
pub fn sub_pick_text(layer: Layer, stack: &OverlayStack, sim: &Sim) -> String {
    match layer {
        Layer::Base(Base::Species | Base::Scent) => sim.roster().name(stack.species).to_string(),
        Layer::Sense => stack.sense_subject.and_then(|id| sim.creatures.get(id)).map_or_else(|| "no predators".to_string(), |c| c.name_str(sim.roster()).to_string()),
        Layer::Disease => match stack.disease {
            Disease::On(Some(p)) => sim.disease.name(p).to_string(),
            Disease::On(None) => "All pathogens".to_string(),
            Disease::Off => stack.pathogen.map_or_else(|| "All pathogens".to_string(), |p| sim.disease.name(p).to_string()),
        },
        _ => String::new(),
    }
}

/// The 24-cell heatmap sample on `ramp`, built from the map's shade glyphs,
/// with the `0% … 100%` ticks beneath (S02 item 14).
pub(super) fn ramp_rows(ramp: &dyn Fn(f32) -> Color) -> Rows<'static> {
    let mut spans = vec![sp("    ", theme::text())];
    for i in 0..24u16 {
        let t = (f32::from(i) + 0.5) / 24.0;
        let c = ramp(t);
        spans.push(sp(glyphs::shade(t).to_string(), Style::default().fg(c).bg(theme::dim(c, 0.75))));
    }
    vec![Box::new(Text::spans(spans)), Box::new(Text::new("  0%      25%      50%      75%      100%").style(theme::dim_text()))]
}

/// The health bands: `■ fit  ■ strained  ■ critical` (S14 item 12).
pub(super) fn health_legend() -> Rows<'static> {
    vec![Box::new(Legend::new(&[(glyphs::SQUARE, theme::GOOD, "fit"), (glyphs::SQUARE, theme::WARN, "strained"), (glyphs::SQUARE, theme::BAD, "critical")]).columns(3).label_w(9))]
}

/// Every region as a `■` swatch, its name and its living count.
pub(super) fn region_legend(sim: &Sim) -> Rows<'static> {
    let world = &sim.world;
    world
        .regions
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let n = sim.creatures.living().filter(|c| world.region_index(c.x, c.y) == i).count();
            let row: Box<dyn Component> = Box::new(Text::spans(vec![
                sp(format!(" {} ", glyphs::SQUARE), Style::default().fg(theme::region(i)).bg(theme::PANEL_BG)),
                sp(format!("{:<16} ", r.0), theme::text()),
                sp(format!("{n} creatures"), theme::dim_text()),
            ]));
            row
        })
        .collect()
}

/// The disease counts: `☻ N sick   ☺ N immune` for the shown pathogen(s).
pub(super) fn disease_legend(stack: &OverlayStack, sim: &Sim) -> Rows<'static> {
    let shown = match stack.disease {
        Disease::On(shown) => shown,
        Disease::Off => stack.pathogen,
    };
    let day = crate::cast!(sim.time.day_index() => u32);
    let slots = sim.disease.pathogens.len();
    let sick = sim.creatures.living().filter(|c| c.infection.is_some_and(|i| shown.is_none_or(|p| p == i.pathogen))).count();
    let immune = sim.creatures.living().filter(|c| immune_to_shown(c, shown, slots, day)).count();
    vec![Box::new(Text::spans(vec![
        sp(format!(" {} {sick} sick", glyphs::DISEASE), Style::default().fg(theme::SICK).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
        sp(format!("   {} {immune} immune", glyphs::IMMUNE), Style::default().fg(theme::IMMUNE).bg(theme::PANEL_BG)),
    ]))]
}

/// The S02k legend ramp: wearing ground on the heat ramp over the left half,
/// climbing ground on the vegetation ramp over the right.
pub(super) fn succession_ramp(t: f32) -> Color {
    if t < 0.5 {
        theme::heat(1.0 - 2.0 * t)
    } else {
        theme::veg(2.0 * t - 1.0)
    }
}

/// The legend rows of one layer (S14 item 12): a ramp sample, the health
/// bands, the region swatches, the disease counts, a ring note, or the dim
/// `terrain and creatures only` for the None base.
pub fn layer_legend(layer: Layer, stack: &OverlayStack, sim: &Sim) -> Rows<'static> {
    match layer {
        Layer::Base(Base::None) => vec![Box::new(Text::new(" terrain and creatures only").style(theme::dim_text()))],
        Layer::Base(Base::Vegetation) => ramp_rows(&theme::veg),
        Layer::Base(Base::Pressure) => ramp_rows(&theme::heat),
        Layer::Base(Base::Moisture) => ramp_rows(&theme::water),
        Layer::Base(Base::Species) => {
            let color = sim.roster().color(stack.species);
            ramp_rows(&move |t| theme::species_ramp(color, t))
        }
        Layer::Base(Base::Parasites) => ramp_rows(&theme::parasite),
        Layer::Base(Base::Scent) => {
            let color = sim.roster().color(stack.species);
            ramp_rows(&move |t| theme::species_ramp(color, t))
        }
        Layer::Base(Base::Succession) => ramp_rows(&succession_ramp),
        Layer::Sense => vec![Box::new(Text::new(format!(" {} ring edge   tinted cells are in range", glyphs::RING)).style(theme::dim_text()))],
        Layer::Regions => region_legend(sim),
        Layer::Health => health_legend(),
        Layer::Disease => disease_legend(stack, sim),
    }
}

/// One layer's compact section: its name (and sub-pick) as a rule, its
/// description and its legend, then a blank row.
fn compact_section(layer: Layer, stack: &OverlayStack, sim: &Sim) -> Rows<'static> {
    let pick = sub_pick_text(layer, stack, sim);
    let title = if pick.is_empty() { layer.name().to_string() } else { format!("{} · {pick}", layer.name()) };
    let mut rows: Rows<'static> = vec![Box::new(Divider::new(title)), Box::new(Text::new(format!(" {}", layer.description())).style(theme::dim_text()))];
    rows.extend(layer_legend(layer, stack, sim));
    rows.push(Box::new(Spacer::rows(1)));
    rows
}

impl WorldMap {
    /// Render the Stack section from `row` of a wrapper-drawn sidebar.
    pub(super) fn stack_rows(f: &mut Frame<'_>, inner: Rect, row: u16, stack: &OverlayStack) {
        let rows = stack_section(stack);
        VStack::from_boxes(&rows).render(f.buffer_mut(), Rect { y: inner.y + row, height: inner.height.saturating_sub(row), ..inner });
    }

    /// The compact sidebar for two or more layers (S14 item 20): one section
    /// per layer in composition order, then the Stack section. Sections that
    /// do not fit are cut from the bottom; the Stack section always survives.
    pub(super) fn compact_sidebar(f: &mut Frame<'_>, area: Rect, sim: &Sim, stack: &OverlayStack) {
        let buf = f.buffer_mut();
        let inner = Panel::new("Overlay").render(buf, area);
        let mut sections: Rows<'static> = Vec::new();
        for layer in stack.layers() {
            sections.extend(compact_section(layer, stack, sim));
        }
        let tail = stack_section(stack);
        let tail_h = crate::cast!(tail.len() => u16).min(inner.height);
        let head = Rect { height: inner.height - tail_h, ..inner };
        VStack::from_boxes(&sections).render(buf, head);
        VStack::from_boxes(&tail).render(buf, Rect { y: head.bottom(), height: tail_h, ..inner });
    }

    /// The S02a–c heatmap sidebar.
    pub(super) fn overlay_sidebar(f: &mut Frame<'_>, area: Rect, world: &World, stack: &OverlayStack) {
        let inner = panel::draw(f, area, "Overlay", panel::Kind::Outer);
        let mut row = 0u16;

        let (name, desc1, desc2, low, high, note) = match stack.base {
            Base::Vegetation => ("Vegetation density", "standing biomass per cell;", "prey graze it down, regrowth (*) restores it.", "bare", "lush", "≈ deep water  ▲ rock (not shaded)"),
            Base::Pressure => ("Population pressure", "traffic of prey (x0.5) and predators (x0.7)", "prey leave pressure as they move", "quiet", "crowded", "≈ deep water  ▲ rock (not shaded)"),
            Base::Moisture => ("Water & moisture", "soil moisture; open water is shown saturated;", "drives regrowth and thirst.", "arid", "wet", "open water counts as 100% moisture"),
            _ => return,
        };

        panel::section(f, inner, row, name);
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(format!(" {desc1}"), theme::dim_text())));
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(format!(" {desc2}"), theme::dim_text())));
        row += 1;

        panel::section(f, inner, row, "Legend");
        row += 1;
        let ramp = |t: f32| -> Color {
            match stack.base {
                Base::Vegetation => theme::veg(t),
                Base::Pressure => theme::heat(t),
                Base::Moisture => theme::water(t),
                _ => theme::DIM,
            }
        };
        let legend = ramp_rows(&ramp);
        VStack::from_boxes(&legend).render(f.buffer_mut(), Rect { y: inner.y + row, height: 2, ..inner });
        row += 2;
        util::line(f, inner, row, Line::from(Span::styled(format!(" {low} … {high}"), theme::dim_text())));
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(format!(" {note}"), theme::dim_text())));
        row += 2;

        panel::section(f, inner, row, "By region");
        row += 1;
        for (ri, r) in world.regions.iter().enumerate() {
            let mean = match stack.base {
                Base::Vegetation => crate::sim::ecology::region_land_veg_mean(world, ri),
                Base::Moisture => crate::sim::ecology::region_display_moisture_mean(world, ri),
                _ => 0.0,
            };
            bars::labeled(f.buffer_mut(), inner, row, &format!(" {}", r.0), mean, ramp(0.8), 18, 14);
            row += 1;
        }
        row += 1;

        panel::section(f, inner, row, "Reading the map");
        row += 1;
        for note in [" creatures & resources faded", " Esc restores the plain map", " ░ <25%  ▒ <50%  ▓ <75%  █ ≥75%"] {
            util::line(f, inner, row, Line::from(Span::styled(note, theme::dim_text())));
            row += 1;
        }
        row += 1;
        Self::stack_rows(f, inner, row, stack);
    }
}
