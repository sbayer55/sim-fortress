//! S07: the live event log. S07a lists every event; S07b (any narrow filter)
//! shows a detail panel with a mini-map and subject link.

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
use crate::widgets::map::{self, MapOptions, Overlay};
use crate::widgets::{panel, status, util};
use crate::{glyphs, theme};

const DETAIL_W: u16 = 55;
const MINI_W: u16 = 25;
const MINI_H: u16 = 9;

/// The S07 filter-chip state. `all` is active, or a subset of the seven kinds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChipFilter {
    pub all: bool,
    /// births, deaths, mutations, migrations, extinctions, droughts, disease.
    pub kinds: [bool; 7],
}

/// Number of kind chips (everything but `all`).
pub const KIND_CHIPS: usize = 7;

impl Default for ChipFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl ChipFilter {
    pub fn new() -> Self {
        ChipFilter { all: true, kinds: [false; KIND_CHIPS] }
    }

    /// Toggle chip `key` (1 = all, 2..=8 = births..disease).
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
    pub fn cycle(&mut self) {
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

    pub fn matches(&self, kind: EventKind) -> bool {
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
            EventKind::Season | EventKind::Note => false,
        }
    }
}

pub struct EventLog {
    filter: ChipFilter,
    selected: usize,
}

impl Default for EventLog {
    fn default() -> Self {
        Self::new()
    }
}

impl EventLog {
    pub fn new() -> Self {
        EventLog { filter: ChipFilter::new(), selected: 0 }
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
        match key.code {
            KeyCode::Char('1') => {
                self.filter.toggle(1);
                self.selected = 0;
                Action::None
            }
            KeyCode::Char(c @ '2'..='8') => {
                self.filter.toggle(c.to_digit(10).unwrap() as usize);
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
                        app.look_cursor = Some(pos);
                        app.follow = None;
                        return Action::Pop;
                    }
                }
                Action::None
            }
            KeyCode::Char('i') => {
                if let Some(sim) = &app.sim {
                    let events = self.filtered(sim);
                    if let Some(subject) = events.get(self.selected).and_then(|e| e.subject) {
                        if sim.creatures.get(subject).is_some() {
                            return Action::Push(Box::new(Inspector::new(subject)));
                        }
                    }
                }
                Action::None
            }
            KeyCode::Esc => Action::Pop,
            _ => Action::Unhandled,
        }
    }

    fn render(&self, app: &AppState, f: &mut Frame, area: Rect) {
        let Some(sim) = &app.sim else {
            return;
        };
        let events = self.filtered(sim);
        let detail = !self.filter.all;
        let status_row = area.y + area.height - 1;
        let body_h = area.height - 1;

        let list_w = if detail { area.width - DETAIL_W } else { area.width };
        let list_area = Rect::new(area.x, area.y, list_w, body_h);
        let title = if detail { "Event Log — filtered" } else { "Event Log" };
        let inner = panel::draw_with_hint(f, list_area, title, &format!("{} events", events.len()), panel::Kind::Outer);
        self.chips(f, inner);
        self.list(f, Rect::new(inner.x, inner.y + 2, inner.width, inner.height - 2), sim, &events, detail);

        if detail {
            let detail_area = Rect::new(area.x + list_w, area.y, DETAIL_W, body_h);
            let selected = events.get(self.selected).copied();
            self.detail(f, detail_area, app, sim, selected);
        }

        let right = format!("{}  {} {}", sim.time.clock_label(), glyphs::SUN, "day");
        let keys: &[(&str, &str)] = if detail {
            &[("↑↓", "select"), ("f", "filter"), ("Enter", "jump"), ("i", "inspect"), ("Esc", "back")]
        } else {
            &[("↑↓", "select"), ("1-8", "chips"), ("f", "cycle"), ("Enter", "jump"), ("Esc", "back")]
        };
        status::render(f, Rect::new(area.x, status_row, area.width, 1), keys, &right);
    }
}

