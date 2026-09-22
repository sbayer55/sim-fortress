//! S07: the live event log. S07a lists every event; S07b (any narrow filter)
//! shows a detail panel with a mini-map and subject link; S07c (`c`) is the
//! chronicle, one paragraph per season (C9).

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget, Wrap};
use ratatui::Frame;

use crate::sim::{Event, EventKind, Outbreak, PathogenId, Sim, SpeciesId};
use crate::ui::screens::common::day_stamp;
use crate::ui::app::AppState;
use crate::ui::screens::s03_inspector::Inspector;
use crate::ui::screens::{Action, Screen};
use crate::ui::style::{EventKindStyle, SpeciesStyle};
use crate::widgets::map::{self, MapOptions, OverlayStack};
use crate::widgets::Constraint::{Fill, Fixed};
use crate::widgets::{panel, util, Column, Component, FilterStrip, StatusBar, Table, TableCell, TableRow, Text};
use crate::{glyphs, theme};

mod chronicle;

const DETAIL_W: u16 = 55;
const MINI_W: u16 = 25;
const MINI_H: u16 = 9;

/// The S07 filter-chip state. `all` is active, or a subset of the eight kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChipFilter {
    pub all: bool,
    /// births, deaths, mutations, migrations, extinctions, droughts, disease, wary.
    pub kinds: [bool; KIND_CHIPS],
}

/// Number of kind chips (everything but `all`).
pub const KIND_CHIPS: usize = 8;

impl Default for ChipFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl ChipFilter {
    pub const fn new() -> Self {
        Self { all: true, kinds: [false; KIND_CHIPS] }
    }

    /// Toggle chip `key` (1 = all, 2..=9 = births..wary).
    pub fn toggle(&mut self, key: usize) {
        if key == 1 {
            self.all = true;
            self.kinds = [false; KIND_CHIPS];
            return;
        }
        let i = key - 2;
        if i >= KIND_CHIPS {
            return;
        }
        if self.all {
            self.all = false;
            self.kinds = [false; KIND_CHIPS];
            self.kinds[i] = true;
        } else {
            self.kinds[i] = !self.kinds[i];
            if !self.kinds.iter().any(|&k| k) {
                self.all = true;
            }
        }
    }

    /// Cycle presets: all → deaths+extinctions → migrations+droughts → disease → all.
    pub const fn cycle(&mut self) {
        if self.all {
            self.all = false;
            self.kinds = [false; KIND_CHIPS];
            self.kinds[1] = true;
            self.kinds[4] = true;
        } else if self.kinds[1] && self.kinds[4] {
            self.kinds = [false; KIND_CHIPS];
            self.kinds[3] = true;
            self.kinds[5] = true;
        } else if self.kinds[3] && self.kinds[5] {
            self.kinds = [false; KIND_CHIPS];
            self.kinds[6] = true;
        } else {
            self.all = true;
            self.kinds = [false; KIND_CHIPS];
        }
    }

    pub const fn matches(self, kind: EventKind) -> bool {
        if self.all {
            return true;
        }
        match kind {
            EventKind::Birth => self.kinds[0],
            EventKind::DeathStarved | EventKind::DeathThirst | EventKind::DeathPredation | EventKind::DeathAge | EventKind::DeathDisease => self.kinds[1],
            EventKind::Mutation => self.kinds[2],
            EventKind::Migration => self.kinds[3],
            EventKind::Extinction => self.kinds[4],
            EventKind::Drought | EventKind::DroughtEased => self.kinds[5],
            EventKind::Outbreak | EventKind::Spillover | EventKind::Epidemic | EventKind::EpidemicOver | EventKind::Recovery => self.kinds[6],
            // C5 FR13: contests share the avoidance chip with the wary tier.
            EventKind::Wary | EventKind::Contest => self.kinds[7],
            EventKind::Season | EventKind::Note => false,
        }
    }
}

