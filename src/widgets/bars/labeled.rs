//! Labeled Bar and its Bare/Marker form. See `docs/components/labeled-bar.md`.

use std::borrow::Cow;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

use crate::widgets::component::Component;
use crate::{glyphs, theme};

/// Whether a vital reads "high is bad" (hunger, thirst).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Inverted {
    No,
    Yes,
}

/// Color for a vital: green when healthy, red when critical.
/// `inverted` = true for hunger/thirst where high is bad.
pub fn vital_color(value: f32, inverted: bool) -> Color {
    let t = if inverted { 1.0 - value } else { value };
    if t > 0.6 {
        theme::GOOD
    } else if t > 0.3 {
        theme::WARN
    } else {
        theme::BAD
    }
}

/// Just the `[████░░░░]` part, as wide as the cell it is given.
#[derive(Clone, Copy, Debug)]
pub struct Bar {
    value: f32,
    color: Color,
    marker: Option<f32>,
}

impl Bar {
    pub const fn new(value: f32) -> Self {
        Self { value, color: theme::TEXT, marker: None }
    }

    #[must_use]
    pub const fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    /// A `│` at a second value, such as the species base.
    #[must_use]
    pub const fn marker(mut self, at: f32) -> Self {
        self.marker = Some(at);
        self
    }
}

impl Component for Bar {
    fn height(&self, _width: u16) -> u16 {
        1
    }

    fn min_width(&self) -> u16 {
        3
    }

    fn render(&self, buf: &mut Buffer, area: Rect) {
        if area.height == 0 || area.width < 3 {
            return;
        }
        let inner = crate::cast!(area.width - 2 => usize);
        let filled = crate::cast!((self.value.clamp(0.0, 1.0) * crate::cast!(inner => f32)).round() => usize);
        let frame: String = std::iter::once(glyphs::BAR_L)
            .chain((0..inner).map(|i| if i < filled { glyphs::BAR_FILL } else { glyphs::BAR_EMPTY }))
            .chain(std::iter::once(glyphs::BAR_R))
            .collect();
        let dim = Style::default().fg(theme::DIM).bg(theme::PANEL_BG);
        buf.set_stringn(area.x, area.y, &frame, crate::cast!(area.width => usize), dim);
        let fill: String = std::iter::repeat_n(glyphs::BAR_FILL, filled).collect();
        buf.set_stringn(area.x + 1, area.y, &fill, filled, Style::default().fg(self.color).bg(theme::PANEL_BG));
        if let Some(at) = self.marker {
            let last = inner.saturating_sub(1);
            let cell = (crate::cast!((at.clamp(0.0, 1.0) * crate::cast!(last => f32)).round() => usize)).min(last);
            let x = area.x + 1 + crate::cast!(cell => u16);
            buf.set_stringn(x, area.y, glyphs::V_LINE.to_string(), 1, Style::default().fg(theme::TEXT_BRIGHT).bg(theme::PANEL_BG));
        }
    }
}

/// `label [████░░░░]   63%`: Label, Bar, Value and an optional Suffix.
#[derive(Clone, Debug)]
pub struct LabeledBar<'a> {
    label: Cow<'a, str>,
    bar: Bar,
    label_w: u16,
    bar_w: u16,
    /// Count / Decimal text; `None` draws the Percent form.
    value_text: Option<Cow<'a, str>>,
    suffix: Option<Cow<'a, str>>,
}

impl<'a> LabeledBar<'a> {
    /// Percent form with the S01 sidebar sizes (`label_w` 12, `bar_w` 20).
    pub fn new(label: impl Into<Cow<'a, str>>, value: f32) -> Self {
        Self { label: label.into(), bar: Bar::new(value), label_w: 12, bar_w: 20, value_text: None, suffix: None }
    }

    /// Fill colour from the vital rule.
    pub fn vital(label: impl Into<Cow<'a, str>>, value: f32, inverted: Inverted) -> Self {
        Self::new(label, value).color(vital_color(value, inverted == Inverted::Yes))
    }

    #[must_use]
    pub const fn color(mut self, color: Color) -> Self {
        self.bar = self.bar.color(color);
        self
    }

    #[must_use]
    pub const fn label_w(mut self, w: u16) -> Self {
        self.label_w = w;
        self
    }

    /// Bar cells including the brackets; never below 3.
    #[must_use]
    pub fn label_and_bar(self, label_w: u16, bar_w: u16) -> Self {
        self.label_w(label_w).bar_w(bar_w)
    }

    #[must_use]
    pub fn bar_w(mut self, w: u16) -> Self {
        self.bar_w = w.max(3);
        self
    }

    /// Count or Decimal: the Value cell shows this instead of a percentage.
    #[must_use]
    pub fn value_text(mut self, text: impl Into<Cow<'a, str>>) -> Self {
        self.value_text = Some(text.into());
        self
    }

    #[must_use]
    pub fn suffix(mut self, text: impl Into<Cow<'a, str>>) -> Self {
        self.suffix = Some(text.into());
        self
    }

    fn value(&self) -> String {
        self.value_text.as_deref().map_or_else(
            || format!("{:>3}%", crate::cast!((self.bar.value.clamp(0.0, 1.0) * 100.0).round() => u32)),
            |t| format!("{t:>4}"),
        )
    }
}

impl Component for LabeledBar<'_> {
    fn height(&self, _width: u16) -> u16 {
        1
    }

    fn min_width(&self) -> u16 {
        let suffix = self.suffix.as_deref().map_or(0, |s| crate::cast!(s.chars().count() => u16) + 1);
        self.label_w + self.bar_w + 7 + suffix
    }

    fn render(&self, buf: &mut Buffer, area: Rect) {
        // The Bar is never cut: without room for Label and Bar the row is not drawn.
        if area.height == 0 || area.width < self.label_w + self.bar_w {
            return;
        }
        let (x, y) = (area.x, area.y);
        let lw = crate::cast!(self.label_w => usize);
        buf.set_stringn(x, y, format!("{:<lw$}", self.label), lw, theme::text());
        self.bar.render(buf, Rect::new(x + self.label_w, y, self.bar_w, 1));
        let vx = x + self.label_w + self.bar_w + 3;
        let right = area.right();
        if vx + 4 > right {
            return; // Value drops before the Bar shrinks.
        }
        buf.set_stringn(vx, y, self.value(), 4, theme::text());
        if let Some(s) = self.suffix.as_deref() {
            let sx = vx + 5;
            if sx < right {
                buf.set_stringn(sx, y, s, crate::cast!(right - sx => usize), theme::text());
            }
        }
    }
}
