//! S10: simulation controls modal drawn over the dimmed world map.

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use crate::ui::app::AppState;
use crate::ui::screens::{Action, Screen};
use crate::ui::style::SeasonStyle;
use crate::widgets::{panel, status, util};
use crate::{glyphs, theme};

const SPEEDS: [u32; 5] = [1, 2, 5, 10, 25];
const STEPS: [(&str, u64); 3] = [("1 tick", 1), ("6 hours", 6), ("1 day", 24)];

pub struct Controls {
    step_idx: usize,
}

impl Default for Controls {
    fn default() -> Self {
        Self::new()
    }
}

impl Controls {
    pub fn new() -> Self {
        Controls { step_idx: 0 }
    }
}

impl Screen for Controls {
    fn opaque(&self) -> bool {
        false
    }

    fn handle_key(&mut self, key: KeyEvent, app: &mut AppState) -> Action {
        match key.code {
            KeyCode::Esc => Action::Pop,
            KeyCode::Char(' ') => {
                app.toggle_pause();
                Action::None
            }
            KeyCode::Char('+') => {
                app.speed_up();
                Action::None
            }
            KeyCode::Char('-') => {
                app.speed_down();
                Action::None
            }
            KeyCode::Char('1') => {
                app.set_speed(0);
                Action::None
            }
            KeyCode::Char('2') => {
                app.set_speed(1);
                Action::None
            }
            KeyCode::Char('3') => {
                app.set_speed(2);
                Action::None
            }
            KeyCode::Char('4') => {
                app.set_speed(3);
                Action::None
            }
            KeyCode::Char('5') => {
                app.set_speed(4);
                Action::None
            }
            KeyCode::Char('.') => {
                app.paused = true;
                app.step_ticks(STEPS[self.step_idx].1);
                Action::None
            }
            KeyCode::Char('<') => {
                self.step_idx = self.step_idx.saturating_sub(1);
                Action::None
            }
            KeyCode::Char('>') => {
                if self.step_idx + 1 < STEPS.len() {
                    self.step_idx += 1;
                }
                Action::None
            }
            KeyCode::Char('a') => {
                app.params.ui.auto_pause_on_extinction = !app.params.ui.auto_pause_on_extinction;
                Action::None
            }
            KeyCode::Char('b') => {
                app.params.ui.log_births = !app.params.ui.log_births;
                Action::None
            }
            KeyCode::Char('c') => {
                app.params.ui.pause_on_follow_death = !app.params.ui.pause_on_follow_death;
                Action::None
            }
            _ => Action::Unhandled,
        }
    }