#[derive(Debug)]
pub struct EventLog {
    filter: ChipFilter,
    selected: usize,
    /// S07c: showing the chronicle instead of the list.
    chronicle: bool,
    chron_scroll: u16,
    /// What the chronicle blit measured last render, for clamping the scroll.
    chron_measured: std::cell::Cell<(u16, u16)>,
}

impl Default for EventLog {
    fn default() -> Self {
        Self::new()
    }
}

impl EventLog {
    pub const fn new() -> Self {
        Self { filter: ChipFilter::new(), selected: 0, chronicle: false, chron_scroll: 0, chron_measured: std::cell::Cell::new((0, 0)) }
    }

    fn filtered<'a>(&self, sim: &'a Sim) -> Vec<&'a Event> {
        sim.events.iter().rev().filter(|e| self.filter.matches(e.kind)).collect()
    }
}

impl Screen for EventLog {
    fn opaque(&self) -> bool {
        true
    }

    fn handle_key(&mut self, key: KeyEvent, app: &mut AppState) -> Action {
        if key.code == KeyCode::Char('c') {
            self.chronicle = !self.chronicle;
            return Action::None;
        }
        if self.chronicle {
            let (content, visible) = self.chron_measured.get();
            let max = crate::widgets::scroll::Overflow::max_offset(content, visible);
            return match key.code {
                KeyCode::Up => {
                    self.chron_scroll = self.chron_scroll.saturating_sub(1);
                    Action::None
                }
                KeyCode::Down => {
                    self.chron_scroll = (self.chron_scroll + 1).min(max);
                    Action::None
                }
                KeyCode::Esc => Action::Pop,
                _ => Action::Unhandled,
            };
        }
        match key.code {
            KeyCode::Char('1') => {
                self.filter.toggle(1);
                self.selected = 0;
                Action::None
            }
            KeyCode::Char(c @ '2'..='8') => {
                self.filter.toggle(crate::cast!(c.to_digit(10).unwrap_or(0) => usize));
                self.selected = 0;
                Action::None
            }
            KeyCode::Char('f') => {
                self.filter.cycle();
                self.selected = 0;
                Action::None
            }
            KeyCode::Up => {
                self.selected = self.selected.saturating_sub(1);
                Action::None
            }
            KeyCode::Down => {
                self.selected += 1;
                Action::None
            }
            KeyCode::Enter => {
                if let Some(sim) = &app.sim {
                    let events = self.filtered(sim);
                    if let Some(pos) = events.get(self.selected).and_then(|e| e.pos) {
                        app.centre_viewport_on(pos.0, pos.1);
                        app.follow = None;
                        app.enter_look(pos);
                        return Action::Pop;
                    }
                }
                Action::None
            }
            KeyCode::Char('i') => {
                let Some(sim) = &app.sim else {
                    return Action::None;
                };
                let events = self.filtered(sim);
                let subject = events.get(self.selected).and_then(|e| e.subject);
                if let Some(subject) = subject.filter(|s| sim.creatures.get(*s).is_some()) {
                    return Action::Push(Box::new(Inspector::new(subject)));
                }
                Action::None
            }
            KeyCode::Esc => Action::Pop,
            _ => Action::Unhandled,
        }
    }

    fn render(&self, app: &AppState, f: &mut Frame<'_>, area: Rect) {
        let Some(sim) = &app.sim else {
            return;
        };
        if self.chronicle {
            let m = chronicle::render(f, area, app, sim, self.chron_scroll);
            self.chron_measured.set((m.content, area.height.saturating_sub(3)));
            return;
        }
        let events = self.filtered(sim);
        let detail = !self.filter.all;
        let status_row = area.y + area.height - 1;
        let body_h = area.height - 1;

        let list_w = if detail { area.width - DETAIL_W } else { area.width };
        let list_area = Rect::new(area.x, area.y, list_w, body_h);
        let title = if detail { "Event Log — filtered" } else { "Event Log" };
        let inner = panel::draw_with_hint(f, list_area, title, &format!("{} events", events.len()), panel::Kind::Outer);
        self.chips(f, inner);
        self.list(f, Rect::new(inner.x, inner.y + 1, inner.width, inner.height - 1), sim, &events);

        if detail {
            let detail_area = Rect::new(area.x + list_w, area.y, DETAIL_W, body_h);
            let selected = events.get(self.selected).copied();
            Self::detail(f, detail_area, app, sim, selected);
        }

        let right = format!("{}  {} {}", sim.time.clock_label(), glyphs::SUN, "day");
        let keys: &[(&str, &str)] = if detail {
            &[("↑↓", "select"), ("f", "filter"), ("Enter", "jump"), ("i", "inspect"), ("c", "chronicle"), ("Esc", "back")]
        } else {
            &[("↑↓", "select"), ("1-9", "chips"), ("f", "cycle"), ("Enter", "jump"), ("c", "chronicle"), ("Esc", "back")]
        };
        StatusBar::new(keys).right(&right).note(app.ai.status_note()).render(f.buffer_mut(), Rect::new(area.x, status_row, area.width, 1));
    }
}

