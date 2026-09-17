//! S04b — per-trait histograms for the selected species.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::Frame;
use crate::sim::species::TRAIT_ABBR;
use crate::sim::{Genome, Sim, SpeciesId, TRAIT_NAMES};
use crate::ui::screens::common::{sp, trait_color, two};
use crate::ui::style::SpeciesStyle;
use crate::widgets::{bars, panel, util};
use crate::{glyphs, theme};
use std::fmt::Write as _;
use super::drift::selection_pressure;

pub(super) fn histograms(f: &mut Frame<'_>, area: Rect, sim: &Sim, id: SpeciesId) {
    let s = &sim.species[id.index()];
    let inner = panel::draw_with_hint(f, area, &format!("{}: trait distributions", sim.roster().display_name(id)), "12 buckets, living adults + juveniles", panel::Kind::Outer);
    // Twelve traits fill a 3 x 4 grid of 25-column blocks (23-wide histograms).
    // `block_h` is exactly the block height (name, hist, axis, labels) so the
    // former spacer row is gone and the grid still leaves room for the two
    // comparison sections below it inside 42 rows.
    let col_w = 25u16;
    let block_h = 7u16;
    let rows = 4u16;
    for t in 0..Genome::LEN {
        let col = crate::cast!((t.div_euclid(crate::cast!(rows => usize))) => u16);
        let r = crate::cast!((t % crate::cast!(rows => usize)) => u16);
        let x = inner.x + 1 + col * (col_w + 1);
        let y = inner.y + 1 + r * block_h;
        let color = trait_color(t);
        let mean = s.mean.0[t];
        let (min, max) = (s.min.0[t], s.max.0[t]);
        let buf = f.buffer_mut();
        buf.set_stringn(x, y, TRAIT_NAMES[t], 11, Style::default().fg(color).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD));
        buf.set_stringn(
            x + 11,
            y,
            format!(".{:02} .{:02} .{:02}", crate::cast!((min * 100.0).round() => u32) % 100, crate::cast!((mean * 100.0).round() => u32) % 100, crate::cast!((max * 100.0).round() => u32) % 100),
            14,
            theme::dim_text(),
        );
        let hist_area = Rect::new(x, y + 1, 24, 4);
        if s.count > 0 {
            bars::histogram(buf, hist_area, &s.hist[t], color, 2);
        }
        let axis: String = std::iter::repeat_n(glyphs::H_LINE, 24).collect();
        buf.set_stringn(x, y + 5, &axis, 24, theme::border());
        let mx = x + (crate::cast!((mean * 23.0).round() => u16)).min(23);
        buf.set_stringn(mx, y + 5, glyphs::CROSS.to_string(), 1, Style::default().fg(theme::TEXT_BRIGHT).bg(theme::PANEL_BG));
        buf.set_stringn(x, y + 6, "0", 1, theme::dim_text());
        buf.set_stringn(x + 23, y + 6, "1", 1, theme::dim_text());
        let n: u32 = s.hist[t].iter().map(|&v| u32::from(v)).sum();
        let peak_bucket = s.hist[t].iter().enumerate().max_by_key(|(_, v)| **v).map_or(0, |(i, _)| i);
        buf.set_stringn(x + 2, y + 6, format!("n={n}"), 7, theme::dim_text());
        buf.set_stringn(x + 11, y + 6, format!("mode .{:02}", crate::cast!(((crate::cast!(peak_bucket => f32) + 0.5) / 12.0 * 100.0).round() => u32)), 11, theme::dim_text());
    }
    let y = inner.y + 1 + rows * block_h;
    let buf = f.buffer_mut();
    buf.set_stringn(
        inner.x + 1,
        y,
        format!("{} mean  {} full  {} half  columns = 1/12 of 0..1  header: min mean max", glyphs::CROSS, glyphs::FULL_BLOCK, glyphs::HALF_LOWER),
        crate::cast!(inner.width => usize) - 2,
        theme::dim_text(),
    );
    // C7: the two trait-comparison sections moved here from the drift panel to
    // make room for its Disease section (eleven traits already fill the top).
    let mut row = y - inner.y + 1;
    panel::section(f, inner, row, "Selection pressure");
    row += 1;
    for note in selection_pressure(s).into_iter().take(2) {
        util::line(f, inner, row, Line::from(sp(format!(" {note}"), theme::dim_text())));
        row += 1;
    }
    row += 1;
    panel::section(f, inner, row, "Compared with other species (mean x100)");
    row += 1;
    let mut cmp_hdr = String::from("             ");
    for a in TRAIT_ABBR {
        let _ = write!(cmp_hdr, " {a:>3}");
    }
    cmp_hdr.push_str("   count  gen");
    util::line(f, inner, row, Line::from(sp(cmp_hdr, theme::dim_text())));
    row += 1;
    for other in &sim.species {
        if row >= inner.height {
            break;
        }
        let absent = other.count == 0;
        let mut spans = vec![
            sp(format!(" {} ", sim.roster().adult_glyph(other.species)), Style::default().fg(if absent { theme::DIM } else { sim.roster().color(other.species) }).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            sp(format!("{:<10}", sim.roster().display_name(other.species)), if other.species == id { theme::title() } else if absent { theme::dim_text() } else { theme::text() }),
        ];
        for t in 0..Genome::LEN {
            spans.push(sp(format!(" {:>3}", two(other.mean.0[t])), Style::default().fg(if absent { theme::DIM } else { trait_color(t) }).bg(theme::PANEL_BG)));
        }
        spans.push(sp(format!("   {:>5}  {:>3}", other.count, other.generation), theme::dim_text()));
        util::line(f, inner, row, Line::from(spans));
        row += 1;
    }
}
