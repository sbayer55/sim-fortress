//! S11: legend & help overlay drawn over the dimmed world map. The three
//! columns scroll together with `↑ ↓ PgUp PgDn Home End`.

use std::cell::Cell;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::Frame;

use crate::sim::{EventKind, Roster, Season};
use crate::ui::app::AppState;
use crate::ui::screens::{Action, Screen};
use crate::ui::style::{EventKindStyle, SeasonStyle};
use crate::widgets::scroll::Overflow;
use crate::widgets::Constraint::{Fill, Fixed};
use crate::widgets::{Component, Divider, HStack, Legend, Modal, Rows, ScrollRegion, Spacer, StatusBar, Text, VStack};
use crate::{glyphs, theme};

#[derive(Debug)]
pub struct Help {
    /// Requested scroll offset; clamped at render time.
    offset: u16,
    /// Rows the content used and the rows visible, measured by the last render.
    measured: Cell<(u16, u16)>,
}

impl Default for Help {
    fn default() -> Self {
        Self::new()
    }
}

impl Help {
    pub const fn new() -> Self {
        Self { offset: 0, measured: Cell::new((0, 0)) }
    }

    /// Scroll by `delta` rows, clamped to the measured content.
    fn scroll_by(&mut self, delta: i32) {
        let (content, visible) = self.measured.get();
        let max = i32::from(Overflow::max_offset(content, visible));
        self.offset = crate::cast!((i32::from(self.offset) + delta).clamp(0, max) => u16);
    }
}

impl Screen for Help {
    fn opaque(&self) -> bool {
        false
    }

    fn handle_key(&mut self, key: KeyEvent, _app: &mut AppState) -> Action {
        let page = i32::from(self.measured.get().1.max(1));
        let delta = match key.code {
            KeyCode::Char('?') | KeyCode::Esc => return Action::Pop,
            KeyCode::Up => -1,
            KeyCode::Down => 1,
            KeyCode::PageUp => -page,
            KeyCode::PageDown => page,
            KeyCode::Home => i32::MIN.div_euclid(2),
            KeyCode::End => i32::MAX.div_euclid(2),
            _ => return Action::Unhandled,
        };
        self.scroll_by(delta);
        Action::None
    }

    fn render(&self, app: &AppState, f: &mut Frame<'_>, area: Rect) {
        let roster = &app.params.species;
        let (terrain, creatures, keys) = (terrain_column(), creature_column(roster), keys_column());
        let (a, b, c) = (VStack::from_boxes(&terrain), VStack::from_boxes(&creatures), VStack::from_boxes(&keys));
        let (rule_a, rule_b) = (Rule, Rule);
        let columns = HStack::new().child_with(Fill(1), &a).child_with(Fixed(1), &rule_a).child_with(Fill(1), &b).child_with(Fixed(1), &rule_b).child_with(Fill(1), &c);
        let region = ScrollRegion::new(&columns).offset(self.offset);

        let modal = Modal::new(120.min(area.width.saturating_sub(2)), 38.min(area.height.saturating_sub(2))).title("Legend & Help").info("? or Esc closes");
        let body = modal.body(area);
        let ov = region.overflow(body);
        let buf = f.buffer_mut();
        modal.foot(ov.foot().unwrap_or_default()).render(buf, area);
        region.render(buf, body);
        self.measured.set((ov.content, ov.visible));

        let status_row = area.y + area.height - 1;
        StatusBar::new(&[("?", "close help"), ("↑↓ PgDn", "scroll"), ("Esc", "close")]).right("help overlay").render(f.buffer_mut(), Rect::new(area.x, status_row, area.width, 1));
    }
}

/// A one-cell vertical rule between the columns, the full height of its area.
#[derive(Debug)]
struct Rule;

impl Component for Rule {
    fn height(&self, _width: u16) -> u16 {
        0
    }

    fn min_width(&self) -> u16 {
        1
    }

    fn render(&self, buf: &mut Buffer, area: Rect) {
        for y in area.top()..area.bottom() {
            buf.set_stringn(area.x, y, glyphs::V_LINE.to_string(), 1, theme::border());
        }
    }
}

fn glyph_span(g: char, color: Color) -> Span<'static> {
    Span::styled(format!(" {g} "), Style::default().fg(color).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD))
}

/// One note per `map::legend()` entry, in its order.
const TERRAIN_NOTES: [&str; 14] = [
    "impassable, drinkable",
    "drinkable, slow",
    "no forage",
    "regrows here first",
    "thin forage",
    "hare & deer grazing",
    "rich grazing, cover",
    "cover for small prey",
    "impassable heights",
    "reeds, standing water",
    "river over rock",
    "shelter, litters",
    "scavenger food",
    "regrowing this season",
];