const CHIPS: [(char, &str); KIND_CHIPS + 1] = [
    ('*', "all"),
    (glyphs::BIRTH, "births"),
    (glyphs::DEATH, "deaths"),
    (glyphs::MUTATION, "mutations"),
    (glyphs::MIGRATION, "migrations"),
    (glyphs::EXTINCTION, "extinctions"),
    (glyphs::DROUGHT, "droughts"),
    (glyphs::DISEASE, "disease"),
    (glyphs::ALERT, "wary"),
];

/// The event columns after the Marker: when, kind, species glyph, text.
const EVENT_COLUMNS: [Column; 4] = [Column::titled("when", Fixed(16)), Column::titled("kind", Fixed(15)), Column::titled("sp", Fixed(3)), Column::titled("event", Fill(1))];

impl EventLog {
    fn chips(&self, f: &mut Frame<'_>, inner: Rect) {
        let kinds = [EventKind::Note, EventKind::Birth, EventKind::DeathPredation, EventKind::Mutation, EventKind::Migration, EventKind::Extinction, EventKind::Drought, EventKind::Outbreak, EventKind::Wary];
        let mut colors = kinds.map(|k| k.color());
        colors[0] = theme::KEY;
        let mut active = [false; KIND_CHIPS + 1];
        active[0] = self.filter.all;
        active[1..].copy_from_slice(&self.filter.kinds);
        FilterStrip::new(&CHIPS).active(&active).colors(&colors).hint("[f] cycles, [1-9] toggles").render(f.buffer_mut(), Rect { height: 1, ..inner });
    }

    fn list(&self, f: &mut Frame<'_>, area: Rect, sim: &Sim, events: &[&Event]) {
        let buf = f.buffer_mut();
        if events.is_empty() {
            Text::new(" no events").style(theme::dim_text()).render(buf, Rect::new(area.x, area.y + 1, area.width, 1).intersection(area));
            return;
        }
        let text_max = usize::from(area.width).saturating_sub(35);
        let rows: Vec<TableRow<'_>> = events
            .iter()
            .take(usize::from(area.height))
            .map(|e| {
                let sp_glyph = e.species.map_or_else(|| "-".to_string(), |s| sim.roster().adult_glyph(s).to_string());
                let text = if e.text.chars().count() > text_max {
                    let mut t: String = e.text.chars().take(text_max.saturating_sub(1)).collect();
                    t.push('~');
                    t
                } else {
                    e.text.clone()
                };
                TableRow::new([
                    TableCell::dim(format!("Y{} D{:03} {:02}:00", e.year, e.day, e.hour)),
                    TableCell::styled(format!("{} {:<10}", e.kind.glyph(), e.kind.label()), e.kind.color()),
                    TableCell::dim(sp_glyph),
                    TableCell::text(text),
                ])
            })
            .collect();
        Table::new(&EVENT_COLUMNS, &rows).selected(Some(self.selected)).render(buf, area);
    }

