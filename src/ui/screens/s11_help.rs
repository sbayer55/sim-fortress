//! S11: legend & help overlay drawn over the dimmed world map. The three
//! columns scroll together with `↑ ↓ PgUp PgDn Home End`.

use std::cell::Cell;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use crate::sim::{EventKind, Kind, Season, SpeciesId};
use crate::ui::app::AppState;
use crate::ui::screens::{Action, Screen};
use crate::ui::style::{EventKindStyle, SeasonStyle, SpeciesStyle};
use crate::widgets::map;
use crate::widgets::scroll::{self, Overflow};
use crate::widgets::{panel, status, util};
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

    fn render(&self, _app: &AppState, f: &mut Frame<'_>, area: Rect) {
        let modal = util::centered(area, 120.min(area.width.saturating_sub(2)), 38.min(area.height.saturating_sub(2)));
        let inner = panel::draw_with_hint(f, modal, "Legend & Help", "? or Esc closes", panel::Kind::Focus);

        let ov = scroll::draw(f, modal, inner, self.offset, columns);
        self.measured.set((ov.content, inner.height));

        let status_row = area.y + area.height - 1;
        util::fill(f.buffer_mut(), Rect::new(area.x, status_row, area.width, 1), Style::default().bg(theme::STATUS_BG));
        status::render(f, Rect::new(area.x, status_row, area.width, 1), &[("?", "close help"), ("↑↓ PgDn", "scroll"), ("Esc", "close")], "help overlay");
    }
}

/// The three columns with their dividers; returns the tallest column's rows.
fn columns(buf: &mut Buffer, canvas: Rect) -> u16 {
    let col_w = (canvas.width - 2).div_euclid(3);
    let cols = [
        Rect::new(canvas.x, canvas.y, col_w, canvas.height),
        Rect::new(canvas.x + col_w + 1, canvas.y, col_w, canvas.height),
        Rect::new(canvas.x + 2 * col_w + 2, canvas.y, canvas.width - 2 * col_w - 2, canvas.height),
    ];
    for cx in [canvas.x + col_w, canvas.x + 2 * col_w + 1] {
        for y in canvas.top()..canvas.bottom() {
            buf.set_stringn(cx, y, glyphs::V_LINE.to_string(), 1, theme::border());
        }
    }
    let a = terrain_column(buf, cols[0]);
    let b = creature_column(buf, cols[1]);
    let c = keys_column(buf, cols[2]);
    a.max(b).max(c)
}

fn glyph_span(g: char, color: Color) -> Span<'static> {
    Span::styled(format!(" {g} "), Style::default().fg(color).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD))
}

