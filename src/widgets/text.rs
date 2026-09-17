//! One styled row of text. See `docs/components/text.md`.

use std::borrow::Cow;

use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

use super::component::{Align, Component};
use crate::theme;

/// A single row of one or more styled spans, left-, right- or centre-aligned
/// inside the row it is given. Paints only its own cells.
#[derive(Clone, Debug)]
pub struct Text<'a> {
    spans: Vec<Span<'a>>,
    align: Align,
}

impl<'a> Text<'a> {
    /// Plain text in `theme::text()`.
    pub fn new(s: impl Into<Cow<'a, str>>) -> Self {
        Self { spans: vec![Span::styled(s, theme::text())], align: Align::Left }
    }

    /// A row of pre-styled spans.
    pub const fn spans(spans: Vec<Span<'a>>) -> Self {
        Self { spans, align: Align::Left }
    }

    /// A ratatui `Line`; its line-level style becomes the base of every span.
    pub fn line(line: Line<'a>) -> Self {
        let base = line.style;
        let spans = line.spans.into_iter().map(|s| Span::styled(s.content, base.patch(s.style))).collect();
        Self { spans, align: Align::Left }
    }

    /// Replace the style of every span.
    #[must_use]
    pub fn style(mut self, style: Style) -> Self {
        for s in &mut self.spans {
            s.style = style;
        }
        self
    }

    /// Set the foreground of every span.
    #[must_use]
    pub fn fg(mut self, color: Color) -> Self {
        for s in &mut self.spans {
            s.style = s.style.fg(color);
        }
        self
    }

    /// Make every span bold.
    #[must_use]
    pub fn bold(mut self) -> Self {
        for s in &mut self.spans {
            s.style = s.style.add_modifier(Modifier::BOLD);
        }
        self
    }

    #[must_use]
    pub const fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    #[must_use]
    pub const fn right(self) -> Self {
        self.align(Align::Right)
    }

    #[must_use]
    pub const fn center(self) -> Self {
        self.align(Align::Center)
    }

    fn width(&self) -> usize {
        self.spans.iter().map(Span::width).sum()
    }
}

impl Component for Text<'_> {
    fn height(&self, _width: u16) -> u16 {
        1
    }

    fn min_width(&self) -> u16 {
        crate::cast!(self.width() => u16)
    }

    fn render(&self, buf: &mut Buffer, area: Rect) {
        if area.height == 0 || area.width == 0 {
            return;
        }
        // Wider than the row: cut at the right edge whatever the alignment.
        let align = if self.width() > usize::from(area.width) { Align::Left } else { self.align };
        let alignment = match align {
            Align::Left => Alignment::Left,
            Align::Right => Alignment::Right,
            Align::Center => Alignment::Center,
        };
        let line = Line::from(self.spans.clone()).alignment(alignment);
        (&line).render(Rect { height: 1, ..area }, buf);
    }
}