    fn detail(f: &mut Frame<'_>, area: Rect, app: &AppState, sim: &Sim, e: Option<&Event>) {
        let inner = panel::draw(f, area, "Event detail", panel::Kind::Focus);
        let bg = theme::PANEL_BG;
        let mut row = 0u16;
        let Some(e) = e else {
            util::line(f, inner, 0, Line::from(Span::styled(" nothing selected", theme::dim_text())));
            return;
        };
        util::line(f, inner, row, Line::from(vec![
            Span::styled(format!(" {} ", e.kind.glyph()), Style::default().fg(e.kind.color()).bg(bg).add_modifier(Modifier::BOLD)),
            Span::styled(e.kind.label(), Style::default().fg(e.kind.color()).bg(bg).add_modifier(Modifier::BOLD)),
            Span::styled(format!("   Year {}, Day {}, {:02}:00", e.year, e.day, e.hour), theme::dim_text()),
        ]));
        row += 1;
        Paragraph::new(Line::from(Span::styled(e.text.clone(), Style::default().fg(theme::TEXT_BRIGHT).bg(bg).add_modifier(Modifier::BOLD))))
            .wrap(Wrap { trim: true })
            .render(Rect::new(inner.x + 1, inner.y + row, inner.width - 2, 3), f.buffer_mut());
        row += 4;

        // Outbreak events show the outbreak record instead of the creature vitals.
        let is_outbreak_event = matches!(e.kind, EventKind::Outbreak | EventKind::Epidemic | EventKind::EpidemicOver | EventKind::Spillover);
        if is_outbreak_event {
            row = Self::outbreak_record(f, inner, row, sim, e);
        }

        // Subject creature.
        if let Some(subject) = e.subject.filter(|_| !is_outbreak_event) {
            if let Some(c) = sim.creatures.get(subject) {
                let state = if c.alive { "alive".to_string() } else { format!("dead: {}", c.death.map_or("?", |d| d.cause.label())) };
                util::line(f, inner, row, Line::from(vec![
                    Span::styled(format!(" {} ", if c.alive { sim.roster().adult_glyph(c.species) } else { glyphs::CARCASS }), Style::default().fg(sim.roster().color(c.species)).bg(bg).add_modifier(Modifier::BOLD)),
                    Span::styled(format!("{} {}  ", c.name_str(sim.roster()), c.tag(sim.roster())), theme::text()),
                    Span::styled(state, theme::dim_text()),
                ]));
                row += 1;
                util::line(f, inner, row, Line::from(vec![
                    Span::styled(" [i]", theme::key()),
                    Span::styled(" inspect creature", theme::dim_text()),
                ]));
                row += 1;
            }
        }
        row += 1;

        // Mini-map.
        if let Some((x, y)) = e.pos {
            let ox = crate::cast!((crate::cast!(x => i64) - i64::from(MINI_W).div_euclid(2)).clamp(0, crate::cast!(sim.world.width() => i64) - i64::from(MINI_W)) => usize);
            let oy = crate::cast!((crate::cast!(y => i64) - i64::from(MINI_H).div_euclid(2)).clamp(0, crate::cast!(sim.world.height() => i64) - i64::from(MINI_H)) => usize);
            let mini = Rect::new(inner.x + 1, inner.y + row, MINI_W + 2, MINI_H + 2);
            let mini_inner = panel::draw(f, mini, "", panel::Kind::Inner);
            let opts = MapOptions {
                stack: OverlayStack::default(),
                night: false,
                winter: false,
                cursor: Some((x, y)),
                follow: None,
                pins: Vec::new(),
                origin: (ox, oy),
                creatures: true,
                selected_region: None,
                species_color: theme::TEXT,
                creature_tint: None,
            };
            map::render(f.buffer_mut(), mini_inner, sim, &opts);
        }
        let _ = row;
        let _ = app;
    }

