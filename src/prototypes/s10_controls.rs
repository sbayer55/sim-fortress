//! S10: simulation controls modal drawn over the dimmed world map.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use super::s01_map;
use super::Prototype;
use crate::fixtures;
use crate::widgets::{panel, status, util};
use crate::{glyphs, theme};

pub struct Controls;

pub fn all() -> Vec<Box<dyn Prototype>> {
    vec![Box::new(Controls)]
}

const SPEEDS: [u8; 5] = [1, 2, 5, 10, 25];
const STEPS: [&str; 3] = ["1 tick", "1 hour", "1 day"];

impl Prototype for Controls {
    fn id(&self) -> &'static str {
        "S10a"
    }
    fn name(&self) -> &'static str {
        "Simulation Controls"
    }
    fn variant(&self) -> &'static str {
        "modal over world map"
    }

    fn render(&self, f: &mut Frame, area: Rect) {
        let fx = fixtures::get();
        s01_map::render_base(f, area);
        util::dim_area(f.buffer_mut(), area, 0.55);

        let modal = util::centered(area, 60, 18);
        let inner = panel::draw_with_hint(f, modal, "Simulation Controls", "Esc closes", panel::Kind::Focus);
        let mut row = 0u16;

        // ---- state
        let running = !fx.clock.paused;
        let (state_glyph, state_txt, state_color) = if running {
            (glyphs::PLAY.to_string(), "RUNNING", theme::GOOD)
        } else {
            (glyphs::PAUSE_STR.to_string(), "PAUSED", theme::WARN)
        };
        util::line(f, inner, row, Line::from(vec![
            Span::styled(" state   ", theme::label()),
            Span::styled(format!("{} {}", state_glyph, state_txt), Style::default().fg(state_color).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            Span::styled(format!("      {} x{}  ", glyphs::FAST_STR, fx.clock.speed), theme::text()),
            Span::styled(format!("{} {}", glyphs::PAUSE_STR, "Space toggles"), theme::dim_text()),
        ]));
        row += 2;

        // ---- speed selector
        let mut spans = vec![Span::styled(" speed   ", theme::label())];
        for s in SPEEDS {
            let label = format!(" x{} ", s);
            if s == fx.clock.speed {
                spans.push(Span::styled(label, theme::selected()));
            } else {
                spans.push(Span::styled(label, theme::text()));
            }
            spans.push(Span::styled(" ", theme::text()));
        }
        spans.push(Span::styled("   +/- or 1-5", theme::dim_text()));
        util::line(f, inner, row, Line::from(spans));
        row += 1;

        // ---- step selector
        let mut spans = vec![Span::styled(" step    ", theme::label())];
        for (i, s) in STEPS.iter().enumerate() {
            let label = format!(" {} ", s);
            if i == 1 {
                spans.push(Span::styled(label, Style::default().fg(theme::TEXT_BRIGHT).bg(theme::PANEL_BG).add_modifier(Modifier::UNDERLINED)));
            } else {
                spans.push(Span::styled(label, theme::text()));
            }
            spans.push(Span::styled(" ", theme::text()));
        }
        spans.push(Span::styled(format!("   {} . steps once", glyphs::STEP_STR), theme::dim_text()));
        util::line(f, inner, row, Line::from(spans));
        row += 2;

        // ---- clock readout
        panel::section(f, inner, row, "Clock");
        row += 1;
        let c = &fx.clock;
        util::line(f, inner, row, Line::from(vec![
            Span::styled(" tick ", theme::dim_text()),
            Span::styled(s01_map::group(c.tick), theme::text()),
            Span::styled("    day ", theme::dim_text()),
            Span::styled(format!("{}", c.day), theme::text()),
            Span::styled(format!(" of {} ", c.season.name()), theme::dim_text()),
            Span::styled(c.season.glyph().to_string(), Style::default().fg(c.season.color()).bg(theme::PANEL_BG)),
            Span::styled("    year ", theme::dim_text()),
            Span::styled(format!("{}", c.year), theme::text()),
            Span::styled(format!("    {:02}:00 {}", c.hour, glyphs::SUN), theme::text()),
        ]));
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(
            format!(" 24 ticks = 1 hour   1 day = 576 ticks   x{} = {} ticks/s", c.speed, 24 * c.speed as u32),
            theme::dim_text(),
        )));
        row += 2;

        // ---- toggles
        panel::section(f, inner, row, "Options");
        row += 1;
        let toggles: [(&str, bool, &str); 3] = [
            ("a", true, "auto-pause on extinction"),
            ("b", false, "log births to the event log"),
            ("c", true, "pause when a followed creature dies"),
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
                Span::styled(format!("[{}]", key), theme::key()),
            ]));
            row += 1;
        }

        // ---- key hints at the bottom of the modal
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

        // Status bar stays readable under the modal.
        let status_row = area.y + area.height - 1;
        util::fill(f.buffer_mut(), Rect::new(area.x, status_row, area.width, 1), Style::default().bg(theme::STATUS_BG));
        let keys: &[(&str, &str)] = &[("Space", "pause"), ("+/-", "speed"), ("1-5", "set speed"), (".", "step"), ("a/b/c", "toggle"), ("Esc", "close")];
        let right = format!("{}  {} day", fx.clock.label(), glyphs::SUN);
        status::render(f, Rect::new(area.x, status_row, area.width, 1), keys, &right);
    }
}
