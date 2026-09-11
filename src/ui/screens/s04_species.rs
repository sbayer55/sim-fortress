//! S04: the live species browser — S04a table + selected-species summary and
//! S04b per-species trait distributions and drift (C4 FR7).

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::Frame;

use crate::sim::stats::SpeciesStats;
use crate::sim::{Kind, Sim, SpeciesId, TRAIT_NAMES};
use crate::ui::app::AppState;
use crate::ui::screens::common::{arrow_color, delta_style, downsample, sp, trait_color, trend_arrow, two};
use crate::ui::screens::{Action, Screen};
use crate::ui::style::SpeciesStyle;
use crate::widgets::{bars, panel, status, util};
use crate::{glyphs, theme};

const TABLE_H: u16 = 13;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortCol {
    Count,
    Births,
    Deaths,
    Generation,
    Name,
}

impl SortCol {
    pub fn next(self) -> SortCol {
        match self {
            SortCol::Count => SortCol::Births,
            SortCol::Births => SortCol::Deaths,
            SortCol::Deaths => SortCol::Generation,
            SortCol::Generation => SortCol::Name,
            SortCol::Name => SortCol::Count,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SortCol::Count => "count",
            SortCol::Births => "births",
            SortCol::Deaths => "deaths",
            SortCol::Generation => "generation",
            SortCol::Name => "name",
        }
    }
}

/// Species indices in display order for `sort`; ties keep species order.
pub fn sorted_indices(sim: &Sim, sort: SortCol) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..6).collect();
    let key = |i: usize| -> (i64, usize) {
        let s = &sim.species[i];
        let v = match sort {
            SortCol::Count => s.count as i64,
            SortCol::Births => s.births_yesterday as i64,
            SortCol::Deaths => s.deaths_yesterday as i64,
            SortCol::Generation => s.generation as i64,
            SortCol::Name => 0,
        };
        (-v, i)
    };
    match sort {
        SortCol::Name => idx.sort_by_key(|&i| (SpeciesId::ALL[i].name(), i)),
        _ => idx.sort_by_key(|&i| key(i)),
    }
    idx
}

pub struct SpeciesBrowser {
    /// Position in the sorted table.
    pub sel: usize,
    pub sort: SortCol,
}

impl Default for SpeciesBrowser {
    fn default() -> Self {
        Self::new()
    }
}

impl SpeciesBrowser {
    pub fn new() -> Self {
        SpeciesBrowser { sel: 0, sort: SortCol::Count }
    }

    fn selected_species(&self, sim: &Sim) -> SpeciesId {
        let order = sorted_indices(sim, self.sort);
        SpeciesId::ALL[order[self.sel.min(5)]]
    }
}

impl Screen for SpeciesBrowser {
    fn opaque(&self) -> bool {
        true
    }

    fn handle_key(&mut self, key: KeyEvent, app: &mut AppState) -> Action {
        match key.code {
            KeyCode::Up => {
                self.sel = self.sel.saturating_sub(1);
                Action::None
            }
            KeyCode::Down => {
                self.sel = (self.sel + 1).min(5);
                Action::None
            }
            KeyCode::Char('s') => {
                // Keep the same species selected across the re-sort.
                let species = app.sim.as_ref().map(|sim| self.selected_species(sim));
                self.sort = self.sort.next();
                if let (Some(sim), Some(species)) = (app.sim.as_ref(), species) {
                    let order = sorted_indices(sim, self.sort);
                    self.sel = order.iter().position(|&i| SpeciesId::ALL[i] == species).unwrap_or(0);
                }
                Action::None
            }
            KeyCode::Enter => match app.sim.as_ref() {
                Some(sim) => Action::Push(Box::new(SpeciesDetail::new(self.selected_species(sim)))),
                None => Action::None,
            },
            KeyCode::Esc => Action::Pop,
            _ => Action::Unhandled,
        }
    }

    fn render(&self, app: &AppState, f: &mut Frame, area: Rect) {
        let Some(sim) = &app.sim else { return };
        let status_row = area.y + area.height - 1;
        let body_h = area.height - 1;
        let species = self.selected_species(sim);
        table(f, Rect::new(area.x, area.y, area.width, TABLE_H), sim, self.sort, species);
        summary(f, Rect::new(area.x, area.y + TABLE_H, area.width, body_h - TABLE_H), sim, species);
        status::render(
            f,
            Rect::new(area.x, status_row, area.width, 1),
            &[("↑↓", "select"), ("Enter", "detail"), ("s", "sort"), ("Esc", "back")],
            &format!("sorted by {} {}  {}", self.sort.label(), glyphs::DOWN, sim.time.clock_label()),
        );
    }
}

