//! S07a: the live event log (chips functional, no detail pane in C2).

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use crate::sim::{Event, EventKind};
use crate::ui::app::AppState;
use crate::ui::screens::{Action, Screen};
use crate::ui::style::{EventKindStyle, SpeciesStyle};
use crate::widgets::{panel, status, util};
use crate::{glyphs, theme};

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
            EventKind::DeathStarved | EventKind::DeathPredation | EventKind::DeathAge => self.kinds[1],
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
                    let events: Vec<&Event> = sim.events.iter().rev().filter(|e| self.filter.matches(e.kind)).collect();
                    if let Some(e) = events.get(self.selected).and_then(|e| e.pos) {
                        let w = sim.world.width();
                        let h = sim.world.height();
                        app.viewport_origin = (
                            e.0.saturating_sub(55).min(w.saturating_sub(110)),
                            e.1.saturating_sub(20).min(h.saturating_sub(40)),
                        );
                        return Action::Pop;
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
        let events: Vec<&Event> = sim.events.iter().rev().filter(|e| self.filter.matches(e.kind)).collect();

        let status_row = area.y + area.height - 1;
        let body = Rect::new(area.x, area.y, area.width, area.height - 1);
        let inner = panel::draw_with_hint(f, body, "Event Log", &format!("{} events", events.len()), panel::Kind::Outer);

        // Filter chip row.
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

        // Column header.
        util::line(f, inner, 1, Line::from(Span::styled(" when         kind        sp  event", theme::dim_text())));

        // Event rows (newest first).
        for (i, e) in events.iter().take(inner.height as usize - 3).enumerate() {
            let selected = i == self.selected;
            let y = inner.y + 2 + i as u16;
            let buf = f.buffer_mut();
            if selected {
                for cx in inner.x..inner.right() {
                    if let Some(c) = buf.cell_mut((cx, y)) {
                        c.set_bg(theme::SELECT_BG);
                    }
                }
            }
            let bright = Style::default().fg(theme::TEXT_BRIGHT).bg(theme::SELECT_BG);
            let when_style = if selected { bright } else { theme::dim_text() };
            buf.set_stringn(inner.x + 1, y, format!("{}Y{} D{:03} {:02}:00", if selected { "►" } else { " " }, e.year, e.day, e.hour), 16, when_style);
            buf.set_stringn(inner.x + 17, y, format!("{} {:<10}", e.kind.glyph(), e.kind.label()), 14, Style::default().fg(e.kind.color()).bg(if selected { theme::SELECT_BG } else { theme::PANEL_BG }).add_modifier(Modifier::BOLD));
            let sp_glyph = match e.species {
                Some(s) => s.glyph().to_ascii_uppercase().to_string(),
                None => "-".to_string(),
            };
            buf.set_stringn(inner.x + 32, y, sp_glyph, 1, if selected { bright } else { theme::dim_text() });
            buf.set_stringn(inner.x + 35, y, &e.text, inner.width.saturating_sub(36) as usize, if selected { bright } else { theme::text() });
        }
        if events.is_empty() {
            util::line(f, inner, 2, Line::from(Span::styled(" no events", theme::dim_text())));
        }

        let right = format!("{}  {} {}", sim.time.clock_label(), glyphs::SUN, "day");
        status::render(f, Rect::new(area.x, status_row, area.width, 1), &[("↑↓", "select"), ("1-7", "chips"), ("f", "cycle"), ("Enter", "jump"), ("Esc", "back")], &right);
    }
}
