//! S01c look mode.

use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::Frame;
use crate::sim::world::Feature;
use crate::sim::{Sim, World};
use crate::ui::app::AppState;
use crate::widgets::{panel, util};
use crate::{glyphs, theme};
use super::WorldMap;

/// Arrow-key cursor movement in look mode (clamped to the world).
pub(super) fn move_look_cursor(cursor: &mut Option<(usize, usize)>, code: KeyCode, step: usize, dims: (usize, usize)) {
    let Some((x, y)) = *cursor else { return };
    *cursor = Some(match code {
        KeyCode::Left => (x.saturating_sub(step), y),
        KeyCode::Right => ((x + step).min(dims.0.saturating_sub(1)), y),
        KeyCode::Up => (x, y.saturating_sub(step)),
        KeyCode::Down => (x, (y + step).min(dims.1.saturating_sub(1))),
        _ => (x, y),
    });
}

/// "lake · 14 cells", "river · 61 cells · to the Ulmar Sea", "range · 23 cells".
fn feature_detail(world: &World, feat: &Feature) -> String {
    let head = format!("{} {} {} cells", feat.kind.label(), glyphs::DOT, feat.cells);
    let (mx, my) = feat.mouth;
    match world.feature_at(mx, my).filter(|t| feat.kind.is_river() && !t.kind.is_river()) {
        Some(to) => format!("{head} {} to {}", glyphs::DOT, to.name),
        None => head,
    }
}

impl WorldMap {
    /// S01c sidebar: the cursor cell readout.
    pub(super) fn look_sidebar(f: &mut Frame<'_>, area: Rect, app: &AppState, sim: &Sim, world: &World) {
        let inner = panel::draw(f, area, "Look", panel::Kind::Outer);
        let mut row = 0u16;
        let Some((cx, cy)) = app.look_cursor else { return };
        let cell = world.cell(cx, cy);
        panel::section(f, inner, row, "Cursor");
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(format!(" ({}, {})  {}", cx, cy, world.region_name(cx, cy)), theme::text())));
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(format!(" {}  {}  elev {:.2}  veg {:.2}", world.terrain_name(cx, cy), cell.biome.name(), cell.elevation, cell.vegetation), theme::dim_text())));
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(format!(" moisture {:.2}  temperature {:.2}", cell.moisture, cell.temperature), theme::dim_text())));
        row += 1;
        if let Some(feat) = world.feature_near(cx, cy) {
            util::line(f, inner, row, Line::from(vec![
                Span::styled(" ", theme::text()),
                Span::styled(feat.name.clone(), theme::title()),
                Span::styled(format!("  {}", feature_detail(world, feat)), theme::dim_text()),
            ]));
            row += 1;
        }
        util::line(f, inner, row, Line::from(Span::styled(" Enter opens the creature inspector", theme::dim_text())));
        row += 2;
        let here = sim.creatures.living().filter(|c| c.x == cx && c.y == cy).count();
        util::line(f, inner, row, Line::from(Span::styled(format!(" creatures here: {here}"), theme::text())));
        row += 2;
        panel::section(f, inner, row, "Keys");
        row += 1;
        for (k, v) in [("Enter", "inspect"), ("f", "follow"), ("z", "zoom"), ("Esc", "exit look")] {
            util::line(f, inner, row, Line::from(vec![Span::styled(format!(" {k} "), theme::key()), Span::styled(v, theme::text())]));
            row += 1;
        }
    }
}
