//! The inline `[k] label` token and rows of them. See `docs/components/key-hint.md`.

use std::borrow::Cow;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

use super::component::{Align, Component};
use super::text::Text;
use crate::theme;

/// `[Key] Label`: the key bold in `theme::KEY`, the label after one cell.
#[derive(Clone, Debug)]
pub struct KeyHint<'a> {
    key: Cow<'a, str>,
    label: Option<Cow<'a, str>>,
    bg: Color,
    label_style: Style,
    /// Dim: the whole token in `theme::dim_text()`, the key not highlighted.
    dim: bool,
    /// Bracketless: `Key Label` in `theme::dim_text()`.
    brackets: bool,
}

impl<'a> KeyHint<'a> {
    pub fn new(key: impl Into<Cow<'a, str>>, label: impl Into<Cow<'a, str>>) -> Self {
        Self { key: key.into(), label: Some(label.into()), bg: theme::PANEL_BG, label_style: theme::text(), dim: false, brackets: true }
    }

    /// Key only: `[Enter]`.
    pub fn key(key: impl Into<Cow<'a, str>>) -> Self {
        Self { key: key.into(), label: None, bg: theme::PANEL_BG, label_style: theme::text(), dim: false, brackets: true }
    }

    /// A row of tokens joined by two cells.
    pub fn row(pairs: &'a [(&'a str, &'a str)]) -> KeyHintRow<'a> {
        KeyHintRow { pairs, sep: Cow::Borrowed("  "), bg: theme::PANEL_BG, label_style: theme::text(), dim: false, brackets: true, align: Align::Left }
    }

    /// The row's background (`theme::STATUS_BG` in the bar, `theme::SELECT_BG` in a menu).
    #[must_use]
    pub const fn bg(mut self, bg: Color) -> Self {
        self.bg = bg;
        self
    }

    #[must_use]
    pub const fn label_style(mut self, style: Style) -> Self {
        self.label_style = style;
        self
    }

    #[must_use]
    pub const fn dim(mut self) -> Self {
        self.dim = true;
        self
    }

    #[must_use]
    pub const fn bracketless(mut self) -> Self {
        self.brackets = false;
        self
    }

    fn spans(&self) -> Vec<Span<'a>> {
        let key_style = if self.dim || !self.brackets {
            theme::dim_text().bg(self.bg)
        } else {
            Style::default().fg(theme::KEY).bg(self.bg).add_modifier(Modifier::BOLD)
        };
        let label_style = if self.dim || !self.brackets { theme::dim_text().bg(self.bg) } else { self.label_style.bg(self.bg) };
        let key = if self.brackets { format!("[{}]", self.key) } else { self.key.to_string() };
        let mut v = vec![Span::styled(key, key_style)];
        if let Some(label) = &self.label {
            v.push(Span::styled(format!(" {label}"), label_style));
        }
        v
    }
}

impl Component for KeyHint<'_> {
    fn height(&self, _width: u16) -> u16 {
        1
    }

    fn min_width(&self) -> u16 {
        crate::cast!(self.spans().iter().map(Span::width).sum::<usize>() => u16)
    }

    fn render(&self, buf: &mut Buffer, area: Rect) {
        Text::spans(self.spans()).render(buf, area);
    }
}

/// Several tokens on one row, joined by a separator.
#[derive(Clone, Debug)]
pub struct KeyHintRow<'a> {
    pairs: &'a [(&'a str, &'a str)],
    sep: Cow<'a, str>,
    bg: Color,
    label_style: Style,
    dim: bool,
    brackets: bool,
    align: Align,
}

impl<'a> KeyHintRow<'a> {
    #[must_use]
    pub fn separator(mut self, sep: impl Into<Cow<'a, str>>) -> Self {
        self.sep = sep.into();
        self
    }

    #[must_use]
    pub const fn bg(mut self, bg: Color) -> Self {
        self.bg = bg;
        self
    }

    #[must_use]
    pub const fn label_style(mut self, style: Style) -> Self {
        self.label_style = style;
        self
    }

    #[must_use]
    pub const fn dim(mut self) -> Self {
        self.dim = true;
        self
    }

    #[must_use]
    pub const fn bracketless(mut self) -> Self {
        self.brackets = false;
        self
    }

    #[must_use]
    pub const fn center(mut self) -> Self {
        self.align = Align::Center;
        self
    }

    fn text(&self) -> Text<'a> {
        let sep_style = if self.dim || !self.brackets { theme::dim_text().bg(self.bg) } else { self.label_style.bg(self.bg) };
        let mut spans = Vec::new();
        for (i, (k, l)) in self.pairs.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(self.sep.clone(), sep_style));
            }
            let mut hint = KeyHint::new(*k, *l).bg(self.bg).label_style(self.label_style);
            if self.dim {
                hint = hint.dim();
            }
            if !self.brackets {
                hint = hint.bracketless();
            }
            spans.extend(hint.spans());
        }
        Text::spans(spans).align(self.align)
    }
}

impl Component for KeyHintRow<'_> {
    fn height(&self, _width: u16) -> u16 {
        1
    }

    fn min_width(&self) -> u16 {
        self.text().min_width()
    }

    fn render(&self, buf: &mut Buffer, area: Rect) {
        self.text().render(buf, area);
    }
}