fn kind_label(id: SpeciesId) -> &'static str {
    match id.kind() {
        Kind::Prey => "prey",
        Kind::Predator => "pred",
    }
}

// ------------------------------------------------------------------ S04a

fn table(f: &mut Frame, area: Rect, sim: &Sim, sort: SortCol, selected: SpeciesId) {
    let alive = sim.species.iter().filter(|s| s.count > 0).count();
    let prey_n = sim.species.iter().filter(|s| s.count > 0 && s.species.kind() == Kind::Prey).count();
    let inner = panel::draw_with_hint(f, area, "Species", &format!("{} species, {} prey / {} predator", alive, prey_n, alive - prey_n), panel::Kind::Outer);
    let dim = theme::dim_text();
    let header = Line::from(vec![
        sp("   ", dim),
        sp(format!("{:<8}", "Species"), dim),
        sp(format!("{:<6}", "Kind"), dim),
        sp(format!("{:>6}", "Count"), dim),
        sp(format!("{:>7}", "Adults"), dim),
        sp(format!("{:>6}", "Juv"), dim),
        sp(format!("{:>8}", "Birth/d"), dim),
        sp(format!("{:>8}", "Death/d"), dim),
        sp(format!("{:>6}", "Peak"), dim),
        sp(format!("{:>5}", "Gen"), dim),
        sp("  30-day trend         ", dim),
        sp("  ", dim),
        sp("  Spd Siz Sen Met Agg Cam Fer Lon", dim),
        sp("   Diet", dim),
    ]);
    util::line(f, inner, 0, header);
    let mut row = 2u16;
    for i in sorted_indices(sim, sort) {
        let s = &sim.species[i];
        let id = s.species;
        let is_sel = id == selected;
        let absent = s.count == 0;
        let base = if is_sel {
            theme::selected()
        } else if absent {
            theme::dim_text()
        } else {
            theme::text()
        };
        let bg = if is_sel { theme::SELECT_BG } else { theme::PANEL_BG };
        let dimmed = if is_sel { base } else { theme::dim_text() };
        let marker = if is_sel { glyphs::PLAY } else { ' ' };
        let arrow = trend_arrow(&s.trend);
        let mut spans = vec![
            sp(marker.to_string(), Style::default().fg(theme::KEY).bg(bg).add_modifier(Modifier::BOLD)),
            sp(format!("{} ", id.glyph().to_ascii_uppercase()), Style::default().fg(if absent { theme::DIM } else { id.color() }).bg(bg).add_modifier(Modifier::BOLD)),
            sp(format!("{:<8}", id.name()), base),
            sp(format!("{:<6}", kind_label(id)), dimmed),
            sp(format!("{:>6}", s.count), base),
            sp(format!("{:>7}", s.adults), base),
            sp(format!("{:>6}", s.juveniles), base),
            sp(format!("{:>8}", s.births_yesterday), Style::default().fg(if absent { theme::DIM } else { theme::GOOD }).bg(bg)),
            sp(format!("{:>8}", s.deaths_yesterday), Style::default().fg(if absent { theme::DIM } else { theme::BAD }).bg(bg)),
            sp(format!("{:>6}", s.peak), base),
            sp(format!("{:>5}", s.generation), base),
            sp(format!("{:<23}", ""), base), // sparkline slot
            sp(format!("{} ", arrow), Style::default().fg(arrow_color(arrow)).bg(bg).add_modifier(Modifier::BOLD)),
            sp("  ", base),
        ];
        for t in 0..8 {
            let st = if absent { Style::default().fg(theme::DIM).bg(bg) } else { Style::default().fg(trait_color(t)).bg(bg) };
            spans.push(sp(format!("{:>3} ", two(s.mean.0[t])), st));
        }
        spans.push(sp(format!("  {}", id.diet()), dimmed));
        util::line(f, inner, row, Line::from(spans));
        if is_sel {
            let buf = f.buffer_mut();
            for x in 0..inner.width {
                if let Some(c) = buf.cell_mut((inner.x + x, inner.y + row)) {
                    c.set_bg(bg);
                }
            }
        }
        if !s.trend.is_empty() {
            bars::sparkline(f.buffer_mut(), inner.x + 65, inner.y + row, 20, &s.trend, if absent { theme::DIM } else { id.color() });
        }
        row += 1;
    }
    // Totals row.
    row += 1;
    let prey: u32 = sim.species.iter().filter(|s| s.species.kind() == Kind::Prey).map(|s| s.count).sum();
    let pred: u32 = sim.species.iter().filter(|s| s.species.kind() == Kind::Predator).map(|s| s.count).sum();
    let births: u32 = (0..6).map(|i| sim.births_today(i)).sum();
    let deaths: u32 = (0..6).map(|i| sim.deaths_today(i)).sum();
    util::line(f, inner, row, Line::from(vec![
        sp(format!("   {:<14}", "totals"), theme::label()),
        sp(format!("{:>6}", prey + pred), theme::text()),
        sp(format!("   prey {}  pred {}  ratio {:.1}:1", prey, pred, prey as f32 / pred.max(1) as f32), theme::dim_text()),
        sp(format!("   births {}", births), Style::default().fg(theme::GOOD).bg(theme::PANEL_BG)),
        sp(format!("  deaths {}", deaths), Style::default().fg(theme::BAD).bg(theme::PANEL_BG)),
        sp(format!("   net {:+} today", births as i32 - deaths as i32), theme::text()),
        sp("     traits = species means x100;  /d = yesterday", theme::dim_text()),
    ]));
}

