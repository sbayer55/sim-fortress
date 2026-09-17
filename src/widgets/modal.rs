//! A Focus panel centred over the screen: body, buttons, hint. See `docs/components/modal.md`.
//!
//! The backdrop dim is the screen stack's job (`render_stack` dims every cell
//! before a non-opaque screen draws); a Modal only draws its box.

use std::borrow::Cow;
use std::fmt;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use super::button_row::ButtonRow;
use super::component::Component;
use super::panel::{Kind, Panel};
use super::text::Text;
use crate::theme;

/// A `w × h` box centred in its area; `render` returns the Body area.
pub struct Modal<'a> {
    w: u16,
    h: u16,
    title: Option<Cow<'a, str>>,
    info: Option<Cow<'a, str>>,
    banner: Option<(Cow<'a, str>, Color)>,
    buttons: Option<ButtonRow<'a>>,
    hint: Option<Box<dyn Component + 'a>>,
}

impl fmt::Debug for Modal<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Modal").field("w", &self.w).field("h", &self.h).field("title", &self.title).finish()
    }
}

impl<'a> Modal<'a> {
    pub const fn new(w: u16, h: u16) -> Self {
        Self { w, h, title: None, info: None, banner: None, buttons: None, hint: None }
    }

    #[must_use]
    pub fn title(mut self, title: impl Into<Cow<'a, str>>) -> Self {
        self.title = Some(title.into());
        self
    }

    #[must_use]
    pub fn info(mut self, info: impl Into<Cow<'a, str>>) -> Self {
        self.info = Some(info.into());
        self
    }

    /// A bold heading centred over the (untitled) top edge.
    #[must_use]
    pub fn banner(mut self, text: impl Into<Cow<'a, str>>, color: Color) -> Self {
        self.banner = Some((text.into(), color));
        self
    }

    /// A centred Button Row above the Hint.
    #[must_use]
    pub fn buttons(mut self, buttons: &'a [&'a str], focused: Option<usize>) -> Self {
        self.buttons = Some(ButtonRow::new(buttons).focused(focused));
        self
    }

    /// Dim, centred text on the last inner row.
    #[must_use]
    pub fn hint(mut self, hint: impl Into<Cow<'a, str>>) -> Self {
        self.hint = Some(Box::new(Text::new(hint).style(theme::dim_text()).center()));
        self
    }

    /// Any component on the last inner row, such as a `KeyHintRow`.
    #[must_use]
    pub fn hint_with(mut self, hint: impl Component + 'a) -> Self {
        self.hint = Some(Box::new(hint));
        self
    }

    /// The box, clamped to `area` and centred in it.
    pub fn frame(&self, area: Rect) -> Rect {
        super::util::centered(area, self.w, self.h)
    }

    /// The Body rows: the inner area above the Buttons and Hint.
    pub fn body(&self, area: Rect) -> Rect {
        let inner = Panel::inner(self.frame(area));
        let taken = self.rows_taken(inner.height);
        Rect { height: inner.height - taken, ..inner }
    }

    /// Rows the Buttons and Hint take; a short modal loses the Hint first.
    fn rows_taken(&self, inner_h: u16) -> u16 {
        let buttons = u16::from(self.buttons.is_some());
        let hint = u16::from(self.hint.is_some());
        (buttons + hint).min(inner_h).min(if inner_h < buttons + hint { buttons.min(inner_h) } else { buttons + hint })
    }

    /// Draw the box, Banner, Buttons and Hint; returns the Body area.
    pub fn render(&self, buf: &mut Buffer, area: Rect) -> Rect {
        let frame = self.frame(area);
        let mut panel = self.title.as_deref().map_or_else(Panel::untitled, Panel::new).kind(Kind::Focus);
        if let Some(info) = self.info.as_deref() {
            panel = panel.info(info);
        }
        let inner = panel.render(buf, frame);
        if let Some((text, color)) = &self.banner {
            let text = format!(" {text} ");
            let w = crate::cast!(text.chars().count() => u16);
            if frame.width >= w + 2 {
                let x = frame.x + 1 + (frame.width - 2 - w).div_euclid(2);
                buf.set_stringn(x, frame.y, &text, crate::cast!(w => usize), Style::default().fg(*color).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD));
            }
        }
        let body = self.body(area);
        let mut y = body.bottom();
        if let Some(buttons) = &self.buttons {
            if y < inner.bottom() {
                buttons.render(buf, Rect::new(inner.x, y, inner.width, 1));
                y += 1;
            }
        }
        if let Some(hint) = &self.hint {
            if y < inner.bottom() {
                hint.render(buf, Rect::new(inner.x, y, inner.width, 1));
            }
        }
        body
    }
}

impl Component for Modal<'_> {
    fn height(&self, _width: u16) -> u16 {
        self.h
    }

    /// The widest of the Button row, the Hint, and Title plus Info, inside the border.
    fn min_width(&self) -> u16 {
        let title = self.title.as_deref().map_or(0, |t| crate::cast!(t.chars().count() => u16) + 2);
        let info = self.info.as_deref().map_or(0, |t| crate::cast!(t.chars().count() => u16) + 2);
        let buttons = self.buttons.as_ref().map_or(0, Component::min_width);
        let hint = self.hint.as_deref().map_or(0, Component::min_width);
        2 + (title + info).max(buttons).max(hint)
    }

    fn render(&self, buf: &mut Buffer, area: Rect) {
        Self::render(self, buf, area);
    }
}
