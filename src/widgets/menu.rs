//! A vertical list of choices with one selected row. See `docs/components/menu.md`.

use std::borrow::Cow;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use super::component::Component;
use crate::{glyphs, theme};

/// One row per item; the selected row is a `theme::selected()` bar.
#[derive(Clone, Debug)]
pub struct Menu<'a> {
    items: &'a [&'a str],
    selected: Option<usize>,
    disabled: &'a [usize],
    hint: Option<Cow<'a, str>>,
    note: Option<Cow<'a, str>>,
    label_w: u16,
    marker: bool,
}

impl<'a> Menu<'a> {
    pub const fn new(items: &'a [&'a str]) -> Self {
        Self { items, selected: None, disabled: &[], hint: None, note: None, label_w: 24, marker: true }
    }

    #[must_use]
    pub const fn selected(mut self, selected: usize) -> Self {
        self.selected = Some(selected);
        self
    }

    /// Rows that cannot be selected, drawn dim.
    #[must_use]
    pub const fn disabled(mut self, disabled: &'a [usize]) -> Self {
        self.disabled = disabled;
        self
    }

    /// Right-aligned on the selected row, e.g. `[Enter]`.
    #[must_use]
    pub fn hint(mut self, hint: impl Into<Cow<'a, str>>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Right-aligned on disabled rows, e.g. `(empty)`.
    #[must_use]
    pub fn note(mut self, note: impl Into<Cow<'a, str>>) -> Self {
        self.note = Some(note.into());
        self
    }

    #[must_use]
    pub const fn label_w(mut self, label_w: u16) -> Self {
        self.label_w = label_w;
        self
    }

    /// Row highlight variant: no Marker column, Label from column 1.
    #[must_use]
    pub const fn marker(mut self, marker: bool) -> Self {
        self.marker = marker;
        self
    }

    const fn label_x(&self) -> u16 {
        if self.marker {
            5
        } else {
            1
        }
    }
}

impl Component for Menu<'_> {
    fn height(&self, _width: u16) -> u16 {
        crate::cast!(self.items.len() => u16)
    }

    fn min_width(&self) -> u16 {
        let longest = self.items.iter().map(|i| crate::cast!(i.chars().count() => u16)).max().unwrap_or(0);
        let hint = self.hint.as_deref().map_or(0, |h| crate::cast!(h.chars().count() => u16) + 1);
        self.label_x() + longest.max(if hint > 0 { self.label_w + hint } else { 0 })
    }

    fn render(&self, buf: &mut Buffer, area: Rect) {
        for (i, item) in self.items.iter().enumerate() {
            let y = area.y.saturating_add(crate::cast!(i => u16));
            if y >= area.bottom() {
                break;
            }
            let selected = self.selected == Some(i);
            let disabled = self.disabled.contains(&i);
            let style = if selected {
                theme::selected()
            } else if disabled {
                theme::dim_text()
            } else {
                theme::text()
            };
            let row = Rect::new(area.x, y, area.width, 1);
            super::util::fill(buf, row, style);
            let lw = crate::cast!(self.label_w => usize);
            let prefix = if self.marker { format!("   {} ", if selected { glyphs::PLAY } else { ' ' }) } else { " ".to_string() };
            let line = format!("{prefix}{item:<lw$}");
            buf.set_stringn(area.x, y, &line, crate::cast!(area.width => usize), style);
            let tail = match (selected, disabled) {
                (true, _) => self.hint.as_deref().map(|h| (h, Style::default().fg(theme::KEY).bg(theme::SELECT_BG).add_modifier(Modifier::BOLD))),
                (false, true) => self.note.as_deref().map(|n| (n, style)),
                _ => None,
            };
            if let Some((text, st)) = tail {
                let w = crate::cast!(text.chars().count() => u16);
                let label_end = self.label_x() + crate::cast!(item.chars().count() => u16);
                if area.width > w + 1 && area.width - 1 - w >= label_end + 1 {
                    buf.set_stringn(area.right() - 1 - w, y, text, crate::cast!(w => usize), st);
                }
            }
        }
    }
}
