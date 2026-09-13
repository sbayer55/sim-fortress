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
    /// The last load failure, shown in the modal (C8: an older-version save is a
    /// friendly message, never a panic or a console write behind the TUI).
    pub error: Option<String>,
}

impl LoadWorld {
    pub fn new() -> Self {
        LoadWorld { sel: 0, scroll: 0, error: None }
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
                self.error = None;
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
                        self.error = Some(e.to_string());
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
        // The per-row wall-clock stamp does not fit the 66-column list, so it lives
        // in the panel hint for the selected save instead.
        let hint = if saves.is_empty() {
            "no saves".to_string()
        } else {
            let when = saves.get(self.sel.min(saves.len() - 1)).map(|e| saved_at(e.header.saved_at_unix));
            match when {
                Some(w) => format!("{} saves  {}  saved {w}", saves.len(), crate::glyphs::DOT),
                None => format!("{} saves", saves.len()),
            }
        };
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
                // Keep the line inside the modal's 66 columns: the wall-clock stamp
                // never fitted, and the version marker has to be visible so an
                // unloadable old save is obvious before Enter is pressed.
                let mark = if e.version == save::VERSION { String::new() } else { format!("  v{}", e.version) };
                let text = format!(
                    "{:<22}  Year {}, Day {:<3}  {:>3} prey /{:>3} pred{}",
                    clip(&e.header.world_name, 22),
                    y,
                    d,
                    prey,
                    pred,
                    mark
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
        if let Some(err) = &self.error {
            // The message is longer than the 66-column modal, so wrap it rather
            // than clip: an older-version save must say why it will not load.
            let lines = wrap(err, inner.width as usize - 2);
            let shown = lines.len().min(2) as u16;
            let first = inner.height.saturating_sub(shown);
            for (i, line) in lines.iter().take(2).enumerate() {
                util::line(
                    f,
                    inner,
                    first + i as u16,
                    Line::from(Span::styled(format!(" {line}"), Style::default().fg(theme::BAD).bg(theme::PANEL_BG))),
                );
            }
        }
        util::fill(f.buffer_mut(), Rect::new(area.x, status_row, area.width, 1), Style::default().bg(theme::STATUS_BG));
        crate::widgets::status::render(
            f,
            Rect::new(area.x, status_row, area.width, 1),
            &[("↑↓", "select"), ("Enter", "load"), ("Del", "delete"), ("Esc", "back")],
            "load world",
        );
    }
}

/// Split `text` into lines of at most `width` cells, breaking on spaces.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in text.split_whitespace() {
        if !cur.is_empty() && cur.chars().count() + 1 + word.chars().count() > width {
            lines.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(word);
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::{Params, Sim};
    use crate::ui::app::AppState;
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::KeyModifiers;
    use ratatui::Terminal;

    fn key(c: KeyCode) -> KeyEvent {
        KeyEvent::new(c, KeyModifiers::NONE)
    }

    #[test]
    fn wrap_breaks_on_spaces_within_width() {
        let msg = "save is from an older version (1, now 3): the genome format changed, so it cannot be loaded";
        let lines = wrap(msg, 64);
        assert_eq!(lines.len(), 2);
        assert!(lines.iter().all(|l| l.chars().count() <= 64), "{lines:?}");
        assert_eq!(lines.join(" "), msg);
        // A word longer than the width is left intact rather than split.
        assert_eq!(wrap("abcdefghij", 4), vec!["abcdefghij".to_string()]);
    }

    #[test]
    fn older_version_error_is_shown_not_panicked() {
        let dir = std::env::temp_dir().join(format!("simf-load-old-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let sim = Sim::new(7, Params::default());
        let path = save::save(&sim, "Old World", &dir).unwrap();
        // Rewrite the version field to the pre-C8 format.
        let mut bytes = std::fs::read(&path).unwrap();
        bytes[4..6].copy_from_slice(&(save::VERSION - 1).to_le_bytes());
        std::fs::write(&path, &bytes).unwrap();

        let mut app = AppState::new(Params::default());
        app.saves_dir = dir;
        let mut screen = LoadWorld::new();
        let action = screen.handle_key(key(KeyCode::Enter), &mut app);
        assert!(matches!(action, Action::None), "an old save must not load");
        let err = screen.error.clone().unwrap_or_default();
        assert!(err.contains("older version"), "{err:?}");

        let backend = TestBackend::new(155, 45);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| screen.render(&app, f, Rect::new(0, 0, 155, 45))).unwrap();
        let buf = terminal.backend().buffer();
        let text: String = (0..45).map(|y| (0..155).map(|x| buf[(x, y)].symbol()).collect::<String>() + "\n").collect();
        assert!(text.contains("older version"), "the reason is on screen: {text}");
        assert!(text.contains("cannot be loaded"), "the message is not clipped: {text}");
        assert!(text.contains(&format!("v{}", save::VERSION - 1)), "the list flags the old format: {text}");
    }
}
