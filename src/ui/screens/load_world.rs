//! Load World list (C6 FR3): a 70×20 modal over S00 listing saves newest first.

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::Frame;

use crate::sim::save::{self, SaveHeader};
use crate::ui::app::{AppState, ConfirmRequest, ConfirmYes};
use crate::ui::config;
use crate::ui::screens::common::clip;
use crate::ui::screens::confirm::ConfirmModal;
use crate::ui::screens::{Action, Screen};
use crate::widgets::{panel, util};
use crate::theme;

const VISIBLE: usize = 14;

pub struct LoadWorld {
    pub sel: usize,
    pub scroll: usize,
}

impl LoadWorld {
    pub fn new() -> Self {
        LoadWorld { sel: 0, scroll: 0 }
    }

    fn clamp_scroll(&mut self, len: usize) {
        if self.sel < self.scroll {
            self.scroll = self.sel;
        }
        if self.sel >= self.scroll + VISIBLE {
            self.scroll = self.sel + 1 - VISIBLE;
        }
        if self.scroll + VISIBLE > len {
            self.scroll = len.saturating_sub(VISIBLE);
        }
    }
}

impl Screen for LoadWorld {
    fn opaque(&self) -> bool {
        false
    }

    fn handle_key(&mut self, key: KeyEvent, app: &mut AppState) -> Action {
        let saves = save::list_saves(&app.saves_dir);
        match key.code {
            KeyCode::Esc => Action::Pop,
            KeyCode::Up => {
                if !saves.is_empty() {
                    self.sel = self.sel.saturating_sub(1);
                    self.clamp_scroll(saves.len());
                }
                Action::None
            }
            KeyCode::Down => {
                if !saves.is_empty() {
                    self.sel = (self.sel + 1).min(saves.len() - 1);
                    self.clamp_scroll(saves.len());
                }
                Action::None
            }
            KeyCode::Enter => {
                let Some(entry) = saves.get(self.sel) else {
                    return Action::None;
                };
                match save::load(&entry.path) {
                    Ok(loaded) => {
                        let name = loaded.header.world_name.clone();
                        let params = loaded.sim.params.clone();
                        let tick = loaded.sim.time.tick;
                        app.sim = Some(loaded.sim);
                        app.params = params;
                        if let Some(ui) = config::load_ui() {
                            app.params.ui = ui;
                        }
                        app.world_name = Some(name.clone());
                        app.last_saved_tick = Some(tick);
                        app.viewport_origin = (0, 0);
                        app.note_params_ignored();
                        Action::EnterWorld { name }
                    }
                    Err(e) => {
                        eprintln!("load error: {e}");
                        Action::None
                    }
                }
            }
            KeyCode::Delete => {
                if let Some(entry) = saves.get(self.sel) {
                    app.confirm = Some(ConfirmRequest {
                        question: format!("Delete save \"{}\"?", entry.header.world_name),
                        yes: ConfirmYes::DeleteSave(entry.path.clone()),
                    });
                    Action::Push(Box::new(ConfirmModal::new()))
                } else {
                    Action::None
                }
            }
            _ => Action::Unhandled,
        }
    }

    fn render(&self, app: &AppState, f: &mut Frame, area: Rect) {
        let saves = save::list_saves(&app.saves_dir);
        let modal = util::centered(area, 70.min(area.width.saturating_sub(2)), 20.min(area.height.saturating_sub(2)));
        let hint = if saves.is_empty() { "no saves".to_string() } else { format!("{} saves", saves.len()) };
        let inner = panel::draw_with_hint(f, modal, "Load World", &hint, panel::Kind::Focus);

        if saves.is_empty() {
            util::line(f, inner, 1, Line::from(Span::styled(" no saved worlds yet", theme::dim_text())));
        } else {
            for i in self.scroll..saves.len().min(self.scroll + VISIBLE) {
                let row = (i - self.scroll) as u16 + 1;
                if row >= inner.height {
                    break;
                }
                let e = &saves[i];
                let (y, d) = year_day(&e.header);
                let prey = e.header.counts[0] + e.header.counts[1] + e.header.counts[2];
                let pred = e.header.counts[3] + e.header.counts[4] + e.header.counts[5];
                let text = format!(
                    "{:<22}  Year {}, Day {:<3}  {:>3} prey / {:>3} predators  {}",
                    clip(&e.header.world_name, 22),
                    y,
                    d,
                    prey,
                    pred,
                    saved_at(e.header.saved_at_unix)
                );
                let st = if i == self.sel { theme::selected() } else { theme::text() };
                let y_pos = inner.y + row;
                if i == self.sel {
                    let buf = f.buffer_mut();
                    for x in inner.x..inner.right() {
                        if let Some(c) = buf.cell_mut((x, y_pos)) {
                            c.set_bg(theme::SELECT_BG);
                        }
                    }
                }
                util::line(f, inner, row, Line::from(Span::styled(text, st)));
            }
        }

        let status_row = area.y + area.height - 1;
        util::fill(f.buffer_mut(), Rect::new(area.x, status_row, area.width, 1), Style::default().bg(theme::STATUS_BG));
        crate::widgets::status::render(
            f,
            Rect::new(area.x, status_row, area.width, 1),
            &[("↑↓", "select"), ("Enter", "load"), ("Del", "delete"), ("Esc", "back")],
            "load world",
        );
    }
}

fn year_day(h: &SaveHeader) -> (u32, u32) {
    let tpd = 24u64;
    let day_index = (h.tick + h.start_hour as u64) / tpd;
    let year_len = 4 * h.season_days as u64;
    let year = (day_index / year_len) as u32 + 1;
    let day = (day_index % year_len) as u32 + 1;
    (year, day)
}

/// `YYYY-MM-DD HH:MM` from a unix timestamp (no chrono dependency).
fn saved_at(secs: u64) -> String {
    let (y, m, d, hh, mm) = civil(secs as i64);
    format!("{y}-{m:02}-{d:02} {hh:02}:{mm:02}")
}

fn civil(secs: i64) -> (i64, u32, u32, u32, u32) {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let hour = (rem / 3600) as u32;
    let minute = ((rem % 3600) / 60) as u32;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (mp + if mp < 10 { 3 } else { -9 }) as u32;
    let y = y + if m <= 2 { 1 } else { 0 };
    (y, m, d, hour, minute)
}
