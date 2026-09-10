//! Row 0: prototype id + name, and navigation hints on the right.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use ratatui::Frame;

use crate::prototypes::Prototype;
use crate::theme;

pub fn render(f: &mut Frame, area: Rect, p: &dyn Prototype, index: usize, total: usize) {
    let bg = Style::default().bg(theme::HEADER_BG);
    let left = Line::from(vec![
        Span::styled(" ", bg),
        Span::styled(p.id(), bg.fg(theme::KEY).add_modifier(Modifier::BOLD)),
        Span::styled("  ", bg),
        Span::styled(p.name(), bg.fg(theme::HEADER_FG).add_modifier(Modifier::BOLD)),
        Span::styled(" — ", bg.fg(theme::DIM)),
        Span::styled(p.variant(), bg.fg(theme::HEADER_FG)),
    ]);
    let right = format!(
        "{}/{}  [ ] prev/next   0-9 jump   q quit ",
        index, total
    );
    let right_w = right.len() as u16;
    Paragraph::new(left).style(bg).render(area, f.buffer_mut());
    if area.width > right_w {
        let r = Rect::new(area.x + area.width - right_w, area.y, right_w, 1);
        Paragraph::new(Line::from(Span::styled(right, bg.fg(theme::DIM))))
            .render(r, f.buffer_mut());
    }
}
