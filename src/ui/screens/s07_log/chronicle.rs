//! S07c: the chronicle view. One paragraph per season, newest first, from
//! `Sim.chronicle` (template entries, or model text when the chronicle feature
//! is on). Toggled from the event log with `c`; `↑↓` scroll.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use crate::sim::chronicle::Source;
use crate::sim::Sim;
use crate::ui::app::AppState;
use crate::ui::style::SeasonStyle;
use crate::widgets::scroll::{self, Overflow};
use crate::widgets::{panel, util, Component, StatusBar};
use crate::{glyphs, theme};

/// Draw the view; returns what the scroll blit measured (for clamping).
pub(super) fn render(f: &mut Frame<'_>, area: Rect, app: &AppState, sim: &Sim, offset: u16) -> Overflow {
    let status_row = area.y + area.height - 1;
    let body = Rect::new(area.x, area.y, area.width, area.height - 1);
    let hint = format!("{} seasons", sim.chronicle.len());
    let inner = panel::draw_with_hint(f, body, "Chronicle", &hint, panel::Kind::Outer);
    let pending = app.chronicle_pending.as_ref().map(|p| p.entry);
    let notice = app.ai_notice.clone();
    let measured = scroll::draw(f, body, inner, offset, |buf, canvas| draw_entries(buf, canvas, sim, pending, notice.as_deref()));

    let right = format!("{}  {} {}", sim.time.clock_label(), glyphs::SUN, "day");
    let keys: &[(&str, &str)] = &[("↑↓", "scroll"), ("c", "event log"), ("Esc", "back")];
    StatusBar::new(keys).right(&right).note(app.ai.status_note()).render(f.buffer_mut(), Rect::new(area.x, status_row, area.width, 1));
    measured
}

fn draw_entries(buf: &mut Buffer, canvas: Rect, sim: &Sim, pending: Option<usize>, notice: Option<&str>) -> u16 {
    let width = crate::cast!(canvas.width.saturating_sub(4) => usize).max(8);
    let mut row = 0u16;
    if let Some(n) = notice {
        util::line_in(buf, canvas, row, Line::from(Span::styled(format!(" {n}"), theme::dim_text())));
        row += 2;
    }
    if sim.chronicle.is_empty() {
        util::line_in(buf, canvas, row, Line::from(Span::styled(" The first entry is written when the season turns.", theme::dim_text())));
        return row + 1;
    }
    for (i, e) in sim.chronicle.iter().enumerate().rev() {
        let writing = pending == Some(i);
        let tag = match (writing, e.source) {
            (true, _) => "writing...",
            (false, Source::Model) => "chronicled",
            (false, Source::Template) => "tally",
        };
        util::line_in(buf, canvas, row, Line::from(vec![
            Span::styled(format!(" {} ", e.season.glyph()), Style::default().fg(e.season.color()).bg(theme::PANEL_BG)),
            Span::styled(format!("Year {}, {}", e.year, e.season.name()), theme::title().add_modifier(Modifier::BOLD)),
            Span::styled(format!("   {tag}"), theme::dim_text()),
        ]));
        row += 1;
        for line in wrap_words(&e.text, width) {
            util::line_in(buf, canvas, row, Line::from(Span::styled(format!("   {line}"), theme::text())));
            row += 1;
        }
        row += 1;
        if row >= scroll::CANVAS_H {
            break;
        }
    }
    row
}

/// Greedy word wrap; explicit newlines start a new line.
pub(super) fn wrap_words(text: &str, width: usize) -> Vec<String> {
    let mut out = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        for word in para.split_whitespace() {
            let need = if line.is_empty() { word.chars().count() } else { line.chars().count() + 1 + word.chars().count() };
            if need > width && !line.is_empty() {
                out.push(std::mem::take(&mut line));
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
        out.push(line);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::wrap_words;

    #[test]
    fn wraps_on_words_and_newlines() {
        let lines = wrap_words("The voles of the marsh thrived.\nThe foxes went hungry.", 14);
        assert_eq!(lines, vec!["The voles of", "the marsh", "thrived.", "The foxes went", "hungry."]);
        assert_eq!(wrap_words("", 10), vec![""]);
    }
}
