//! Bordered panel helper: double-line outer panels, single-line inner splits.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Widget};
use ratatui::Frame;

use crate::theme;

#[derive(Clone, Copy, PartialEq, Eq)]
#[derive(Debug)]
pub enum Kind {
    Outer,
    Inner,
    Focus,
}

/// Draw a titled panel and return the inner area.
pub fn draw(f: &mut Frame<'_>, area: Rect, title: &str, kind: Kind) -> Rect {
    draw_with_hint(f, area, title, "", kind)
}

/// Draw a titled panel with a right-aligned hint in the top border.
pub fn draw_with_hint(f: &mut Frame<'_>, area: Rect, title: &str, hint: &str, kind: Kind) -> Rect {
    let (border_type, border_style) = match kind {
        Kind::Outer => (BorderType::Double, theme::border()),
        Kind::Inner => (BorderType::Plain, theme::border()),
        Kind::Focus => (BorderType::Double, theme::border_focus()),
    };
    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_type(border_type)
        .border_style(border_style)
        .style(Style::default().bg(theme::PANEL_BG));
    if !title.is_empty() {
        block = block.title(Line::from(vec![
            Span::styled(" ", theme::border()),
            Span::styled(title, theme::title().add_modifier(Modifier::BOLD)),
            Span::styled(" ", theme::border()),
        ]));
    }
    if !hint.is_empty() {
        block = block.title(
            Line::from(vec![
                Span::styled(" ", theme::border()),
                Span::styled(hint, theme::dim_text()),
                Span::styled(" ", theme::border()),
            ])
            .right_aligned(),
        );
    }
    let inner = block.inner(area);
    super::util::fill(f.buffer_mut(), area, Style::default().bg(theme::PANEL_BG));
    block.render(area, f.buffer_mut());
    inner
}

/// A single-row section title inside a panel: `── Title ────`.
pub fn section(f: &mut Frame<'_>, area: Rect, row: u16, title: &str) {
    if row >= area.height {
        return;
    }
    let y = area.y + row;
    let buf = f.buffer_mut();
    let line: String = std::iter::repeat_n('─', crate::cast!(area.width => usize)).collect();
    buf.set_stringn(area.x, y, &line, crate::cast!(area.width => usize), theme::border());
    let t = format!(" {title} ");
    buf.set_stringn(area.x + 1, y, &t, (crate::cast!(area.width => usize)).saturating_sub(2), theme::label());
}