    /// The outbreak record for an Outbreak / Epidemic / `EpidemicOver` / Spillover
    /// event: the pathogen named in the event text, and its latest outbreak
    /// started on or before the event day. Returns the next free row.
    fn outbreak_record(f: &mut Frame<'_>, inner: Rect, mut row: u16, sim: &Sim, e: &Event) -> u16 {
        let bg = theme::PANEL_BG;
        let Some((pid, o)) = find_outbreak(sim, e) else {
            util::line(f, inner, row, Line::from(Span::styled(" no outbreak record for this event", theme::dim_text())));
            return row + 1;
        };
        let strain = sim.disease.pathogen(pid).is_some_and(crate::sim::disease::Pathogen::is_strain);
        let color = if strain { theme::MAGENTA } else { theme::SICK };
        let region = sim.world.regions.get(crate::cast!(o.origin_region => usize)).map_or("?", |r| r.0.as_str());
        let state = match o.ended_day {
            None => "ongoing".to_string(),
            Some(d) => format!("over after {} days", d.saturating_sub(o.started_day)),
        };
        util::line(f, inner, row, Line::from(vec![
            Span::styled(format!(" {} ", glyphs::DISEASE), Style::default().fg(color).bg(bg).add_modifier(Modifier::BOLD)),
            Span::styled(sim.disease.name(pid).to_string(), Style::default().fg(color).bg(bg).add_modifier(Modifier::BOLD)),
            Span::styled(if o.epidemic { "  epidemic  " } else { "  outbreak  " }, theme::dim_text()),
            Span::styled(state, theme::dim_text()),
        ]));
        row += 1;
        util::line(f, inner, row, Line::from(vec![
            Span::styled(" began ", theme::dim_text()),
            Span::styled(day_stamp(i64::from(o.started_day), sim.time.season_days), theme::text()),
            Span::styled(" in ", theme::dim_text()),
            Span::styled(region.to_string(), theme::text()),
        ]));
        row += 1;
        util::line(f, inner, row, Line::from(vec![
            Span::styled(" cases ", theme::dim_text()),
            Span::styled(o.cases.to_string(), Style::default().fg(theme::SICK).bg(bg)),
            Span::styled(" / dead ", theme::dim_text()),
            Span::styled(o.deaths.to_string(), Style::default().fg(theme::BAD).bg(bg)),
            Span::styled(" / recovered ", theme::dim_text()),
            Span::styled(o.recovered.to_string(), Style::default().fg(theme::GOOD).bg(bg)),
            Span::styled(" / peak ", theme::dim_text()),
            Span::styled(o.peak_active.to_string(), theme::text()),
        ]));
        row += 1;
        if let Some((i, _)) = o.species_cases.iter().enumerate().filter(|(_, n)| **n > 0).max_by_key(|(i, n)| (**n, std::cmp::Reverse(*i))) {
            let id = SpeciesId::from_index(i);
            let end = if o.ended_day.is_some() { o.resist_at_end[i] } else { sim.series.last().map_or(o.resist_at_start[i], |s| s.genome_mean[i].resistance()) };
            util::line(f, inner, row, Line::from(vec![
                Span::styled(" resist ", theme::dim_text()),
                Span::styled(format!("{} ", sim.roster().adult_glyph(id)), Style::default().fg(sim.roster().color(id)).bg(bg).add_modifier(Modifier::BOLD)),
                Span::styled(format!("{} {} {}", short2(o.resist_at_start[i]), glyphs::RIGHT, short2(end)), Style::default().fg(theme::SICK).bg(bg)),
                Span::styled(if o.ended_day.is_none() { "  (so far)" } else { "" }, theme::dim_text()),
            ]));
            row += 1;
        }
        if let Some(parent) = sim.disease.pathogen(pid).and_then(|p| p.parent) {
            util::line(f, inner, row, Line::from(vec![
                Span::styled(" strain of ", theme::dim_text()),
                Span::styled(sim.disease.name(parent).to_string(), Style::default().fg(theme::MAGENTA).bg(bg)),
            ]));
            row += 1;
        }
        row
    }
}

/// `.31` for 0.31 (two decimals, no leading zero).
fn short2(v: f32) -> String {
    format!("{v:.2}").replace("0.", ".")
}

