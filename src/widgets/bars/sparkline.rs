//! One-row `░▒▓█` strip, one cell per sample. See `docs/components/sparkline.md`.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

use crate::widgets::component::Component;
use crate::{glyphs, theme};

/// Draws the last `width` values, shaded relative to the drawn slice.
#[derive(Clone, Debug)]
pub struct Sparkline {
    values: Vec<u16>,
    color: Color,
}

impl Sparkline {
    pub fn new(values: &[u16]) -> Self {
        Self { values: values.to_vec(), color: theme::TEXT }
    }

    #[must_use]
    pub const fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }
}

impl Component for Sparkline {
    fn height(&self, _width: u16) -> u16 {
        1
    }

    fn min_width(&self) -> u16 {
        1
    }

    fn render(&self, buf: &mut Buffer, area: Rect) {
        if area.height == 0 {
            return;
        }
        let n = self.values.len().min(crate::cast!(area.width => usize));
        let Some(slice) = self.values.get(self.values.len() - n..) else { return };
        let max = f32::from(slice.iter().copied().max().unwrap_or(1).max(1));
        let min = f32::from(slice.iter().copied().min().unwrap_or(0));
        let s: String = slice
            .iter()
            .map(|&v| {
                let t = if max > min { (f32::from(v) - min) / (max - min) } else { 0.5 };
                let i = crate::cast!((t * 3.0).round() => usize) + 1;
                glyphs::SHADES[i.min(4)]
            })
            .collect();
        buf.set_stringn(area.x, area.y, &s, n, Style::default().fg(self.color).bg(theme::PANEL_BG));
    }
}
