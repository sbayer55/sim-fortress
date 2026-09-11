//! S00: title / main menu (C6 FR3). Renders from the newest save's header; the
//! in-memory world (if any) is kept for the dirty/quit checks.

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use crate::sim::save::{self, SaveHeader};
use crate::sim::{Season, SpeciesId};
use crate::ui::app::{AppState, ConfirmRequest, ConfirmYes};
use crate::ui::screens::confirm::ConfirmModal;
use crate::ui::screens::common::clip;
use crate::ui::screens::load_world::LoadWorld;
use crate::ui::screens::s09_worldgen::WorldGen;
use crate::ui::screens::s10_controls::Controls;
use crate::ui::screens::{Action, Screen};
use crate::ui::style::{SeasonStyle, SpeciesStyle};
use crate::widgets::map;
use crate::widgets::{panel, status, util};
use crate::{glyphs, theme};

const MENU: [&str; 4] = ["New World", "Load World", "Options", "Quit"];

pub struct Title {
    pub selection: usize,
}

impl Title {
    pub fn new() -> Self {
        Title { selection: 0 }
    }

    fn move_sel(&mut self, dir: i32, saves: bool) {
        let mut i = (self.selection as i32 + dir).rem_euclid(MENU.len() as i32) as usize;
        let mut guard = 0;
        while i == 1 && !saves && guard < MENU.len() {
            i = (i as i32 + dir).rem_euclid(MENU.len() as i32) as usize;
            guard += 1;
        }
        self.selection = i;
    }
}

impl Screen for Title {
    fn opaque(&self) -> bool {
        true
    }

    fn handle_key(&mut self, key: KeyEvent, app: &mut AppState) -> Action {
        let saves = !save::list_saves(&app.saves_dir).is_empty();
        match key.code {
            KeyCode::Up => {
                self.move_sel(-1, saves);
                Action::None
            }
            KeyCode::Down => {
                self.move_sel(1, saves);
                Action::None
            }
            KeyCode::Enter => match self.selection {
                0 => Action::Push(Box::new(WorldGen::from_params(app.params.clone()))),
                1 if saves => Action::Push(Box::new(LoadWorld::new())),
                2 => Action::Push(Box::new(Controls::new())),
                3 => self.quit(app),
                _ => Action::None,
            },
            KeyCode::Char('q') | KeyCode::Char('w') => self.quit(app),
            _ => Action::Unhandled,
        }
    }

