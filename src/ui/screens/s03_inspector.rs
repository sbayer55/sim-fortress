//! S03: the live creature inspector (S03a prey / S03c corpse; S03b predator is
//! placeholder until C5). Mirrors the three-column layout with live
//!
//! data; C4 adds family names, offspring forecast, kin, legacy and timeline.
//! Each column scrolls independently: `← →` move the focus, `↑ ↓ PgUp PgDn
//! Home End` scroll it.

use std::cell::Cell;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::Frame;
use crate::sim::creatures::{Cause, Creature, CreatureId, Goal};
use crate::sim::Kind;
use crate::ui::app::AppState;
use crate::ui::screens::s08_lineage::LineageScreen;
use crate::ui::screens::{Action, Screen};
use crate::ui::style::SpeciesStyle;
use crate::widgets::scroll::{self, Overflow};
use crate::widgets::{panel, status, util};
use crate::{glyphs, theme};

use identity::{identity, identity_title};
use genome::genome;
use life::life;
use style::sp;

const LEFT_W: u16 = 52;

const MID_W: u16 = 52;

/// The three columns, in screen order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pane {
    Identity,
    Genome,
    Life,
}

impl Pane {
    const ALL: [Self; 3] = [Self::Identity, Self::Genome, Self::Life];

    const fn index(self) -> usize {
        match self {
            Self::Identity => 0,
            Self::Genome => 1,
            Self::Life => 2,
        }
    }

    const fn next(self) -> Self {
        match self {
            Self::Identity => Self::Genome,
            Self::Genome => Self::Life,
            Self::Life => Self::Identity,
        }
    }

    const fn prev(self) -> Self {
        match self {
            Self::Identity => Self::Life,
            Self::Genome => Self::Identity,
            Self::Life => Self::Genome,
        }
    }
}

#[derive(Debug)]
pub struct Inspector {
    pub id: CreatureId,
    /// The column that `↑ ↓` scroll.
    focus: Pane,
    /// Requested scroll offset per column; clamped at render time.
    offset: [u16; 3],
    /// Rows each column's body used and the rows visible, measured by the
    /// last render (the same interior-mutability trick as `viewport_size`).
    measured: Cell<([u16; 3], u16)>,
}

impl Inspector {
    pub const fn new(id: CreatureId) -> Self {
        Self { id, focus: Pane::Identity, offset: [0; 3], measured: Cell::new(([0; 3], 0)) }
    }

    /// Scroll the focused column by `delta` rows, clamped to its content.
    fn scroll_by(&mut self, delta: i32) {
        let (content, visible) = self.measured.get();
        let i = self.focus.index();
        let max = i32::from(Overflow::max_offset(content[i], visible));
        let cur = i32::from(self.offset[i]);
        self.offset[i] = crate::cast!((cur + delta).clamp(0, max) => u16);
    }

    fn panel_kind(&self, pane: Pane) -> panel::Kind {
        if self.focus == pane { panel::Kind::Focus } else { panel::Kind::Outer }
    }
}

impl Screen for Inspector {
    fn opaque(&self) -> bool {
        true
    }

    fn handle_key(&mut self, key: KeyEvent, app: &mut AppState) -> Action {
        let page = i32::from(self.measured.get().1.max(1));
        match key.code {
            KeyCode::Char('f') => {
                app.follow = Some(self.id);
                app.leave_look();
                Action::Pop
            }
            KeyCode::Tab => {
                if let Some(sim) = &app.sim {
                    let ids = sim.creatures.living_ids();
                    if !ids.is_empty() {
                        let cur = ids.iter().position(|&x| x == self.id).unwrap_or(0);
                        self.id = ids[(cur + 1) % ids.len()];
                        self.offset = [0; 3];
                    }
                }
                Action::None
            }
            KeyCode::Char('l') => Action::Push(Box::new(LineageScreen::new(self.id))),
            KeyCode::Left => {
                self.focus = self.focus.prev();
                Action::None
            }
            KeyCode::Right => {
                self.focus = self.focus.next();
                Action::None
            }
            KeyCode::Up => {
                self.scroll_by(-1);
                Action::None
            }
            KeyCode::Down => {
                self.scroll_by(1);
                Action::None
            }
            KeyCode::PageUp => {
                self.scroll_by(-page);
                Action::None
            }
            KeyCode::PageDown => {
                self.scroll_by(page);
                Action::None
            }
            KeyCode::Home => {
                self.scroll_by(i32::MIN.div_euclid(2));
                Action::None
            }
            KeyCode::End => {
                self.scroll_by(i32::MAX.div_euclid(2));
                Action::None
            }
            KeyCode::Esc => Action::Pop,
            _ => Action::Unhandled,
        }
    }