const EVENTS: [(EventKind, &str); 11] = [
    (EventKind::Birth, "birth / litter"),
    (EventKind::DeathPredation, "death by predation"),
    (EventKind::DeathStarved, "death by starvation / thirst"),
    (EventKind::DeathAge, "death of old age"),
    (EventKind::Mutation, "mutation in a newborn"),
    (EventKind::Migration, "migration between regions"),
    (EventKind::Extinction, "species extinct"),
    (EventKind::Drought, "drought / scarcity warning"),
    (EventKind::Outbreak, "outbreak / epidemic / death by disease"),
    (EventKind::Recovery, "recovery (now immune)"),
    (EventKind::Wary, "prey giving predators room"),
];

const MARKS: [(char, Color, &str); 7] = [
    (glyphs::CURSOR, theme::CURSOR_BG, "look cursor (inverted cell)"),
    (glyphs::CORNER, theme::CURSOR_BG, "cursor corner marks"),
    (glyphs::TRAIL, theme::TRAIL, "trail of the followed creature"),
    (glyphs::DIAMOND, theme::ACCENT, "its current target"),
    (glyphs::RING, theme::ACCENT, "sense-range ring edge"),
    (glyphs::ALERT, theme::BAD, "danger: predator nearby"),
    (glyphs::PARASITE, theme::WARN, "parasites (cell tint)"),
];

/// Terrain with notes, the event glyphs and the map marks.
fn terrain_column() -> Rows<'static> {
    let events: Vec<(char, Color, &'static str)> = EVENTS.iter().map(|(kind, label)| (kind.glyph(), kind.color(), *label)).collect();
    vec![
        Box::new(Divider::new("Terrain")),
        Box::new(Legend::map().columns(1).label_w(14).notes(&TERRAIN_NOTES)),
        Box::new(Spacer::rows(1)),
        Box::new(Divider::new("Events")),
        Box::new(Legend::new(&events).columns(1).label_w(40).help()),
        Box::new(Spacer::rows(1)),
        Box::new(Divider::new("Map marks")),
        Box::new(Legend::new(&MARKS).columns(1).label_w(40).help()),
    ]
}

const SEASON_NOTES: [&str; 4] = ["births peak, regrowth fast", "water cells shrink", "forage peaks then falls", "regrowth halved, ice"];

/// The creatures table and the seasons.
fn creature_column(roster: &Roster) -> Rows<'static> {
    let seasons: Vec<(char, Color, &'static str)> = Season::ALL.iter().map(|s| (s.glyph(), s.color(), s.name())).collect();
    vec![
        Box::new(Divider::new("Creatures")),
        Box::new(Legend::creatures(roster)),
        Box::new(Spacer::rows(1)),
        Box::new(Divider::new("Seasons & time")),
        Box::new(Legend::new(&seasons).columns(1).label_w(8).notes(&SEASON_NOTES)),
        Box::new(Text::spans(vec![
            glyph_span(glyphs::SUN, theme::ACCENT),
            Span::styled("day     ", theme::text()),
            glyph_span(glyphs::MOON, theme::INFO),
            Span::styled("night (blue-shifted)", theme::dim_text()),
        ])),
    ]
}

const KEY_GROUPS: [(&str, &[(&str, &str)]); 5] = [
    ("Navigation", &[("← → ↑ ↓", "scroll the map"), ("Tab", "toggle the sidebar"), ("c", "centre (while following)")]),
    (
        "Look mode",
        &[
            ("k", "enter look mode"),
            ("← → ↑ ↓", "move the cursor"),
            ("Enter", "inspect cell / creature"),
            ("f", "follow creature"),
            ("z", "local zoom view"),
            ("Esc", "leave look mode"),
        ],
    ),
    (
        "Overlays",
        &[
            ("o", "cycle overlays"),
            ("1 2 3", "veg · pressure · moisture"),
            ("4 5 6", "sense · regions · species"),
            ("7", "health (weakest vital)"),
            ("8 9", "disease · parasites"),
            ("Tab", "next predator / species"),
            ("Shift+Tab", "previous species"),
            ("Esc", "clear overlay"),
        ],
    ),
    ("Speed", &[("Space", "pause / resume"), ("+ / -", "faster / slower"), (".", "step one tick"), ("p", "controls panel")]),
    (
        "Screens",
        &[
            ("s", "species & traits"),
            ("g", "graphs & charts"),
            ("y", "ecology & regions"),
            ("e", "event log"),
            ("l", "lineage tree"),
            ("F5", "save world"),
            ("F9", "quick-load"),
            ("?", "this help"),
            ("q", "quit to title"),
            ("w", "world generation"),
        ],
    ),
];

/// The key groups.
fn keys_column() -> Rows<'static> {
    let mut rows = Rows::new();
    for (title, keys) in KEY_GROUPS {
        rows.push(Box::new(Divider::new(title)));
        for (k, label) in keys {
            rows.push(Box::new(Text::spans(vec![Span::styled(format!(" {k:<10}"), theme::key()), Span::styled(*label, theme::text())])));
        }
    }
    rows
}