    fn render(&self, app: &AppState, f: &mut Frame, area: Rect) {
        let Some(sim) = &app.sim else {
            return;
        };
        let time = &sim.time;

        let modal = util::centered(area, 60.min(area.width.saturating_sub(2)), 18.min(area.height.saturating_sub(2)));
        let inner = panel::draw_with_hint(f, modal, "Simulation Controls", "Esc closes", panel::Kind::Focus);
        let mut row = 0u16;

        let running = !app.paused;
        let (state_glyph, state_txt, state_color) = if running {
            (glyphs::PLAY.to_string(), "RUNNING", theme::GOOD)
        } else {
            (glyphs::PAUSE_STR.to_string(), "PAUSED", theme::WARN)
        };
        util::line(f, inner, row, Line::from(vec![
            Span::styled(" state   ", theme::label()),
            Span::styled(format!("{} {}", state_glyph, state_txt), Style::default().fg(state_color).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            Span::styled(format!("      {} x{}  ", glyphs::FAST_STR, app.speed()), theme::text()),
            Span::styled(format!("{} {}", glyphs::PAUSE_STR, "Space toggles"), theme::dim_text()),
        ]));
        row += 2;

        let mut spans = vec![Span::styled(" speed   ", theme::label())];
        for s in SPEEDS {
            let label = format!(" x{s} ");
            if s == app.speed() {
                spans.push(Span::styled(label, theme::selected()));
            } else {
                spans.push(Span::styled(label, theme::text()));
            }
            spans.push(Span::styled(" ", theme::text()));
        }
        spans.push(Span::styled("   +/- or 1-5", theme::dim_text()));
        util::line(f, inner, row, Line::from(spans));
        row += 1;

        let mut spans = vec![Span::styled(" step    ", theme::label())];
        for (i, (s, _)) in STEPS.iter().enumerate() {
            let label = format!(" {s} ");
            if i == self.step_idx {
                spans.push(Span::styled(label, Style::default().fg(theme::TEXT_BRIGHT).bg(theme::PANEL_BG).add_modifier(Modifier::UNDERLINED)));
            } else {
                spans.push(Span::styled(label, theme::text()));
            }
            spans.push(Span::styled(" ", theme::text()));
        }
        spans.push(Span::styled(format!("   {} . steps once", glyphs::STEP_STR), theme::dim_text()));
        util::line(f, inner, row, Line::from(spans));
        row += 2;

        panel::section(f, inner, row, "Clock");
        row += 1;
        let season = time.season();
        util::line(f, inner, row, Line::from(vec![
            Span::styled(" tick ", theme::dim_text()),
            Span::styled(crate::ui::screens::s01_map::group(time.tick), theme::text()),
            Span::styled("    day ", theme::dim_text()),
            Span::styled(format!("{}", time.day_of_season()), theme::text()),
            Span::styled(format!(" of {} ", season.name()), theme::dim_text()),
            Span::styled(season.glyph().to_string(), Style::default().fg(season.color()).bg(theme::PANEL_BG)),
            Span::styled("    year ", theme::dim_text()),
            Span::styled(format!("{}", time.year()), theme::text()),
            Span::styled(format!("    {} {}", time.hour_label(), glyphs::SUN), theme::text()),
        ]));
        row += 1;
        let ticks_per_day = app.params.time.ticks_per_day;
        util::line(f, inner, row, Line::from(Span::styled(
            format!(" 1 tick = 1 hour   1 day = {ticks_per_day} ticks   x{} = {} ticks/s", app.speed(), 2 * app.speed()),
            theme::dim_text(),
        )));
        row += 2;

        panel::section(f, inner, row, "Options");
        row += 1;
        let toggles: [(&str, bool, &str); 3] = [
            ("a", app.params.ui.auto_pause_on_extinction, "auto-pause on extinction"),
            ("b", app.params.ui.log_births, "show births in the map ticker"),
            ("c", app.params.ui.pause_on_follow_death, "pause when a followed creature dies"),
        ];
        for (key, on, label) in toggles {
            let mark = if on { "[x]" } else { "[ ]" };
            let mark_style = if on {
                Style::default().fg(theme::GOOD).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)
            } else {
                theme::dim_text()
            };
            util::line(f, inner, row, Line::from(vec![
                Span::styled(" ", theme::text()),
                Span::styled(mark, mark_style),
                Span::styled(format!(" {:<36}", label), theme::text()),
                Span::styled(format!("[{key}]"), theme::key()),
            ]));
            row += 1;
        }

        let hint_row = inner.height - 1;
        util::line(f, inner, hint_row, Line::from(vec![
            Span::styled(" ", theme::text()),
            Span::styled("[Space]", theme::key()),
            Span::styled(" pause  ", theme::dim_text()),
            Span::styled("[+/-]", theme::key()),
            Span::styled(" speed  ", theme::dim_text()),
            Span::styled("[.]", theme::key()),
            Span::styled(" step  ", theme::dim_text()),
            Span::styled("[Esc]", theme::key()),
            Span::styled(" close", theme::dim_text()),
        ]));

        // Repaint the status bar undimmed.
        let status_row = area.y + area.height - 1;
        util::fill(f.buffer_mut(), Rect::new(area.x, status_row, area.width, 1), Style::default().bg(theme::STATUS_BG));
        let keys: &[(&str, &str)] = &[("Space", "pause"), ("+/-", "speed"), ("1-5", "set speed"), (".", "step"), ("a/b/c", "toggle"), ("Esc", "close")];
        let sky = if time.is_night() { glyphs::MOON } else { glyphs::SUN };
        let right = format!("{}  {} {}", time.clock_label(), sky, if time.is_night() { "night" } else { "day" });
        status::render(f, Rect::new(area.x, status_row, area.width, 1), keys, &right);
    }
}
