//! S11: legend & help overlay drawn over the dimmed world map.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use super::s01_map;
use super::Prototype;
use crate::fixtures::{EventKind, Kind, Season, SpeciesId};
use crate::widgets::map;
use crate::widgets::{panel, status, util};
use crate::{glyphs, theme};

pub struct Help;

pub fn all() -> Vec<Box<dyn Prototype>> {
    vec![Box::new(Help)]
}

impl Prototype for Help {
    fn id(&self) -> &'static str {
        "S11a"
    }
    fn name(&self) -> &'static str {
        "Legend & Help"
    }
    fn variant(&self) -> &'static str {
        "overlay over world map"
    }

    fn render(&self, f: &mut Frame, area: Rect) {
        s01_map::render_base(f, area);
        util::dim_area(f.buffer_mut(), area, 0.55);

        let modal = util::centered(area, 120, 38);
        let inner = panel::draw_with_hint(f, modal, "Legend & Help", "? or Esc closes", panel::Kind::Focus);

        // Three columns separated by single vertical rules.
        let col_w = (inner.width - 2) / 3; // 38
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

        self.terrain_column(f, cols[0]);
        self.creature_column(f, cols[1]);
        self.keys_column(f, cols[2]);

        let status_row = area.y + area.height - 1;
        util::fill(f.buffer_mut(), Rect::new(area.x, status_row, area.width, 1), Style::default().bg(theme::STATUS_BG));
        let keys: &[(&str, &str)] = &[("?", "close help"), ("Esc", "close"), ("PgUp/PgDn", "scroll"), ("k", "look"), ("o", "overlay"), ("s", "species")];
        status::render(f, Rect::new(area.x, status_row, area.width, 1), keys, "help overlay");
    }
}

fn glyph_span(g: char, color: Color) -> Span<'static> {
    Span::styled(format!(" {} ", g), Style::default().fg(color).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD))
}

impl Help {
    fn terrain_column(&self, f: &mut Frame, col: Rect) {
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
        let events: [(EventKind, &str); 8] = [
            (EventKind::Birth, "birth / litter"),
            (EventKind::DeathPredation, "death by predation"),
            (EventKind::DeathStarved, "death by starvation / thirst"),
            (EventKind::DeathAge, "death of old age"),
            (EventKind::Mutation, "mutation in a newborn"),
            (EventKind::Migration, "migration between regions"),
            (EventKind::Extinction, "species extinct"),
            (EventKind::Drought, "drought / scarcity warning"),
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
        let marks: [(char, Color, &str); 6] = [
            (glyphs::CURSOR, theme::CURSOR_BG, "look cursor (inverted cell)"),
            (glyphs::CORNER, theme::CURSOR_BG, "cursor corner marks"),
            (glyphs::TRAIL, theme::TRAIL, "trail of the followed creature"),
            (glyphs::DIAMOND, theme::ACCENT, "its current target"),
            (glyphs::RING, theme::ACCENT, "sense-range ring edge"),
            (glyphs::ALERT, theme::BAD, "danger: predator nearby"),
        ];
        for (g, color, label) in marks {
            util::line(f, col, row, Line::from(vec![
                glyph_span(g, color),
                Span::styled(label, theme::text()),
            ]));
            row += 1;
        }
        row += 1;
        util::line(f, col, row, Line::from(Span::styled(" Heatmap shades: ░ <25%  ▒ <50%", theme::dim_text())));
        row += 1;
        util::line(f, col, row, Line::from(Span::styled("                 ▓ <75%  █ full", theme::dim_text())));
    }

    fn creature_column(&self, f: &mut Frame, col: Rect) {
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
        row += 1;
        util::line(f, col, row, Line::from(vec![
            Span::styled(" bright ", theme::dim_text()),
            Span::styled("W", Style::default().fg(theme::TEXT_BRIGHT).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            Span::styled(" selected   ", theme::dim_text()),
            Span::styled("H", Style::default().fg(theme::CURSOR_FG).bg(theme::ACCENT).add_modifier(Modifier::BOLD)),
            Span::styled(" followed", theme::dim_text()),
        ]));
        row += 2;

        panel::section(f, col, row, "Tags");
        row += 1;
        util::line(f, col, row, Line::from(vec![
            Span::styled(" h#217 ", theme::text()),
            Span::styled("= species letter + id", theme::dim_text()),
        ]));
        row += 1;
        util::line(f, col, row, Line::from(vec![
            Span::styled(format!(" {} {} ", glyphs::MALE, glyphs::FEMALE), theme::text()),
            Span::styled("sex      ", theme::dim_text()),
            Span::styled("gen 23 ", theme::text()),
            Span::styled("generation", theme::dim_text()),
        ]));
        row += 2;

        panel::section(f, col, row, "Seasons & time");
        row += 1;
        for s in [Season::Spring, Season::Summer, Season::Autumn, Season::Winter] {
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
        row += 1;
        util::line(f, col, row, Line::from(vec![
            Span::styled(format!(" {}", glyphs::PLAY), Style::default().fg(theme::GOOD).bg(theme::PANEL_BG)),
            Span::styled(" running  ", theme::text()),
            Span::styled(glyphs::PAUSE_STR, Style::default().fg(theme::WARN).bg(theme::PANEL_BG)),
            Span::styled(" paused  ", theme::text()),
            Span::styled(glyphs::FAST_STR, Style::default().fg(theme::GOOD).bg(theme::PANEL_BG)),
            Span::styled(" x2 speed", theme::text()),
        ]));
        row += 2;

        panel::section(f, col, row, "Vitals & trends");
        row += 1;
        for (color, label) in [(theme::GOOD, "healthy / rising"), (theme::WARN, "strained"), (theme::BAD, "critical / falling")] {
            util::line(f, col, row, Line::from(vec![
                Span::styled(format!(" {}{}{} ", glyphs::BAR_FILL, glyphs::BAR_FILL, glyphs::BAR_FILL), Style::default().fg(color).bg(theme::PANEL_BG)),
                Span::styled(label, theme::text()),
            ]));
            row += 1;
        }
        util::line(f, col, row, Line::from(vec![
            Span::styled(format!(" {} {} {} ", glyphs::UP, glyphs::FLAT, glyphs::DOWN), theme::text()),
            Span::styled("30-day population trend", theme::dim_text()),
        ]));
        row += 1;
        util::line(f, col, row, Line::from(vec![
            Span::styled(format!(" {} ", glyphs::MUTATION), Style::default().fg(theme::INFO).bg(theme::PANEL_BG)),
            Span::styled("trait differs from species mean", theme::dim_text()),
        ]));
    }

    fn keys_column(&self, f: &mut Frame, col: Rect) {
        let mut row = 0u16;
        let groups: [(&str, &[(&str, &str)]); 5] = [
            ("Navigation", &[
                ("← → ↑ ↓", "scroll the map"),
                ("PgUp PgDn", "scroll a page"),
                ("Home", "center on the valley"),
                ("Tab", "next notable creature"),
                ("c", "center on selection"),
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
                ("1", "vegetation density"),
                ("2", "population pressure"),
                ("3", "water & moisture"),
                ("4", "sense range"),
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
                ("w", "world generation"),
                ("?", "this help"),
                ("q", "quit"),
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
}