impl EventLog {
    fn chips(&self, f: &mut Frame, inner: Rect) {
        let chips: [(char, &str, EventKind); KIND_CHIPS + 1] = [
            ('*', " all", EventKind::Note),
            ('♥', " births", EventKind::Birth),
            ('x', " deaths", EventKind::DeathPredation),
            ('§', " mutations", EventKind::Mutation),
            ('→', " migrations", EventKind::Migration),
            ('‼', " extinctions", EventKind::Extinction),
            ('¡', " droughts", EventKind::Drought),
            (glyphs::DISEASE, " disease", EventKind::Outbreak),
        ];
        let mut x = inner.x + 1;
        for (i, (glyph, name, kind)) in chips.iter().enumerate() {
            let active = if i == 0 { self.filter.all } else { self.filter.kinds[i - 1] };
            let buf = f.buffer_mut();
            if let Some(c) = buf.cell_mut((x, inner.y)) {
                c.set_char(*glyph);
                c.set_style(if i == 0 { theme::key() } else { Style::default().fg(kind.color()).bg(theme::PANEL_BG) });
            }
            buf.set_stringn(x + 1, inner.y, name, name.chars().count(), if active { theme::selected() } else { theme::text() });
            x += 1 + name.chars().count() as u16 + 1;
        }
        let hint = "[f] cycles, [1-8] toggles";
        let hint_w = hint.len() as u16;
        if x + 2 + hint_w <= inner.right() {
            f.buffer_mut().set_stringn(inner.right() - hint_w - 1, inner.y, hint, hint.len(), theme::dim_text());
        }
        util::line(f, inner, 1, Line::from(Span::styled(" when         kind        sp  event", theme::dim_text())));
    }

    fn list(&self, f: &mut Frame, area: Rect, sim: &Sim, events: &[&Event], compact: bool) {
        let text_w = area.width.saturating_sub(1) as usize;
        for (i, e) in events.iter().take(area.height as usize).enumerate() {
            let selected = i == self.selected;
            let y = area.y + i as u16;
            let buf = f.buffer_mut();
            let bg = if selected { theme::SELECT_BG } else { theme::PANEL_BG };
            for cx in area.x..area.right().min(area.x + text_w as u16) {
                if let Some(c) = buf.cell_mut((cx, y)) {
                    c.set_bg(bg);
                }
            }
            let bright = Style::default().fg(theme::TEXT_BRIGHT).bg(theme::SELECT_BG);
            let when_style = if selected { bright } else { theme::dim_text() };
            buf.set_stringn(area.x + 1, y, format!("{}Y{} D{:03} {:02}:00", if selected { "►" } else { " " }, e.year, e.day, e.hour), 16, when_style);
            buf.set_stringn(area.x + 17, y, format!("{} {:<10}", e.kind.glyph(), e.kind.label()), 14, Style::default().fg(e.kind.color()).bg(bg).add_modifier(Modifier::BOLD));
            let sp_glyph = match e.species {
                Some(s) => s.glyph().to_ascii_uppercase().to_string(),
                None => "-".to_string(),
            };
            buf.set_stringn(area.x + 32, y, sp_glyph, 1, if selected { bright } else { theme::dim_text() });
            let text_max = text_w.saturating_sub(35);
            let text = if e.text.chars().count() > text_max {
                let mut t: String = e.text.chars().take(text_max.saturating_sub(1)).collect();
                t.push('~');
                t
            } else {
                e.text.clone()
            };
            buf.set_stringn(area.x + 35, y, &text, text_max, if selected { bright } else { theme::text() });
        }
        if events.is_empty() {
            util::line(f, area, 0, Line::from(Span::styled(" no events", theme::dim_text())));
        }
        let _ = sim;
        let _ = compact;
    }

    fn detail(&self, f: &mut Frame, area: Rect, app: &AppState, sim: &Sim, e: Option<&Event>) {
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
            row = self.outbreak_record(f, inner, row, sim, e);
        }

