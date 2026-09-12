//! S11: legend & help overlay drawn over the dimmed world map.

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
use crate::widgets::{panel, status, util};
use crate::{glyphs, theme};

pub struct Help;

impl Default for Help {
    fn default() -> Self {
        Self::new()
    }
}

impl Help {
    pub fn new() -> Self {
        Help
    }
}

impl Screen for Help {
    fn opaque(&self) -> bool {
        false
    }

    fn handle_key(&mut self, key: KeyEvent, _app: &mut AppState) -> Action {
        match key.code {
            KeyCode::Char('?') | KeyCode::Esc => Action::Pop,
            _ => Action::Unhandled,
        }
    }

    fn render(&self, _app: &AppState, f: &mut Frame, area: Rect) {
        let modal = util::centered(area, 120.min(area.width.saturating_sub(2)), 38.min(area.height.saturating_sub(2)));
        let inner = panel::draw_with_hint(f, modal, "Legend & Help", "? or Esc closes", panel::Kind::Focus);

        let col_w = (inner.width - 2) / 3;
        let cols = [
            Rect::new(inner.x, inner.y, col_w, inner.height),
            Rect::new(inner.x + col_w + 1, inner.y, col_w, inner.height),
            Rect::new(inner.x + 2 * col_w + 2, inner.y, inner.width - 2 * col_w - 2, inner.height),
        ];
        {
            let buf = f.buffer_mut();
            for cx in [inner.x + col_w, inner.x + 2 * col_w + 1] {
                for y in inner.top()..inner.bottom() {
                    buf.set_stringn(cx, y, glyphs::V_LINE.to_string(), 1, theme::border());
                }
            }
        }

        terrain_column(f, cols[0]);
        creature_column(f, cols[1]);
        keys_column(f, cols[2]);

        let status_row = area.y + area.height - 1;
        util::fill(f.buffer_mut(), Rect::new(area.x, status_row, area.width, 1), Style::default().bg(theme::STATUS_BG));
        status::render(f, Rect::new(area.x, status_row, area.width, 1), &[("?", "close help"), ("Esc", "close")], "help overlay");
    }
}

fn glyph_span(g: char, color: Color) -> Span<'static> {
    Span::styled(format!(" {} ", g), Style::default().fg(color).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD))
}

fn terrain_column(f: &mut Frame, col: Rect) {
    let mut row = 0u16;
    panel::section(f, col, row, "Terrain");
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
        util::line(f, col, row, Line::from(vec![
            glyph_span(g, color),
            Span::styled(format!("{:<14}", label), theme::text()),
            Span::styled(note, theme::dim_text()),
        ]));
        row += 1;
    }
    row += 1;

    panel::section(f, col, row, "Events");
    row += 1;
    let events: [(EventKind, &str); 10] = [
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
    ];
    for (kind, label) in events {
        util::line(f, col, row, Line::from(vec![
            glyph_span(kind.glyph(), kind.color()),
            Span::styled(label, theme::text()),
        ]));
        row += 1;
    }
    row += 1;

    panel::section(f, col, row, "Map marks");
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
        util::line(f, col, row, Line::from(vec![glyph_span(g, color), Span::styled(label, theme::text())]));
        row += 1;
    }
}

fn creature_column(f: &mut Frame, col: Rect) {
    let mut row = 0u16;
    panel::section(f, col, row, "Creatures");
    row += 1;
    util::line(f, col, row, Line::from(Span::styled(" ad jv  species role      diet", theme::label())));
    row += 1;
    for id in SpeciesId::ALL {
        let kind = match id.kind() {
            Kind::Prey => ("prey", theme::GOOD),
            Kind::Predator => ("predator", theme::BAD),
        };
        util::line(f, col, row, Line::from(vec![
            Span::styled(format!(" {}  {}  ", id.glyph().to_ascii_uppercase(), id.glyph()), Style::default().fg(id.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            Span::styled(format!("{:<7}", id.name()), theme::text()),
            Span::styled(format!("{:<10}", kind.0), Style::default().fg(kind.1).bg(theme::PANEL_BG)),
            Span::styled(id.diet(), theme::dim_text()),
        ]));
        row += 1;
    }
    util::line(f, col, row, Line::from(Span::styled(" UPPER adult   lower juvenile", theme::dim_text())));
    row += 2;

    panel::section(f, col, row, "Seasons & time");
    row += 1;
    for s in Season::ALL {
        let note = match s {
            Season::Spring => "births peak, regrowth fast",
            Season::Summer => "water cells shrink",
            Season::Autumn => "forage peaks then falls",
            Season::Winter => "regrowth halved, ice",
        };
        util::line(f, col, row, Line::from(vec![
            glyph_span(s.glyph(), s.color()),
            Span::styled(format!("{:<8}", s.name()), theme::text()),
            Span::styled(note, theme::dim_text()),
        ]));
        row += 1;
    }
    util::line(f, col, row, Line::from(vec![
        glyph_span(glyphs::SUN, theme::ACCENT),
        Span::styled("day     ", theme::text()),
        glyph_span(glyphs::MOON, theme::INFO),
        Span::styled("night (blue-shifted)", theme::dim_text()),
    ]));
}

fn keys_column(f: &mut Frame, col: Rect) {
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
        panel::section(f, col, row, title);
        row += 1;
        for (k, label) in keys.iter() {
            util::line(f, col, row, Line::from(vec![
                Span::styled(format!(" {:<10}", k), theme::key()),
                Span::styled(*label, theme::text()),
            ]));
            row += 1;
        }
    }
}
