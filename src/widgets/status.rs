//! Bottom status / key-hint bar.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use ratatui::Frame;

use crate::theme;

/// Render `[key] label` pairs across the bar, with an optional right-side message
/// in the accent colour.
pub fn render(f: &mut Frame<'_>, area: Rect, keys: &[(&str, &str)], right: &str) {
    render_colored(f, area, keys, right, theme::ACCENT);
}

/// `render` with the right-side message in `right_fg`.
pub fn render_colored(f: &mut Frame<'_>, area: Rect, keys: &[(&str, &str)], right: &str, right_fg: Color) {
    let bg = Style::default().bg(theme::STATUS_BG);
    let mut spans = vec![Span::styled(" ", bg)];
    for (k, label) in keys {
        spans.push(Span::styled(format!("[{k}]"), bg.fg(theme::KEY).add_modifier(Modifier::BOLD)));
        spans.push(Span::styled(format!(" {label}  "), bg.fg(theme::TEXT)));
    }
    Paragraph::new(Line::from(spans)).style(bg).render(area, f.buffer_mut());
    if !right.is_empty() {
        let w = (crate::cast!(right.chars().count() => u16) + 1).min(area.width);
        let r = Rect::new(area.x + area.width - w, area.y, w, 1);
        Paragraph::new(Line::from(Span::styled(format!("{right} "), bg.fg(right_fg))))
            .render(r, f.buffer_mut());
    }
}
