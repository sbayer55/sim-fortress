//! Names drawn over the map under the region overlay: the eight region
//! names, bold and bright, and the named features (lakes, rivers, ranges)
//! in plain text beneath them.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use super::{cell_at, MapOptions};
use crate::sim::world::{FeatureKind, World};
use crate::theme;

/// Where region `ri`'s label starts in world coordinates: centred on the
/// region's centre cell, clamped so the whole label stays inside its
/// bounding box and the world.
pub fn region_label_origin(world: &World, ri: usize) -> (usize, usize) {
    let Some(r) = world.regions.get(ri) else { return (0, 0) };
    let w = r.0.chars().count();
    let (cx, cy) = world.region_centre(ri);
    let x = cx.saturating_sub(w.div_euclid(2)).max(r.1);
    let x = x.min(r.3.saturating_sub(w)).min(world.width().saturating_sub(w));
    (x, cy)
}

/// Draw each region's name, bold and bright, clipped (never shifted) at the
/// viewport edge.
pub(super) fn region_labels(buf: &mut Buffer, area: Rect, world: &World, opts: &MapOptions) {
    for (i, r) in world.regions.iter().enumerate() {
        let (lx, ly) = region_label_origin(world, i);
        let selected = opts.selected_region == Some(i);
        for (k, ch) in r.0.chars().enumerate() {
            if let Some(cell) = cell_at(buf, area, opts, lx + k, ly) {
                cell.set_char(ch);
                let st = if selected {
                    theme::selected()
                } else {
                    Style::default().fg(theme::TEXT_BRIGHT).bg(cell.bg).add_modifier(Modifier::BOLD)
                };
                cell.set_style(st);
            }
        }
    }
}

/// Draw each named feature's name at its anchor, plain (not bold, not
/// bright) so the region names stay dominant, skipping any that would
/// land on a region label's row within its span; the ocean is left
/// unlabelled (it is the coast, not a place).
pub(super) fn feature_labels(buf: &mut Buffer, area: Rect, world: &World, opts: &MapOptions) {
    let taken: Vec<(usize, usize, usize)> = (0..world.regions.len())
        .map(|ri| {
            let (x, y) = region_label_origin(world, ri);
            (x, y, world.regions[ri].0.chars().count())
        })
        .collect();
    for f in world.names.features.iter().filter(|f| f.kind != FeatureKind::Ocean) {
        let len = f.name.chars().count();
        let (ax, ay) = f.anchor;
        let lx = ax.saturating_sub(len.div_euclid(2)).min(world.width().saturating_sub(len));
        if taken.iter().any(|&(x, y, w)| y == ay && lx < x + w && x < lx + len) {
            continue;
        }
        for (k, ch) in f.name.chars().enumerate() {
            if let Some(cell) = cell_at(buf, area, opts, lx + k, ay) {
                cell.set_char(ch);
                cell.set_style(Style::default().fg(theme::TEXT).bg(cell.bg));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::params::WorldParams;
    use crate::widgets::map::Overlay;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn feature_labels_are_plain_and_skip_region_label_rows() {
        let world = World::generate(1, &WorldParams::default());
        let (w, h) = (crate::cast!(world.width() => u16), crate::cast!(world.height() => u16));
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        let opts = MapOptions { overlay: Overlay::Region, ..MapOptions::default() };
        terminal
            .draw(|f| {
                let area = Rect::new(0, 0, w, h);
                feature_labels(f.buffer_mut(), area, &world, &opts);
                region_labels(f.buffer_mut(), area, &world, &opts);
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        let row = |y: usize| (0..w).map(|x| buf[(x, crate::cast!(y => u16))].symbol().chars().next().unwrap_or(' ')).collect::<String>();
        let mut drawn = 0;
        for f in world.names.features.iter().filter(|f| f.kind != FeatureKind::Ocean) {
            let line = row(f.anchor.1);
            if let Some(x) = line.find(&f.name) {
                drawn += 1;
                let cell = &buf[(crate::cast!(x => u16), crate::cast!(f.anchor.1 => u16))];
                assert_eq!(cell.fg, theme::TEXT, "{} is plain text", f.name);
                assert!(!cell.modifier.contains(Modifier::BOLD), "{} is not bold", f.name);
            }
        }
        assert!(drawn > 0, "at least one feature label is drawn");
        for ri in 0..world.regions.len() {
            let (x, y) = region_label_origin(&world, ri);
            let cell = &buf[(crate::cast!(x => u16), crate::cast!(y => u16))];
            assert!(cell.modifier.contains(Modifier::BOLD), "region label {} is intact", world.regions[ri].0);
        }
    }
}