    fn render(&self, app: &AppState, f: &mut Frame<'_>, area: Rect) {
        let Some(sim) = &app.sim else { return };
        let Some(c) = sim.creatures.get(self.id) else { return };
        let status_row = area.y + area.height - 1;
        let body_h = area.height - 1;

        let left = Rect::new(area.x, area.y, LEFT_W, body_h);
        let mid = Rect::new(area.x + LEFT_W, area.y, MID_W, body_h);
        let right = Rect::new(area.x + LEFT_W + MID_W, area.y, area.width - LEFT_W - MID_W, body_h);

        let mut content = [0u16; 3];
        let mut visible = 0u16;
        for pane in Pane::ALL {
            let i = pane.index();
            let (rect, inner) = match pane {
                Pane::Identity => (left, panel::draw(f, left, identity_title(c), self.panel_kind(pane))),
                Pane::Genome => {
                    let hint = format!("vs {} mean", c.species.plural());
                    (mid, panel::draw_with_hint(f, mid, "Genome", &hint, self.panel_kind(pane)))
                }
                Pane::Life => (right, panel::draw(f, right, "Life", self.panel_kind(pane))),
            };
            let ov = scroll::draw(f, rect, inner, self.offset[i], |buf, canvas| match pane {
                Pane::Identity => identity(buf, canvas, sim, c),
                Pane::Genome => genome(buf, canvas, sim, c),
                Pane::Life => life(buf, canvas, sim, c, self.id),
            });
            content[i] = ov.content;
            visible = inner.height;
        }
        self.measured.set((content, visible));

        let right_text = format!("{} {}  {}", c.name_str(), c.tag(), sim.time.clock_label());
        status::render(
            f,
            Rect::new(area.x, status_row, area.width, 1),
            &[("f", "follow"), ("l", "lineage"), ("Tab", "next creature"), ("←→", "panel"), ("↑↓", "scroll"), ("Esc", "back")],
            &right_text,
        );
    }
}

/// `Name tag` for a relative, from the store or the lineage.
/// S03c (Identity & Death panel): the killer from
/// `death.killer` and the two nearest living predators with the Scavenge goal.
pub(crate) fn killer_and_scavengers(buf: &mut Buffer, inner: Rect, mut row: u16, sim: &crate::sim::Sim, c: &Creature) -> u16 {
    // C7 (S03c): a disease death names its outbreak instead of a killer.
    let outbreak_index = if c.death.is_some_and(|d| d.cause == Cause::Disease) {
        sim.lineage.get(c.id).and_then(|n| n.outbreak).or_else(|| c.infection.map(|i| i.outbreak))
    } else {
        None
    };
    if let Some(o) = outbreak_index.and_then(|i| sim.disease.outbreak(i)) {
        panel::section_in(buf, inner, row, "Outbreak");
        row += 1;
        let year = o.started_day.div_euclid((4 * sim.time.season_days).max(1)) + 1;
        let region = sim.world.regions.get(crate::cast!(o.origin_region => usize)).map_or("?", |r| r.0.as_str());
        util::line_in(buf, inner, row, Line::from(vec![
            sp(format!(" {} ", glyphs::DISEASE), Style::default().fg(theme::SICK).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            sp(format!("{} outbreak of Year {}, began {}", sim.disease.name(o.pathogen), year, region), theme::text()),
        ]));
        row += 1;
        util::line_in(buf, inner, row, Line::from(sp(format!("   {} others died in it", o.deaths.saturating_sub(1)), theme::dim_text())));
        row += 2;
    } else if let Some(killer_id) = c.death.and_then(|d| d.killer) {
        panel::section_in(buf, inner, row, "Killer");
        row += 1;
        let kname = sim
            .creatures
            .get(killer_id)
            .map(|k| format!("{} {}", k.name_str(), k.tag()))
            .or_else(|| sim.lineage.get(killer_id).map(|n| format!("{} {}", n.name_str(), n.tag)))
            .unwrap_or_else(|| format!("#{}", killer_id.0));
        let kglyph = sim.creatures.get(killer_id).map_or('?', |k| k.species.glyph().to_ascii_uppercase());
        let kcolor = sim.creatures.get(killer_id).map_or(theme::DIM, |k| k.species.color());
        let kills = sim.creatures.get(killer_id).map_or(0, |k| k.kills);
        let chase = c.death.map_or(0, |d| d.chase_ticks);
        util::line_in(buf, inner, row, Line::from(vec![
            sp(format!(" {kglyph} "), Style::default().fg(kcolor).bg(theme::PANEL_BG)),
            sp(kname, theme::title()),
            sp(format!("  {kills} kills  chase {chase} ticks"), theme::text()),
        ]));
        row += 2;
    }
    panel::section_in(buf, inner, row, "Scavengers nearby");
    row += 1;
    let mut scav: Vec<(f32, &Creature)> = sim
        .creatures
        .living()
        .filter(|o| o.species.kind() == Kind::Predator && o.goal == Goal::Scavenge)
        .map(|o| (crate::sim::dist(c.x, c.y, o.x, o.y), o))
        .collect();
    scav.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal).then(a.1.id.cmp(&b.1.id)));
    if scav.is_empty() {
        util::line_in(buf, inner, row, Line::from(sp(" none", theme::dim_text())));
        row += 1;
    }
    for (d, o) in scav.iter().take(2) {
        util::line_in(buf, inner, row, Line::from(vec![
            sp(format!(" {} ", o.species.glyph().to_ascii_uppercase()), Style::default().fg(o.species.color()).bg(theme::PANEL_BG)),
            sp(format!("{:<9}{:<7} {:>3.0} cells {}  {}", o.name_str(), o.tag(), d, compass(c.x, c.y, o.x, o.y), o.goal.plain()), theme::text()),
        ]));
        row += 1;
    }
    row
}

