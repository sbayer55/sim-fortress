//! Confirm modal (C6 FR4): 50×7, one question, `[ Yes ]  [ No ]`.

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::Frame;

use crate::ui::app::{AppState, ConfirmYes};
use crate::ui::screens::common::clip;
use crate::ui::screens::{Action, Screen};
use crate::widgets::{status, Component, Modal, Text};
use crate::theme;

#[derive(Debug)]
pub struct ConfirmModal {
    pub focus: usize,
}

impl ConfirmModal {
    pub const fn new() -> Self {
        Self { focus: 0 }
    }
}

impl Default for ConfirmModal {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for ConfirmModal {
    fn opaque(&self) -> bool {
        false
    }

    fn handle_key(&mut self, key: KeyEvent, app: &mut AppState) -> Action {
        match key.code {
            KeyCode::Left | KeyCode::Right | KeyCode::Tab | KeyCode::BackTab => {
                self.focus = 1 - self.focus;
                Action::None
            }
            KeyCode::Enter => self.activate(app),
            KeyCode::Char('y' | 'Y') => {
                self.focus = 0;
                self.activate(app)
            }
            KeyCode::Char('n' | 'N') | KeyCode::Esc => {
                app.confirm = None;
                Action::Pop
            }
            _ => Action::Unhandled,
        }
    }

    fn render(&self, app: &AppState, f: &mut Frame<'_>, area: Rect) {
        let modal = Modal::new(50.min(area.width.saturating_sub(2)), 7.min(area.height.saturating_sub(2)))
            .buttons(&["[ Yes ]", "[ No ]"], Some(self.focus))
            .hint("←→ move  Enter select  Esc no");
        let question = app.confirm.as_ref().map(|r| r.question.clone()).unwrap_or_default();
        let buf = f.buffer_mut();
        let body = modal.render(buf, area);
        let text = Text::new(clip(&question, crate::cast!(body.width => usize))).style(Style::default().fg(theme::TEXT_BRIGHT).bg(theme::PANEL_BG));
        text.render(buf, Rect::new(body.x, body.y + 1, body.width, 1).intersection(body));

        // Repaint the status bar undimmed, as every other modal does.
        let status_row = area.y + area.height - 1;
        status::render(f, Rect::new(area.x, status_row, area.width, 1), &[("y", "yes"), ("n", "no"), ("←→", "move"), ("Enter", "select"), ("Esc", "no")], "confirm");
    }
}

impl ConfirmModal {
    fn activate(&self, app: &mut AppState) -> Action {
        let Some(req) = app.confirm.take() else {
            return Action::Pop;
        };
        if self.focus == 1 {
            return Action::Pop; // No
        }
        match req.yes {
            ConfirmYes::QuickLoad => {
                app.quick_load();
                match app.world_name.clone() {
                    Some(name) => Action::EnterWorld { name },
                    None => Action::Pop,
                }
            }
            ConfirmYes::DeleteSave(path) => {
                let _ = std::fs::remove_file(&path);
                Action::Pop
            }
            ConfirmYes::QuitApp => Action::Quit,
            ConfirmYes::ToTitle => Action::GoTitle,
        }
    }
}
