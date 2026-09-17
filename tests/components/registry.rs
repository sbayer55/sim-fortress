//! One entry per example heading in `docs/components/*.md`. Keep the heading
//! text verbatim, including inline code; `examples_render_cell_for_cell`
//! matches on it.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Span;
use sim_fortress::{cast, theme};
use sim_fortress::widgets::{Component, Divider, Kind, Panel, Spacer, Text, VStack};

use super::{Entry, Frame};

/// Row `i` of `area`, one row high.
const fn row(area: Rect, i: u16) -> Rect {
    Rect::new(area.x, area.y + i, area.width, 1)
}

fn bare(sheet: &'static str, heading: &'static str, draw: fn(&mut Buffer, Rect)) -> Entry {
    Entry { sheet, heading, frame: Frame::Bare, fg: &[], bg: &[], draw }
}

fn rows(sheet: &'static str, heading: &'static str, draw: fn(&mut Buffer, Rect)) -> Entry {
    Entry { sheet, heading, frame: Frame::Rows, fg: &[], bg: &[], draw }
}

/// A panel with one text row per line, for the area examples.
fn panel_with(buf: &mut Buffer, area: Rect, panel: &Panel<'_>, lines: &[&str]) {
    let inner = panel.render(buf, area);
    for (i, l) in lines.iter().enumerate() {
        Text::new(*l).render(buf, row(inner, cast!(i => u16)));
    }
}

fn greeting_with_metadata(buf: &mut Buffer, area: Rect) {
    let inner = Panel::new("Greeting").render(buf, area);
    let (a, d, b) = (Text::new(" Hello!"), Divider::new("Metadata"), Text::new(" Sent Sep 9th, 2026"));
    VStack::new().child(&a).child(&d).child(&b).render(buf, inner);
}

pub fn examples() -> Vec<Entry> {
    let mut v = vec![
        // ---- panel.md
        bare("panel", "Simple (43 columns)", |b, a| panel_with(b, a, &Panel::new("Greeting"), &[" Hello!"])),
        bare("panel", "With Info (43 columns)", |b, a| {
            panel_with(b, a, &Panel::new("Status").info("Esc closes"), &[" Year 12  Day 4     ♫ Autumn"]);
        }),
        Entry {
            fg: &[(0, 0, theme::BORDER_FOCUS), (42, 2, theme::BORDER_FOCUS)],
            ..bare("panel", "Focus (43 columns)", |b, a| {
                let p = Panel::new("Simulation Controls").info("Esc closes").kind(Kind::Focus);
                panel_with(b, a, &p, &[" state   ► RUNNING      ►► x2"]);
            })
        },
        bare("panel", "Inner (43 columns)", |b, a| {
            panel_with(b, a, &Panel::new("Surroundings").kind(Kind::Inner), &[" tile contents"]);
        }),
        bare("panel", "Nested, Inner inside Outer (43 columns)", |b, a| {
            let inner = Panel::new("Life").render(b, a);
            let nested = Rect::new(inner.x + 1, inner.y, inner.width - 2, 3);
            let p = Panel::new("Surroundings").kind(Kind::Inner);
            panel_with(b, nested, &p, &["~~~~~♠♣\"\"\"\"\"\"\"***,,*,♣♣♠♠♠♠\"\"\"\"\"\"\"\"**"]);
        }),
        bare("panel", "Scrolled, Foot set by Scroll Region (43 columns)", |b, a| {
            let p = Panel::new("Legend").foot("↑3 ↓12");
            panel_with(b, a, &p, &[" ≈ deep water       ~ shallow water", " · sand             . bare dirt"]);
        }),
        bare("panel", "Divider inside a panel (43 columns)", greeting_with_metadata),
        // ---- divider.md
        rows("divider", "In-panel (43 columns)", |b, a| Divider::new("Clock").render(b, a)),
        bare("divider", "In a panel body (43 columns)", greeting_with_metadata),
        rows("divider", "No text (43 columns)", |b, a| Divider::rule().render(b, a)),
        bare("divider", "Bare, 41 columns wide", |b, a| Divider::new("Population, last 240 days").render(b, a)),
        rows("divider", "Text cut at width − 2 (43 columns)", |b, a| {
            Divider::new("Offspring forecast (with an average mate) tonight").render(b, a);
        }),
        // ---- spacer.md
        rows("spacer", "One blank row between sections (43 columns)", |b, a| {
            let (t, s, d) = (Text::new(" 14:00  ☼ day      ►► x2  running"), Spacer::rows(1), Divider::new("Resources"));
            VStack::new().child(&t).child(&s).child(&d).render(b, a);
        }),
        // ---- text.md
        rows("text", "Plain (43 columns)", |b, a| Text::new(" Hello!").render(b, a)),
        rows("text", "Right-aligned (43 columns)", |b, a| Text::new(" 44%").right().render(b, a)),
        rows("text", "Centred (43 columns)", |b, a| Text::new("Enter select   ←→ move   Esc continue").center().render(b, a)),
        rows("text", "Cut at the right edge (43 columns)", |b, a| {
            Text::new("The quick brown fox jumps over the lazy dog and on").render(b, a);
        }),
        Entry {
            fg: &[(1, 1, theme::TEXT), (21, 1, theme::ACCENT)],
            ..rows("text", "Styled spans (43 columns)", |b, a| {
                Text::spans(vec![
                    Span::styled(" Year 12  Day 4", theme::text()),
                    Span::styled("     ♫ Autumn", Style::default().fg(theme::ACCENT).bg(theme::PANEL_BG)),
                ])
                .render(b, a);
            })
        },
        Entry {
            fg: &[(1, 1, theme::DIM)],
            ..rows("text", "Dim (43 columns)", |b, a| Text::new(" tick 5,184").style(theme::dim_text()).render(b, a))
        },
    ];
    v.sort_by_key(|e| (e.sheet, e.heading));
    v
}
