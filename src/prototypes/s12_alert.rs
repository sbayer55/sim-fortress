//! S12: extinction alert modal drawn over the dimmed world map.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use super::s01_map;
use super::Prototype;
use crate::fixtures::{self, SpeciesId};
use crate::widgets::{panel, status, util};
use crate::{glyphs, theme};

pub struct Alert;

pub fn all() -> Vec<Box<dyn Prototype>> {
    vec![Box::new(Alert)]
}

const BUTTONS: [&str; 3] = ["[ Continue ]", "[ View lineage ]", "[ Pause ]"];

impl Prototype for Alert {
    fn id(&self) -> &'static str {
        "S12a"
    }
    fn name(&self) -> &'static str {
        "Alert Modal"
    }
    fn variant(&self) -> &'static str {
        "extinction event"
    }

    fn render(&self, f: &mut Frame, area: Rect) {
        let fx = fixtures::get();
        s01_map::render_base(f, area);
        util::dim_area(f.buffer_mut(), area, 0.55);

        let modal = util::centered(area, 64, 12);
        let inner = panel::draw(f, modal, "", panel::Kind::Focus);
        // Custom-styled title on the top border.
        {
            let buf = f.buffer_mut();
            let title = format!(" {} EXTINCTION {} ", glyphs::EXTINCTION, glyphs::EXTINCTION);
            let x = modal.x + (modal.width - title.chars().count() as u16) / 2;
            buf.set_stringn(x, modal.y, &title, title.chars().count(), Style::default().fg(theme::MAGENTA).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD));
        }

        let lynx = fx.species.iter().find(|s| s.id == SpeciesId::Lynx).unwrap();
        let mut row = 1u16;

        let center = |s: &str| -> String {
            let pad = (inner.width as usize).saturating_sub(s.chars().count()) / 2;
            format!("{}{}", " ".repeat(pad), s)
        };

        util::line(f, inner, row, Line::from(Span::styled(
            center("The Lynx are extinct"),
            Style::default().fg(theme::MAGENTA).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD),
        )));
        row += 2;
        util::line(f, inner, row, Line::from(vec![
            Span::styled("  Year 12, Day 4 of Autumn ", theme::text()),
            Span::styled(glyphs::AUTUMN.to_string(), Style::default().fg(fx.clock.season.color()).bg(theme::PANEL_BG)),
            Span::styled("   last individual: ", theme::dim_text()),
            Span::styled("Gloam l#088", Style::default().fg(theme::LYNX).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
        ]));
        row += 1;
        util::line(f, inner, row, Line::from(vec![
            Span::styled("  ", theme::text()),
            Span::styled(format!("{} ", glyphs::DEATH), Style::default().fg(theme::WARN).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            Span::styled("starved", Style::default().fg(theme::WARN).bg(theme::PANEL_BG)),
            Span::styled(" in Sunfall Coast at (131, 9), age 412 days", theme::text()),
        ]));
        row += 2;
        util::line(f, inner, row, Line::from(vec![
            Span::styled("  peak population ", theme::dim_text()),
            Span::styled(format!("{}", lynx.peak), theme::text()),
            Span::styled("   generations survived ", theme::dim_text()),
            Span::styled(format!("{}", lynx.generation), theme::text()),
            Span::styled("   years ", theme::dim_text()),
            Span::styled(format!("{}", fx.clock.year), theme::text()),
        ]));
        row += 2;

        // ---- buttons
        let total: usize = BUTTONS.iter().map(|b| b.chars().count()).sum::<usize>() + 2 * (BUTTONS.len() - 1) * 2;
        let pad = (inner.width as usize).saturating_sub(total) / 2;
        let mut spans = vec![Span::styled(" ".repeat(pad), theme::text())];
        for (i, b) in BUTTONS.iter().enumerate() {
            let st = if i == 0 { theme::selected() } else { theme::text() };
            spans.push(Span::styled(*b, st));
            spans.push(Span::styled("    ", theme::text()));
        }
        util::line(f, inner, row, Line::from(spans));
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(
            center("Enter select   ←→ move   Esc continue"),
            theme::dim_text(),
        )));

        // Status bar under the modal.
        let status_row = area.y + area.height - 1;
        util::fill(f.buffer_mut(), Rect::new(area.x, status_row, area.width, 1), Style::default().bg(theme::STATUS_BG));
        let keys: &[(&str, &str)] = &[("Enter", "select"), ("←→", "move"), ("l", "lineage"), ("Space", "pause"), ("Esc", "continue")];
        let right = format!("{} paused on extinction", glyphs::PAUSE_STR);
        status::render(f, Rect::new(area.x, status_row, area.width, 1), keys, &right);
    }
}