fn terrain_column(buf: &mut Buffer, col: Rect) -> u16 {
    let mut row = 0u16;
    panel::section_in(buf, col, row, "Terrain");
    row += 1;
    let notes: [&str; 12] = [
        "impassable, drinkable",
        "drinkable, slow",
        "no forage",
        "regrows here first",
        "thin forage",
        "hare & deer grazing",
        "rich grazing, cover",
        "cover for small prey",
        "impassable heights",
        "shelter, litters",
        "scavenger food",
        "regrowing this season",
    ];
    for ((g, color, label), note) in map::legend().into_iter().zip(notes) {
        util::line_in(buf, col, row, Line::from(vec![
            glyph_span(g, color),
            Span::styled(format!("{label:<14}"), theme::text()),
            Span::styled(note, theme::dim_text()),
        ]));
        row += 1;
    }
    row += 1;

    panel::section_in(buf, col, row, "Events");
    row += 1;
    let events: [(EventKind, &str); 11] = [
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
    for (kind, label) in events {
        util::line_in(buf, col, row, Line::from(vec![
            glyph_span(kind.glyph(), kind.color()),
            Span::styled(label, theme::text()),
        ]));
        row += 1;
    }
    row += 1;

    panel::section_in(buf, col, row, "Map marks");
    row += 1;
    let marks: [(char, Color, &str); 7] = [
        (glyphs::CURSOR, theme::CURSOR_BG, "look cursor (inverted cell)"),
        (glyphs::CORNER, theme::CURSOR_BG, "cursor corner marks"),
        (glyphs::TRAIL, theme::TRAIL, "trail of the followed creature"),
        (glyphs::DIAMOND, theme::ACCENT, "its current target"),
        (glyphs::RING, theme::ACCENT, "sense-range ring edge"),
        (glyphs::ALERT, theme::BAD, "danger: predator nearby"),
        (glyphs::PARASITE, theme::WARN, "parasites (cell tint)"),
    ];
    for (g, color, label) in marks {
        util::line_in(buf, col, row, Line::from(vec![glyph_span(g, color), Span::styled(label, theme::text())]));
        row += 1;
    }
    row
}

fn creature_column(buf: &mut Buffer, col: Rect) -> u16 {
    let mut row = 0u16;
    panel::section_in(buf, col, row, "Creatures");
    row += 1;
    util::line_in(buf, col, row, Line::from(Span::styled(" ad jv  species role      diet", theme::label())));
    row += 1;
    for id in SpeciesId::ALL {
        let kind = match id.kind() {
            Kind::Prey => ("prey", theme::GOOD),
            Kind::Predator => ("predator", theme::BAD),
        };
        util::line_in(buf, col, row, Line::from(vec![
            Span::styled(format!(" {}  {}  ", id.glyph().to_ascii_uppercase(), id.glyph()), Style::default().fg(id.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            Span::styled(format!("{:<7}", id.name()), theme::text()),
            Span::styled(format!("{:<10}", kind.0), Style::default().fg(kind.1).bg(theme::PANEL_BG)),
            Span::styled(id.diet(), theme::dim_text()),
        ]));
        row += 1;
    }
    util::line_in(buf, col, row, Line::from(Span::styled(" UPPER adult   lower juvenile", theme::dim_text())));
    row += 2;

    panel::section_in(buf, col, row, "Seasons & time");
    row += 1;
    for s in Season::ALL {
        let note = match s {
            Season::Spring => "births peak, regrowth fast",
            Season::Summer => "water cells shrink",
            Season::Autumn => "forage peaks then falls",
            Season::Winter => "regrowth halved, ice",
        };
        util::line_in(buf, col, row, Line::from(vec![
            glyph_span(s.glyph(), s.color()),
            Span::styled(format!("{:<8}", s.name()), theme::text()),
            Span::styled(note, theme::dim_text()),
        ]));
        row += 1;
    }
    util::line_in(buf, col, row, Line::from(vec![
        glyph_span(glyphs::SUN, theme::ACCENT),
        Span::styled("day     ", theme::text()),
        glyph_span(glyphs::MOON, theme::INFO),
        Span::styled("night (blue-shifted)", theme::dim_text()),
    ]));
    row + 1
}

fn keys_column(buf: &mut Buffer, col: Rect) -> u16 {
    let mut row = 0u16;
    let groups: [(&str, &[(&str, &str)]); 5] = [
        ("Navigation", &[
            ("← → ↑ ↓", "scroll the map"),
            ("Tab", "toggle the sidebar"),
            ("c", "centre (while following)"),
        ]),
        ("Look mode", &[
            ("k", "enter look mode"),
            ("← → ↑ ↓", "move the cursor"),
            ("Enter", "inspect cell / creature"),
            ("f", "follow creature"),
            ("z", "local zoom view"),
            ("Esc", "leave look mode"),
        ]),
        ("Overlays", &[
            ("o", "cycle overlays"),
            ("1 2 3", "veg · pressure · moisture"),
            ("4 5 6", "sense · regions · species"),
            ("7", "health (weakest vital)"),
            ("8 9", "disease · parasites"),
            ("Tab", "next predator / species"),
            ("Shift+Tab", "previous species"),
            ("Esc", "clear overlay"),
        ]),
        ("Speed", &[
            ("Space", "pause / resume"),
            ("+ / -", "faster / slower"),
            (".", "step one tick"),
            ("p", "controls panel"),
        ]),
        ("Screens", &[
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
        ]),
    ];
    for (title, keys) in groups {
        panel::section_in(buf, col, row, title);
        row += 1;
        for (k, label) in keys {
            util::line_in(buf, col, row, Line::from(vec![
                Span::styled(format!(" {k:<10}"), theme::key()),
                Span::styled(*label, theme::text()),
            ]));
            row += 1;
        }
    }
    row
}
