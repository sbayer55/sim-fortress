//! S09 preview panel, below the biome shares: the world summary and the
//! generation chronicle, both derived from the preview world.

use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::Frame;

use crate::sim::world::{chronicle, summary};
use crate::sim::World;
use crate::widgets::{panel, util};
use crate::{glyphs, theme};

/// Draw the `World` summary and the `Chronicle` from `row` down; the
/// chronicle is capped so the block never scrolls.
pub(super) fn summary_and_chronicle(f: &mut Frame<'_>, inner: Rect, mut row: u16, world: &World, age: u8) {
    let s = summary(world);
    panel::section(f, inner, row, "World");
    row += 1;
    // Two lines, each at most 80 cells with the longest names (22) and
    // region names (16), so nothing clips in the 87-cell panel.
    let sea = world.features_of_kind(crate::sim::world::FeatureKind::Ocean).next().map_or_else(String::new, |o| format!(" on {}", o.name));
    util::line(f, inner, row, Line::from(vec![
        Span::styled(" coast ", theme::dim_text()),
        Span::styled(format!("{} cells{sea}", s.coast), theme::text()),
        Span::styled("  peak ", theme::dim_text()),
        Span::styled(format!("{:.2} in {}", s.peak.1, s.peak.2), theme::text()),
    ]));
    row += 1;
    let river = s.longest_river.as_ref().map_or_else(|| "none".to_string(), |(name, cells)| format!("{name} {cells} cells"));
    util::line(f, inner, row, Line::from(vec![
        Span::styled(" longest river ", theme::dim_text()),
        Span::styled(river, theme::text()),
        Span::styled(format!("  {} {} lakes {} {} rivers {} {} ranges", glyphs::DOT, s.lakes, glyphs::DOT, s.rivers, glyphs::DOT, s.ranges), theme::dim_text()),
    ]));
    row += 1;
    panel::section(f, inner, row, "Chronicle");
    row += 1;
    for line in chronicle(world, age) {
        if row >= inner.height {
            break;
        }
        util::line(f, inner, row, Line::from(vec![
            Span::styled(format!(" {} ", glyphs::NOTE), theme::dim_text()),
            Span::styled(line, theme::text()),
        ]));
        row += 1;
    }
}