pub(crate) fn kin_name(sim: &crate::sim::Sim, id: CreatureId) -> String {
    if let Some(c) = sim.creatures.get(id) {
        return format!("{} {}", c.name_str(), c.tag());
    }
    match sim.lineage.get(id) {
        Some(n) => format!("{} {}", n.name_str(), n.tag),
        None => format!("#{}", id.0),
    }
}

/// Compass direction from `(x, y)` to `(tx, ty)` (map cells are 2:1).
pub(crate) fn compass(x: usize, y: usize, tx: usize, ty: usize) -> &'static str {
    let dx = crate::cast!(tx => i64) - crate::cast!(x => i64);
    let dy = (crate::cast!(ty => i64) - crate::cast!(y => i64)) * 2;
    let ns = if dy < -1 { "N" } else if dy > 1 { "S" } else { "" };
    let ew = if dx < -1 { "W" } else if dx > 1 { "E" } else { "" };
    match (ns, ew) {
        ("", "") => "here",
        ("N", "") => "N",
        ("S", "") => "S",
        ("", "E") => "E",
        ("", "W") => "W",
        ("N", "E") => "NE",
        ("N", "W") => "NW",
        ("S", "E") => "SE",
        _ => "SW",
    }
}

/// C7 (S03 Condition): infectious conspecifics within `contact_cheb` Chebyshev
/// cells, over 8, clamped to 0..1.
pub(crate) fn contagion_risk(sim: &crate::sim::Sim, c: &Creature) -> f32 {
    let r = crate::cast!(sim.params.disease.contact_cheb => i64);
    let n = sim
        .creatures
        .living()
        .filter(|o| o.id != c.id && o.species == c.species && crate::sim::disease::is_infectious(o))
        .filter(|o| (crate::cast!(o.x => i64) - crate::cast!(c.x => i64)).abs() <= r && (crate::cast!(o.y => i64) - crate::cast!(c.y => i64)).abs() <= r)
        .count();
    (crate::cast!(n => f32) / 8.0).clamp(0.0, 1.0)
}

pub(crate) fn local_forage(sim: &crate::sim::Sim, x: usize, y: usize) -> f32 {
    let (mut sum, mut n) = (0.0f32, 0usize);
    for dy in -3i32..=3 {
        for dx in -3i32..=3 {
            let (nx, ny) = (crate::cast!(x => i32) + dx, crate::cast!(y => i32) + dy);
            if sim.world.in_bounds(nx, ny) {
                sum += sim.world.cell(crate::cast!(nx => usize), crate::cast!(ny => usize)).vegetation;
                n += 1;
            }
        }
    }
    if n > 0 { sum / crate::cast!(n => f32) } else { 0.0 }
}

pub(crate) fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        let mut t: String = text.chars().take(max.saturating_sub(1)).collect();
        t.push(glyphs::DOT);
        t
    }
}

mod identity;
mod genome;
mod life;
mod style;
#[cfg(test)]
mod tests;
