//! Bottom status / key-hint bar.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use ratatui::Frame;

use crate::theme;

/// Render `[key] label` pairs across the bar, with an optional right-side message.
pub fn render(f: &mut Frame, area: Rect, keys: &[(&str, &str)], right: &str) {
    let bg = Style::default().bg(theme::STATUS_BG);
    let mut spans = vec![Span::styled(" ", bg)];
    for (k, label) in keys {
        spans.push(Span::styled(format!("[{}]", k), bg.fg(theme::KEY).add_modifier(Modifier::BOLD)));
        spans.push(Span::styled(format!(" {}  ", label), bg.fg(theme::TEXT)));
    }
    Paragraph::new(Line::from(spans)).style(bg).render(area, f.buffer_mut());
    if !right.is_empty() {
        let w = (right.chars().count() as u16 + 1).min(area.width);
        let r = Rect::new(area.x + area.width - w, area.y, w, 1);
        Paragraph::new(Line::from(Span::styled(format!("{} ", right), bg.fg(theme::ACCENT))))
            .render(r, f.buffer_mut());
    }
}
