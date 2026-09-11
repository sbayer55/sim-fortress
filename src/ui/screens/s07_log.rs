//! S07: the live event log. S07a lists every event; S07b (any narrow filter)
//! shows a detail panel with a mini-map and subject link.

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget, Wrap};
use ratatui::Frame;

use crate::sim::{Event, EventKind, Sim};
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

/// The S07 filter-chip state. `all` is active, or a subset of the six kinds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChipFilter {
    pub all: bool,
    /// births, deaths, mutations, migrations, extinctions, droughts.
    pub kinds: [bool; 6],
}

impl Default for ChipFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl ChipFilter {
    pub fn new() -> Self {
        ChipFilter { all: true, kinds: [false; 6] }
    }

    /// Toggle chip `key` (1 = all, 2..=7 = births..droughts).
    pub fn toggle(&mut self, key: usize) {
        if key == 1 {
            self.all = true;
            self.kinds = [false; 6];
            return;
        }
        let i = key - 2;
        if self.all {
            self.all = false;
            self.kinds = [false; 6];
            self.kinds[i] = true;
        } else {
            self.kinds[i] = !self.kinds[i];
            if !self.kinds.iter().any(|&k| k) {
                self.all = true;
            }
        }
    }

    /// Cycle presets: all → deaths+extinctions → migrations+droughts → all.
    pub fn cycle(&mut self) {
        if self.all {
            self.all = false;
            self.kinds = [false; 6];
            self.kinds[1] = true;
            self.kinds[4] = true;
        } else if self.kinds[1] && self.kinds[4] {
            self.kinds = [false; 6];
            self.kinds[3] = true;
            self.kinds[5] = true;
        } else {
            self.all = true;
            self.kinds = [false; 6];
        }
    }

    pub fn matches(&self, kind: EventKind) -> bool {
        if self.all {
            return true;
        }
        match kind {
            EventKind::Birth => self.kinds[0],
            EventKind::DeathStarved | EventKind::DeathThirst | EventKind::DeathPredation | EventKind::DeathAge => self.kinds[1],
            EventKind::Mutation => self.kinds[2],
            EventKind::Migration => self.kinds[3],
            EventKind::Extinction => self.kinds[4],
            EventKind::Drought | EventKind::DroughtEased => self.kinds[5],
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
            KeyCode::Char(c @ '2'..='7') => {
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
            &[("↑↓", "select"), ("1-7", "chips"), ("f", "cycle"), ("Enter", "jump"), ("Esc", "back")]
        };
        status::render(f, Rect::new(area.x, status_row, area.width, 1), keys, &right);
    }
}

impl EventLog {
    fn chips(&self, f: &mut Frame, inner: Rect) {
        let chips: [(char, &str, EventKind); 7] = [
            ('*', " all", EventKind::Note),
            ('♥', " births", EventKind::Birth),
            ('x', " deaths", EventKind::DeathPredation),
            ('§', " mutations", EventKind::Mutation),
            ('→', " migrations", EventKind::Migration),
            ('‼', " extinctions", EventKind::Extinction),
            ('¡', " droughts", EventKind::Drought),
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

        // Subject creature.
        if let Some(subject) = e.subject {
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
            };
            map::render(f.buffer_mut(), mini_inner, sim, &opts);
        }
        let _ = row;
        let _ = app;
    }
}