    fn render(&self, app: &AppState, f: &mut Frame, area: Rect) {
        let status_row = area.y + area.height - 1;
        util::fill(f.buffer_mut(), area, Style::default().bg(theme::BG));
        let bg = |st: Style| st.bg(theme::BG);

        let saves = save::list_saves(&app.saves_dir);
        let newest = saves.first().map(|e| e.header.clone());

        // ---- big title ------------------------------------------------------
        let word = "SIM FORTRESS";
        let title_w = (word.chars().count() * 7 - 2) as u16;
        let tx = area.x + area.width.saturating_sub(title_w) / 2;
        let ty = area.y + 3;
        {
            let buf = f.buffer_mut();
            for row in 0..5 {
                let color = theme::lerp(theme::TITLE, theme::ACCENT, row as f32 / 4.0);
                let mut x = tx;
                for ch in word.chars() {
                    let s = letter(ch)[row];
                    buf.set_stringn(x, ty + row as u16, s, 5, Style::default().fg(color).bg(theme::BG).add_modifier(Modifier::BOLD));
                    x += 7;
                }
            }
            let shadow: String = std::iter::repeat_n(glyphs::SHADE_1, title_w as usize).collect();
            buf.set_stringn(tx, ty + 5, &shadow, title_w as usize, Style::default().fg(theme::dim(theme::ACCENT, 0.6)).bg(theme::BG));
        }

        // ---- tagline --------------------------------------------------------
        let tagline = format!("predator {} prey {} evolution {} scarcity", glyphs::DOT, glyphs::DOT, glyphs::DOT);
        center(f, area, 10, Line::from(Span::styled(tagline, bg(theme::dim_text()).add_modifier(Modifier::ITALIC))));

        // ---- terrain strips (from the newest save header) --------------------
        let strip_w = 120u16;
        let sx = area.x + area.width.saturating_sub(strip_w) / 2;
        if let Some(h) = &newest {
            strip(f, sx, area.y + 12, strip_w, &h.strip_rows[0..2]);
            strip(f, sx, area.y + 36, strip_w, &h.strip_rows[2..4]);
        }

        // ---- menu box -------------------------------------------------------
        let box_w = 34u16;
        let box_h = 8u16;
        let bx = area.x + area.width.saturating_sub(box_w) / 2;
        let by = area.y + 16;
        let inner = panel::draw(f, Rect::new(bx, by, box_w, box_h), "Main Menu", panel::Kind::Focus);
        for (i, entry) in MENU.iter().enumerate() {
            let row = 1 + i as u16;
            let disabled = i == 1 && newest.is_none();
            let selected = i == self.selection && !disabled;
            let text = if selected {
                format!("   {} {:<24}", glyphs::PLAY, entry)
            } else if disabled {
                format!("     {:<24} (empty)", entry)
            } else {
                format!("     {:<24}", entry)
            };
            let style = if disabled { theme::dim_text() } else if selected { theme::selected() } else { theme::text() };
            util::line(f, inner, row, Line::from(Span::styled(format!("{:<w$}", text, w = inner.width as usize), style)));
            if selected {
                f.buffer_mut().set_stringn(inner.right() - 8, inner.y + row, "[Enter]", 7, Style::default().fg(theme::KEY).bg(theme::SELECT_BG).add_modifier(Modifier::BOLD));
            }
        }

        // ---- last world summary ---------------------------------------------
        match &newest {
            Some(h) => {
                let (y, season, doy, hour) = clock_parts(h);
                let prey = h.counts[0] + h.counts[1] + h.counts[2];
                let pred = h.counts[3] + h.counts[4] + h.counts[5];
                center(
                    f,
                    area,
                    26,
                    Line::from(vec![
                        Span::styled("last world: ", bg(theme::dim_text())),
                        Span::styled(clip(&h.world_name, 28), bg(theme::title())),
                        Span::styled(format!("  {} Year {}", glyphs::DOT, y), bg(theme::text())),
                        Span::styled(format!("  {} {} prey / {} predators", glyphs::DOT, prey, pred), bg(theme::text())),
                        Span::styled(format!("  {} seed {:#x}", glyphs::DOT, h.seed), bg(theme::dim_text())),
                    ]),
                );
                center(
                    f,
                    area,
                    27,
                    Line::from(vec![Span::styled(
                        format!("autosaved Year {}, Day {} of {}  {:02}:00  {} {}", y, doy, season.name(), hour, season.glyph(), season.name()),
                        bg(theme::dim_text()),
                    )]),
                );
                // species roll-call
                let mut spans = vec![Span::styled("inhabitants  ", bg(theme::dim_text()))];
                for (i, id) in SpeciesId::ALL.iter().enumerate() {
                    let count = h.counts[i];
                    let count_style = if count > 0 { bg(theme::text()) } else { bg(theme::dim_text()) };
                    spans.push(Span::styled(id.glyph().to_ascii_uppercase().to_string(), Style::default().fg(id.color()).bg(theme::BG).add_modifier(Modifier::BOLD)));
                    spans.push(Span::styled(format!(" {} {:<4}  ", id.plural(), count), count_style));
                }
                center(f, area, 30, Line::from(spans));
            }
            None => {
                center(f, area, 26, Line::from(Span::styled("no saved worlds yet", bg(theme::dim_text()))));
            }
        }

        // ---- footer ---------------------------------------------------------
        center(
            f,
            area,
            33,
            Line::from(vec![
                Span::styled("Sim Fortress ", bg(theme::text())),
                Span::styled(format!("v{}", env!("CARGO_PKG_VERSION")), bg(theme::label())),
                Span::styled(format!("  {}  ratatui {} truecolor {} CP437", glyphs::DOT, glyphs::DOT, glyphs::DOT), bg(theme::dim_text())),
            ]),
        );
        center(
            f,
            area,
            40,
            Line::from(Span::styled(
                format!("{} every creature you see is born, hunts, breeds and dies by the numbers in its genome {}", glyphs::DIAMOND, glyphs::DIAMOND),
                bg(theme::dim_text()),
            )),
        );

        // ---- status bar -----------------------------------------------------
        let right = if app.sim.is_some() { "world loaded" } else { "no world loaded" };
        status::render(
            f,
            Rect::new(area.x, status_row, area.width, 1),
            &[("↑↓", "select"), ("Enter", "confirm"), ("q", "quit")],
            right,
        );
    }
}