/// The pathogen named in an event's text (longest matching name wins, so a
/// strain named after its parent is preferred over the parent) and its latest
/// outbreak started on or before the event day.
fn find_outbreak<'a>(sim: &'a Sim, e: &Event) -> Option<(PathogenId, &'a Outbreak)> {
    let year_len = 4 * sim.time.season_days;
    let event_day = e.year.saturating_sub(1) * year_len + e.day.saturating_sub(1);
    let pid = sim
        .disease
        .pathogens
        .iter()
        .enumerate()
        .filter(|(_, p)| !p.name().is_empty() && e.text.contains(p.name()))
        .max_by_key(|(i, p)| (p.name().len(), std::cmp::Reverse(*i)))
        .map(|(i, _)| PathogenId(crate::cast!(i => u8)))?;
    let o = sim.disease.outbreaks.iter().filter(|o| o.pathogen == pid && o.started_day <= event_day).max_by_key(|o| o.started_day)?;
    Some((pid, o))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::{CreatureId, Params};
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::KeyModifiers;
    use ratatui::Terminal;

    fn screen_text(app: &AppState, screen: &EventLog) -> String {
        let backend = TestBackend::new(155, 45);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| screen.render(app, f, Rect::new(0, 0, 155, 45))).unwrap();
        let buf = terminal.backend().buffer();
        (0..45).map(|y| (0..155).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>() + "\n").collect()
    }

    #[test]
    fn s07_disease_chip() {
        let mut app = AppState::new(Params::default());
        let mut sim = Sim::new(7, Params::default());
        let name = sim.disease.name(PathogenId(0)).to_string();
        assert!(name != "?");
        sim.disease.first_index = 0;
        sim.disease.outbreaks.push(Outbreak {
            pathogen: PathogenId(0),
            started_day: 0,
            ended_day: None,
            origin_region: 2,
            index_case: CreatureId(0),
            cases: 9,
            deaths: 2,
            recovered: 3,
            peak_active: 4,
            peak_day: 0,
            species_cases: vec![0, 9, 0, 0, 0, 0],
            species_deaths: vec![0, 2, 0, 0, 0, 0],
            epidemic: false,
            resist_at_start: vec![0.31; 6],
            resist_at_end: vec![0.0; 6],
            active: 4,
            cases_today: 1,
        });
        let region = sim.world.regions[2].0.clone();
        let mk = |kind: EventKind, text: String| Event { year: 1, day: 1, hour: 6, kind, species: None, subject: None, text, pos: None, detail: String::new() };
        sim.events.push(mk(EventKind::Birth, "a birth".into()));
        sim.events.push(mk(EventKind::DeathDisease, "died of disease".into()));
        sim.events.push(mk(EventKind::Outbreak, format!("{name} breaks out among the hares of {region}")));
        app.sim = Some(sim);

        let mut s = EventLog::new();
        let all = screen_text(&app, &s);
        assert!(all.contains(&format!("{} disease", glyphs::DISEASE)), "eighth chip missing: {all}");
        assert!(all.contains(&format!("{} wary", glyphs::ALERT)), "ninth chip missing: {all}");
        assert!(all.contains("[f] cycles, [1-9] toggles"), "{all}");

        // Key 8 keeps only the disease kinds; disease deaths stay under deaths.
        s.handle_key(KeyEvent::new(KeyCode::Char('8'), KeyModifiers::NONE), &mut app);
        assert_eq!(s.filter.kinds, [false, false, false, false, false, false, true, false]);
        let text = screen_text(&app, &s);
        assert!(text.contains("breaks out"), "{text}");
        assert!(!text.contains("died of disease"), "{text}");
        assert!(!text.contains("a birth"), "{text}");
        // The detail panel shows the outbreak record.
        assert!(text.contains(&format!("began Y1 D001 in {region}")), "{text}");
        assert!(text.contains("cases 9 / dead 2 / recovered 3 / peak 4"), "{text}");
        assert!(text.contains(&format!("resist H .31 {} ", glyphs::RIGHT)), "{text}");

        // f cycles through the disease stop.
        let mut f = ChipFilter::new();
        f.cycle();
        f.cycle();
        f.cycle();
        assert!(f.kinds[6] && !f.all);
        f.cycle();
        assert!(f.all);
    }
}