        // Subject creature.
        if let Some(subject) = e.subject.filter(|_| !is_outbreak_event) {
            if let Some(c) = sim.creatures.get(subject) {
                let state = if c.alive { "alive".to_string() } else { format!("dead: {}", c.death.map(|d| d.cause.label()).unwrap_or("?")) };
                util::line(f, inner, row, Line::from(vec![
                    Span::styled(format!(" {} ", if c.alive { c.species.glyph().to_ascii_uppercase() } else { glyphs::CARCASS }), Style::default().fg(c.species.color()).bg(bg).add_modifier(Modifier::BOLD)),
                    Span::styled(format!("{} {}  ", c.name_str(), c.tag()), theme::text()),
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
            let ox = (x as i64 - MINI_W as i64 / 2).clamp(0, sim.world.width() as i64 - MINI_W as i64) as usize;
            let oy = (y as i64 - MINI_H as i64 / 2).clamp(0, sim.world.height() as i64 - MINI_H as i64) as usize;
            let mini = Rect::new(inner.x + 1, inner.y + row, MINI_W + 2, MINI_H + 2);
            let mini_inner = panel::draw(f, mini, "", panel::Kind::Inner);
            let opts = MapOptions {
                overlay: Overlay::None,
                night: false,
                winter: false,
                cursor: Some((x, y)),
                follow: None,
                origin: (ox, oy),
                creatures: true,
                fade_creatures: false,
                selected_region: None,
                species_color: crate::theme::TEXT,
                creature_tint: None,
            };
            map::render(f.buffer_mut(), mini_inner, sim, &opts);
        }
        let _ = row;
        let _ = app;
    }

    /// The outbreak record for an Outbreak / Epidemic / EpidemicOver / Spillover
    /// event: the pathogen named in the event text, and its latest outbreak
    /// started on or before the event day. Returns the next free row.
    fn outbreak_record(&self, f: &mut Frame, inner: Rect, mut row: u16, sim: &Sim, e: &Event) -> u16 {
        let bg = theme::PANEL_BG;
        let Some((pid, o)) = find_outbreak(sim, e) else {
            util::line(f, inner, row, Line::from(Span::styled(" no outbreak record for this event", theme::dim_text())));
            return row + 1;
        };
        let strain = sim.disease.pathogen(pid).is_some_and(|p| p.is_strain());
        let color = if strain { theme::MAGENTA } else { theme::SICK };
        let region = sim.world.regions.get(o.origin_region as usize).map(|r| r.0.as_str()).unwrap_or("?");
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
            Span::styled(day_stamp(o.started_day as i64, sim.time.season_days), theme::text()),
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
            let id = SpeciesId::ALL[i];
            let end = if o.ended_day.is_some() { o.resist_at_end[i] } else { sim.series.last().map(|s| s.genome_mean[i].resistance()).unwrap_or(o.resist_at_start[i]) };
            util::line(f, inner, row, Line::from(vec![
                Span::styled(" resist ", theme::dim_text()),
                Span::styled(format!("{} ", id.glyph().to_ascii_uppercase()), Style::default().fg(id.color()).bg(bg).add_modifier(Modifier::BOLD)),
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
    format!("{:.2}", v).replace("0.", ".")
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
        .map(|(i, _)| PathogenId(i as u8))?;
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
            species_cases: [0, 9, 0, 0, 0, 0],
            species_deaths: [0, 2, 0, 0, 0, 0],
            epidemic: false,
            resist_at_start: [0.31; 6],
            resist_at_end: [0.0; 6],
            active: 4,
            cases_today: 1,
        });
        let region = sim.world.regions[2].0.clone();
        let mk = |kind: EventKind, text: String| Event { year: 1, day: 1, hour: 6, kind, species: None, subject: None, text, pos: None, detail: String::new() };
        sim.events.push(mk(EventKind::Birth, "a birth".into()));
        sim.events.push(mk(EventKind::DeathDisease, "died of disease".into()));
        sim.events.push(mk(EventKind::Outbreak, format!("{} breaks out among the hares of {}", name, region)));
        app.sim = Some(sim);

        let mut s = EventLog::new();
        let all = screen_text(&app, &s);
        assert!(all.contains(&format!("{} disease", glyphs::DISEASE)), "eighth chip missing: {all}");
        assert!(all.contains("[f] cycles, [1-8] toggles"), "{all}");

        // Key 8 keeps only the disease kinds; disease deaths stay under deaths.
        s.handle_key(KeyEvent::new(KeyCode::Char('8'), KeyModifiers::NONE), &mut app);
        assert_eq!(s.filter.kinds, [false, false, false, false, false, false, true]);
        let text = screen_text(&app, &s);
        assert!(text.contains("breaks out"), "{text}");
        assert!(!text.contains("died of disease"), "{text}");
        assert!(!text.contains("a birth"), "{text}");
        // The detail panel shows the outbreak record.
        assert!(text.contains(&format!("began Y1 D001 in {}", region)), "{text}");
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