/// Narrative line from the 30-day change (FR7).
fn narrative(s: &SpeciesStats) -> (String, Style) {
    match s.change_pct() {
        Some(p) if p > 3.0 => (
            format!("{} {:+.0} % — births outpaced deaths over the last 30 days", glyphs::UP, p),
            Style::default().fg(theme::GOOD).bg(theme::PANEL_BG),
        ),
        Some(p) if p < -3.0 => (
            format!("{} {:+.0} % — deaths outpaced births over the last 30 days", glyphs::DOWN, p),
            Style::default().fg(theme::BAD).bg(theme::PANEL_BG),
        ),
        Some(_) => ("stable — births and deaths balanced over the last 30 days".to_string(), theme::dim_text()),
        None => ("no 30-day history yet".to_string(), theme::dim_text()),
    }
}

fn summary(f: &mut Frame, area: Rect, sim: &Sim, id: SpeciesId) {
    let s = &sim.species[id.index()];
    let inner = panel::draw_with_hint(f, area, &format!("Selected: {}", id.name()), "Enter for full detail", panel::Kind::Focus);
    let left_w = 64u16;
    let left = Rect::new(inner.x, inner.y, left_w, inner.height);
    let right = Rect::new(inner.x + left_w + 1, inner.y, inner.width - left_w - 1, inner.height);

    // ---- left: identity + trait table
    let mut row = 0u16;
    util::line(f, left, row, Line::from(vec![
        sp(format!(" {} ", id.glyph().to_ascii_uppercase()), Style::default().fg(id.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
        sp(id.plural(), theme::title()),
        sp(format!("   {}   diet: {}", if id.kind() == Kind::Prey { "prey" } else { "predator" }, id.diet()), theme::text()),
    ]));
    row += 1;
    util::line(f, left, row, Line::from(vec![
        sp(format!(" {} alive  {} adults  {} juveniles  generation {}  peak {}", s.count, s.adults, s.juveniles, s.generation, s.peak), theme::dim_text()),
    ]));
    row += 2;
    panel::section(f, left, row, "Base genome vs current mean");
    row += 1;
    util::line(f, left, row, Line::from(sp(" trait        base   current            delta   spread", theme::dim_text())));
    row += 1;
    let base = id.base_genome();
    for t in 0..8 {
        let b = base.0[t];
        let m = s.mean.0[t];
        let d = m - b;
        let color = trait_color(t);
        let y = left.y + row;
        let buf = f.buffer_mut();
        buf.set_stringn(left.x, y, format!(" {:<12}", TRAIT_NAMES[t]), 13, theme::text());
        buf.set_stringn(left.x + 13, y, format!("{:.2}", b), 4, theme::dim_text());
        buf.set_stringn(left.x + 20, y, format!("{:.2}", m), 4, theme::text());
        bars::bar(buf, left.x + 25, y, 14, m, color);
        let bx = left.x + 26 + (b * 11.0).round() as u16;
        buf.set_stringn(bx, y, glyphs::V_LINE.to_string(), 1, Style::default().fg(theme::TEXT_BRIGHT).bg(theme::PANEL_BG));
        buf.set_stringn(left.x + 42, y, format!("{:+.2}", d), 5, delta_style(d));
        bars::range(buf, left.x + 49, y, 13, s.min.0[t], m, s.max.0[t], color);
        row += 1;
    }
    util::line(f, left, row, Line::from(vec![
        sp(format!(" {} base marker   spread = min/mean/max across living {}", glyphs::V_LINE, id.plural()), theme::dim_text()),
    ]));
    row += 2;
    panel::section(f, left, row, "Interactions");
    row += 1;
    if id.kind() == Kind::Prey {
        let hunters: Vec<String> = SpeciesId::ALL
            .iter()
            .filter(|p| p.kind() == Kind::Predator && sim.params.predation.preference(**p, id) > 0.0)
            .map(|p| format!("{} {}", p.glyph().to_ascii_uppercase(), p.name()))
            .collect();
        if hunters.is_empty() {
            util::line(f, left, row, Line::from(vec![sp(" eaten by: ", theme::dim_text()), sp("none", theme::text())]));
        } else {
            util::line(f, left, row, Line::from(vec![sp(" eaten by: ", theme::dim_text()), sp(hunters.join(" and "), theme::text())]));
        }
        row += 1;
        let others: Vec<String> = SpeciesId::ALL
            .iter()
            .filter(|o| o.kind() == Kind::Prey && **o != id)
            .map(|o| format!("{} {}", o.glyph().to_ascii_uppercase(), o.name()))
            .collect();
        util::line(f, left, row, Line::from(vec![
            sp(" competes with ", theme::dim_text()),
            sp(others.join(" and "), theme::text()),
            sp(" for grass", theme::dim_text()),
        ]));
        row += 1;
    } else {
        let prey: Vec<String> = SpeciesId::ALL
            .iter()
            .filter(|p| p.kind() == Kind::Prey && sim.params.predation.preference(id, **p) > 0.0)
            .map(|p| format!("{} {}", p.glyph().to_ascii_uppercase(), p.name()))
            .collect();
        util::line(f, left, row, Line::from(vec![
            sp(" hunts ", theme::dim_text()),
            sp(if prey.is_empty() { "nothing".to_string() } else { prey.join(", ") }, theme::text()),
        ]));
        row += 1;
        let rivals: Vec<String> = SpeciesId::ALL
            .iter()
            .filter(|o| o.kind() == Kind::Predator && **o != id)
            .map(|o| format!("{} {}", o.glyph().to_ascii_uppercase(), o.name()))
            .collect();
        util::line(f, left, row, Line::from(vec![
            sp(" competes with ", theme::dim_text()),
            sp(rivals.join(" and "), theme::text()),
        ]));
        row += 1;
    }
    row += 1;
    panel::section(f, left, row, "Notable individuals");
    row += 1;
    let day = sim.time.day_index();
    let mut members: Vec<&crate::sim::Creature> = sim.creatures.living().filter(|c| c.species == id).collect();
    members.sort_by(|a, b| b.offspring.cmp(&a.offspring).then(b.age_days(day).cmp(&a.age_days(day))).then(a.id.cmp(&b.id)));
    if members.is_empty() {
        util::line(f, left, row, Line::from(sp(" none living", theme::dim_text())));
    }
    for c in members.iter().take(5) {
        if row >= left.height {
            break;
        }
        util::line(f, left, row, Line::from(vec![
            sp(format!(" {} ", if c.adult { id.glyph().to_ascii_uppercase() } else { id.glyph() }), Style::default().fg(id.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            sp(format!("{:<9}{:<7}", c.name_str(), c.tag()), theme::text()),
            sp(format!("{:>3} offspring  gen {:<3} {:>4} days  ", c.offspring, c.generation, c.age_days(day)), theme::dim_text()),
            sp(sim.world.region_name(c.x, c.y).to_string(), theme::dim_text()),
        ]));
        row += 1;
    }

    // ---- right: population history
    let mut row = 0u16;
    panel::section(f, right, row, "Population, last 240 days");
    row += 1;
    let samples = sim.series.samples();
    let start = samples.len().saturating_sub(240);
    let series: Vec<f32> = samples[start..].iter().map(|x| x.population[id.index()] as f32).collect();
    let drought: Vec<bool> = samples[start..].iter().map(|x| x.drought_regions >= 2).collect();
    let cols = 100usize;
    let data = downsample(&series, cols);
    let max = *data.iter().max().unwrap_or(&1) as f32;
    let min = *data.iter().min().unwrap_or(&0) as f32;
    let rows = 6u16;
    {
        let buf = f.buffer_mut();
        for (i, v) in data.iter().enumerate() {
            let t = if max > min { (*v as f32 - min) / (max - min) } else { 0.5 };
            let halves = (t * rows as f32 * 2.0).round() as u16;
            let di = i * drought.len().max(1) / cols;
            let dry = drought.get(di).copied().unwrap_or(false);
            for r in 0..rows {
                let y = right.y + row + rows - 1 - r;
                let level = halves.saturating_sub(r * 2);
                let ch = if level >= 2 {
                    glyphs::FULL_BLOCK
                } else if level == 1 {
                    glyphs::HALF_LOWER
                } else {
                    glyphs::SHADE_1
                };
                let color = if level >= 1 { id.color() } else { theme::dim(theme::DIM, 0.6) };
                let bg = if dry { theme::dim(theme::WARN, 0.78) } else { theme::PANEL_BG };
                buf.set_stringn(right.x + 6 + i as u16, y, ch.to_string(), 1, Style::default().fg(color).bg(bg));
            }
        }
        buf.set_stringn(right.x, right.y + row, format!("{:>4} ", max as u32), 5, theme::dim_text());
        buf.set_stringn(right.x, right.y + row + rows - 1, format!("{:>4} ", min as u32), 5, theme::dim_text());
    }
    row += rows;
    let span_days = series.len().max(1);
    util::line(f, right, row, Line::from(vec![
        sp(format!("      D-{:<3}", span_days), theme::dim_text()),
        sp(format!("{:>34}", if drought.iter().any(|&d| d) { "drought bands shaded" } else { "" }), Style::default().fg(theme::WARN).bg(theme::PANEL_BG)),
        sp(format!("{:>49}", "today"), theme::dim_text()),
    ]));
    row += 2;
    util::line(f, right, row, Line::from(sp(" 30-day trend ", theme::dim_text())));
    if !s.trend.is_empty() {
        bars::sparkline(f.buffer_mut(), right.x + 14, right.y + row, 30, &s.trend, id.color());
    }
    let a = trend_arrow(&s.trend);
    f.buffer_mut().set_stringn(
        right.x + 45,
        right.y + row,
        format!(" {} {}", a, if a == glyphs::UP { "growing" } else if a == glyphs::DOWN { "declining" } else { "stable" }),
        14,
        Style::default().fg(arrow_color(a)).bg(theme::PANEL_BG),
    );
    row += 2;
    let first = series.first().copied().unwrap_or(0.0);
    let last = s.count as f32;
    let lo = series.iter().cloned().fold(f32::MAX, f32::min);
    let hi = series.iter().cloned().fold(0.0f32, f32::max);
    let stats: Vec<(String, String, Style)> = vec![
        (format!("{} days ago", span_days), format!("{:.0}", first), theme::text()),
        ("today".into(), format!("{}", s.count), theme::text()),
        ("change".into(), format!("{:+.0} ({:+.0}%)", last - first, (last - first) / first.max(1.0) * 100.0), delta_style(last - first)),
        ("low / high".into(), format!("{:.0} / {:.0}", if lo == f32::MAX { 0.0 } else { lo }, hi), theme::text()),
        ("births today".into(), format!("{}  (yesterday {})", sim.births_today(id.index()), s.births_yesterday), Style::default().fg(theme::GOOD).bg(theme::PANEL_BG)),
        ("deaths today".into(), format!("{}  (yesterday {})", sim.deaths_today(id.index()), s.deaths_yesterday), Style::default().fg(theme::BAD).bg(theme::PANEL_BG)),
        (
            "first birth".into(),
            match s.first_birth_day {
                Some(d) => crate::ui::screens::common::day_stamp(d as i64, sim.time.season_days),
                None => "none yet".into(),
            },
            theme::text(),
        ),
    ];
    for (k, v, st) in stats {
        util::line(f, right, row, Line::from(vec![sp(format!(" {:<14}", k), theme::dim_text()), sp(v, st)]));
        row += 1;
    }
    row += 1;
    let (text, st) = narrative(s);
    util::line(f, right, row, Line::from(vec![sp(format!(" {} ", glyphs::NOTE), theme::label()), sp(text, st)]));
    row += 2;
    panel::section(f, right, row, "Habitat (living individuals by region)");
    row += 1;
    let mut per_region: Vec<(&str, usize)> = sim.world.regions.iter().map(|r| (r.0.as_str(), 0usize)).collect();
    for c in sim.creatures.living().filter(|c| c.species == id) {
        let ri = sim.world.region_index(c.x, c.y);
        if let Some(e) = per_region.get_mut(ri) {
            e.1 += 1;
        }
    }
    per_region.sort_by(|a, b| b.1.cmp(&a.1));
    let max = per_region.first().map(|e| e.1).unwrap_or(1).max(1) as f32;
    let half = right.width / 2;
    for (i, (name, n)) in per_region.iter().enumerate() {
        let col = (i % 2) as u16;
        let r = row + (i / 2) as u16;
        if r >= right.height {
            break;
        }
        let x = right.x + 1 + col * half;
        let y = right.y + r;
        let buf = f.buffer_mut();
        buf.set_stringn(x, y, format!("{:<17}", name), 17, theme::text());
        bars::bar(buf, x + 17, y, 14, *n as f32 / max, id.color());
        buf.set_stringn(x + 32, y, format!("{:>4}", n), 4, theme::dim_text());
    }
}

// ------------------------------------------------------------------ S04b

pub struct SpeciesDetail {
    pub species: SpeciesId,
}

impl SpeciesDetail {
    pub fn new(species: SpeciesId) -> Self {
        SpeciesDetail { species }
    }
}

impl Screen for SpeciesDetail {
    fn opaque(&self) -> bool {
        true
    }

    fn handle_key(&mut self, key: KeyEvent, _app: &mut AppState) -> Action {
        match key.code {
            KeyCode::Esc => Action::Pop,
            KeyCode::Left | KeyCode::Up => {
                self.species = SpeciesId::ALL[(self.species.index() + 5) % 6];
                Action::None
            }
            KeyCode::Right | KeyCode::Down => {
                self.species = SpeciesId::ALL[(self.species.index() + 1) % 6];
                Action::None
            }
            _ => Action::Unhandled,
        }
    }

    fn render(&self, app: &AppState, f: &mut Frame, area: Rect) {
        let Some(sim) = &app.sim else { return };
        let status_row = area.y + area.height - 1;
        let body_h = area.height - 1;
        let left_w = 80u16;
        histograms(f, Rect::new(area.x, area.y, left_w, body_h), sim, self.species);
        drift(f, Rect::new(area.x + left_w, area.y, area.width - left_w, body_h), sim, self.species);
        status::render(
            f,
            Rect::new(area.x, status_row, area.width, 1),
            &[("←→", "other species"), ("Esc", "back")],
            &format!("{} detail  {}", self.species.name(), sim.time.clock_label()),
        );
    }
}

fn histograms(f: &mut Frame, area: Rect, sim: &Sim, id: SpeciesId) {
    let s = &sim.species[id.index()];
    let inner = panel::draw_with_hint(f, area, &format!("{}: trait distributions", id.name()), "12 buckets, living adults + juveniles", panel::Kind::Outer);
    let col_w = 38u16;
    let block_h = 9u16;
    for t in 0..8 {
        let col = (t / 4) as u16;
        let r = (t % 4) as u16;
        let x = inner.x + 1 + col * (col_w + 1);
        let y = inner.y + 1 + r * block_h;
        let color = trait_color(t);
        let mean = s.mean.0[t];
        let (min, max) = (s.min.0[t], s.max.0[t]);
        let buf = f.buffer_mut();
        buf.set_stringn(x, y, TRAIT_NAMES[t], 12, Style::default().fg(color).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD));
        buf.set_stringn(
            x + 12,
            y,
            format!("min .{:02} mean .{:02} max .{:02}", (min * 100.0).round() as u32 % 100, (mean * 100.0).round() as u32 % 100, (max * 100.0).round() as u32 % 100),
            26,
            theme::dim_text(),
        );
        let hist_area = Rect::new(x, y + 1, 36, 5);
        if s.count > 0 {
            bars::histogram(buf, hist_area, &s.hist[t], color, 3);
        }
        let axis: String = std::iter::repeat_n(glyphs::H_LINE, 36).collect();
        buf.set_stringn(x, y + 6, &axis, 36, theme::border());
        let mx = x + ((mean * 35.0).round() as u16).min(35);
        buf.set_stringn(mx, y + 6, glyphs::CROSS.to_string(), 1, Style::default().fg(theme::TEXT_BRIGHT).bg(theme::PANEL_BG));
        buf.set_stringn(x, y + 7, "0.0", 3, theme::dim_text());
        buf.set_stringn(x + 16, y + 7, "0.5", 3, theme::dim_text());
        buf.set_stringn(x + 33, y + 7, "1.0", 3, theme::dim_text());
        let n: u32 = s.hist[t].iter().map(|&v| v as u32).sum();
        let peak_bucket = s.hist[t].iter().enumerate().max_by_key(|(_, v)| **v).map(|(i, _)| i).unwrap_or(0);
        buf.set_stringn(x + 4, y + 7, format!("n={}", n), 8, theme::dim_text());
        buf.set_stringn(x + 20, y + 7, format!("mode {:.2}", (peak_bucket as f32 + 0.5) / 12.0), 12, theme::dim_text());
    }
    let y = inner.y + 1 + 4 * block_h;
    let buf = f.buffer_mut();
    buf.set_stringn(
        inner.x + 1,
        y,
        format!("{} mean   {} full  {} half bucket   each column is 1/12 of the 0..1 range", glyphs::CROSS, glyphs::FULL_BLOCK, glyphs::HALF_LOWER),
        inner.width as usize - 2,
        theme::dim_text(),
    );
}

/// Selection-pressure lines (FR7): traits whose drift over the last 3 samples
/// exceeds ±0.02.
pub fn selection_pressure(s: &SpeciesStats) -> Vec<String> {
    let n = s.drift.len();
    let mut out = Vec::new();
    if n >= 2 {
        let a = &s.drift[n.saturating_sub(3)];
        let b = &s.drift[n - 1];
        let gens = b.0.saturating_sub(a.0).max(1);
        for t in 0..8 {
            let d = b.1 .0[t] - a.1 .0[t];
            if d.abs() > 0.02 {
                out.push(format!("{} {} {} ({:+.2} over {} generations)", glyphs::MUTATION, TRAIT_NAMES[t], if d > 0.0 { "rising" } else { "falling" }, d, gens));
            }
        }
    }
    if out.is_empty() {
        out.push(format!("{} no trait moving more than 0.02", glyphs::NOTE));
    }
    out
}

fn drift(f: &mut Frame, area: Rect, sim: &Sim, id: SpeciesId) {
    let s = &sim.species[id.index()];
    let inner = panel::draw_with_hint(f, area, "Drift over generations", &format!("{} sampled generations", s.drift.len()), panel::Kind::Outer);
    let mut row = 0u16;
    let n = s.drift.len();
    let first_gen = s.drift.first().map(|d| d.0).unwrap_or(1);
    {
        let y = inner.y + row;
        let buf = f.buffer_mut();
        buf.set_stringn(inner.x, y, format!(" trait       g{:<4}", first_gen), 18, theme::dim_text());
        buf.set_stringn(inner.x + 21, y, "oldest", 6, theme::dim_text());
        buf.set_stringn(inner.x + 51, y, "newest", 6, theme::dim_text());
        buf.set_stringn(inner.x + 58, y, format!(" g{:<3} change", s.generation), 12, theme::dim_text());
    }
    row += 1;
    for t in 0..8 {
        let color = trait_color(t);
        let y = inner.y + row;
        let buf = f.buffer_mut();
        buf.set_stringn(inner.x, y, format!(" {:<12}", TRAIT_NAMES[t]), 13, Style::default().fg(color).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD));
        if n > 0 {
            let first = s.drift[0].1 .0[t];
            let last = s.drift[n - 1].1 .0[t];
            let vals: Vec<u16> = s.drift.iter().map(|g| (g.1 .0[t] * 100.0).round() as u16).collect();
            buf.set_stringn(inner.x + 13, y, format!("{:.2}", first), 4, theme::dim_text());
            let wide: Vec<u16> = vals.iter().flat_map(|v| [*v, *v, *v]).collect();
            bars::sparkline(buf, inner.x + 21, y, 36, &wide, color);
            buf.set_stringn(inner.x + 59, y, format!("{:.2}", last), 4, theme::text());
            buf.set_stringn(inner.x + 66, y, format!("{:+.2}", last - first), 5, delta_style(last - first));
        } else {
            buf.set_stringn(inner.x + 13, y, "no samples yet", 14, theme::dim_text());
        }
        row += 1;
    }
    row += 1;
    panel::section(f, inner, row, "Per-generation means (x100)");
    row += 1;
    let mut hdr = String::from("              ");
    for g in &s.drift {
        hdr.push_str(&format!("g{:<3}", g.0));
    }
    util::line(f, inner, row, Line::from(sp(hdr, theme::dim_text())));
    row += 1;
    for t in 0..8 {
        let mut spans = vec![sp(format!(" {:<12} ", TRAIT_NAMES[t]), theme::text())];
        for g in 0..n {
            let v = s.drift[g].1 .0[t];
            let prev = if g == 0 { v } else { s.drift[g - 1].1 .0[t] };
            let st = if g == 0 { theme::dim_text() } else { delta_style(v - prev) };
            spans.push(sp(format!("{:<4}", two(v)), st));
        }
        util::line(f, inner, row, Line::from(spans));
        row += 1;
    }
    util::line(f, inner, row, Line::from(sp(" green = rose vs previous sample, red = fell", theme::dim_text())));
    row += 2;

    panel::section(f, inner, row, "Population");
    row += 1;
    let arrow = trend_arrow(&s.trend);
    let stats: Vec<(String, String, Style)> = vec![
        ("count".into(), format!("{}  ({} adults, {} juveniles)", s.count, s.adults, s.juveniles), theme::text()),
        ("generation".into(), format!("{}", s.generation), theme::text()),
        ("peak".into(), format!("{}  ({}% of peak now)", s.peak, s.count * 100 / s.peak.max(1)), theme::text()),
        ("births today".into(), format!("{}", sim.births_today(id.index())), Style::default().fg(theme::GOOD).bg(theme::PANEL_BG)),
        ("deaths today".into(), format!("{}", sim.deaths_today(id.index())), Style::default().fg(theme::BAD).bg(theme::PANEL_BG)),
        ("trend".into(), format!("{} over 30 days", arrow), Style::default().fg(arrow_color(arrow)).bg(theme::PANEL_BG)),
    ];
    for (k, v, st) in stats {
        util::line(f, inner, row, Line::from(vec![sp(format!(" {:<14}", k), theme::dim_text()), sp(v, st)]));
        row += 1;
    }
    row += 1;
    panel::section(f, inner, row, "Selection pressure");
    row += 1;
    for note in selection_pressure(s) {
        util::line(f, inner, row, Line::from(sp(format!(" {}", note), theme::dim_text())));
        row += 1;
    }
    row += 1;
    panel::section(f, inner, row, "Compared with other species (mean x100)");
    row += 1;
    util::line(f, inner, row, Line::from(sp("              Spd Siz Sen Met Agg Cam Fer Lon   count  gen", theme::dim_text())));
    row += 1;
    for other in &sim.species {
        if row >= inner.height {
            break;
        }
        let absent = other.count == 0;
        let mut spans = vec![
            sp(format!(" {} ", other.species.glyph().to_ascii_uppercase()), Style::default().fg(if absent { theme::DIM } else { other.species.color() }).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            sp(format!("{:<10}", other.species.name()), if other.species == id { theme::title() } else if absent { theme::dim_text() } else { theme::text() }),
        ];
        for t in 0..8 {
            spans.push(sp(format!(" {:>3}", two(other.mean.0[t])), Style::default().fg(if absent { theme::DIM } else { trait_color(t) }).bg(theme::PANEL_BG)));
        }
        spans.push(sp(format!("   {:>5}  {:>3}", other.count, other.generation), theme::dim_text()));
        util::line(f, inner, row, Line::from(spans));
        row += 1;
    }
}