impl Title {
    fn quit(&self, app: &mut AppState) -> Action {
        if app.sim.is_some() && app.dirty() {
            app.confirm = Some(ConfirmRequest { question: "World has unsaved changes. Quit anyway?".into(), yes: ConfirmYes::QuitApp });
            Action::Push(Box::new(ConfirmModal::new()))
        } else {
            Action::Quit
        }
    }
}

/// `(year, season, day_of_season, hour)` derived from a header (tick = hours).
fn clock_parts(h: &SaveHeader) -> (u32, Season, u32, u32) {
    let day_index = (h.tick + h.start_hour as u64) / 24;
    let season_days = h.season_days.max(1) as u64;
    let year = (day_index / (4 * season_days)) as u32 + 1;
    let doy = (day_index % (4 * season_days)) as u32 + 1;
    let season = match (day_index / season_days) % 4 {
        0 => Season::Spring,
        1 => Season::Summer,
        2 => Season::Autumn,
        _ => Season::Winter,
    };
    let hour = ((h.tick + h.start_hour as u64) % 24) as u32;
    (year, season, doy, hour)
}

fn letter(c: char) -> [&'static str; 5] {
    match c {
        'S' => ["▄████", "█    ", "▀███▄", "    █", "████▀"],
        'I' => ["█████", "  █  ", "  █  ", "  █  ", "█████"],
        'M' => ["█▄ ▄█", "█▀█▀█", "█ ▀ █", "█   █", "█   █"],
        'F' => ["█████", "█    ", "████ ", "█    ", "█    "],
        'O' => ["▄███▄", "█   █", "█   █", "█   █", "▀███▀"],
        'R' => ["████▄", "█   █", "████▀", "█  █ ", "█   █"],
        'T' => ["█████", "  █  ", "  █  ", "  █  ", "  █  "],
        'E' => ["█████", "█    ", "████ ", "█    ", "█████"],
        _ => ["     ", "     ", "     ", "     ", "     "],
    }
}

fn center(f: &mut Frame, area: Rect, row: u16, line: Line) {
    let w = line.width() as u16;
    let x = area.x + area.width.saturating_sub(w) / 2;
    util::line(f, Rect::new(x, area.y, w.min(area.width), area.height), row, line);
}

/// A band of terrain glyphs from serialised codes, dimmed and faded at the edges.
fn strip(f: &mut Frame, x: u16, y: u16, w: u16, rows: &[Vec<u8>]) {
    let buf = f.buffer_mut();
    for (i, row) in rows.iter().enumerate() {
        for sx in 0..w as usize {
            let code = row[sx.min(row.len() - 1)];
            let (g, fg, bgc) = map::terrain_code_cell(code);
            let edge = (sx.min(w as usize - 1 - sx) as f32 / 12.0).min(1.0);
            let fg = theme::lerp(theme::BG, fg, edge);
            let bgc = theme::lerp(theme::BG, theme::dim(bgc, 0.35), edge);
            if let Some(c) = buf.cell_mut((x + sx as u16, y + i as u16)) {
                c.set_char(g);
                c.set_style(Style::default().fg(fg).bg(bgc));
            }
        }
    }
}
