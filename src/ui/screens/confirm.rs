//! Confirm modal (C6 FR4): 50×7, one question, `[ Yes ]  [ No ]`.

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use crate::ui::app::{AppState, ConfirmYes};
use crate::ui::screens::common::clip;
use crate::ui::screens::{Action, Screen};
use crate::widgets::{panel, util};
use crate::theme;

pub struct ConfirmModal {
    pub focus: usize,
}

impl ConfirmModal {
    pub fn new() -> Self {
        ConfirmModal { focus: 0 }
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
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                self.focus = 0;
                self.activate(app)
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                app.confirm = None;
                Action::Pop
            }
            _ => Action::Unhandled,
        }
    }

    fn render(&self, app: &AppState, f: &mut Frame, area: Rect) {
        let modal = util::centered(area, 50.min(area.width.saturating_sub(2)), 7.min(area.height.saturating_sub(2)));
        let inner = panel::draw(f, modal, "", panel::Kind::Focus);
        let question = app.confirm.as_ref().map(|r| r.question.clone()).unwrap_or_default();

        util::line(f, inner, 1, Line::from(Span::styled(
            clip(&question, inner.width as usize),
            Style::default().fg(theme::TEXT_BRIGHT).bg(theme::PANEL_BG),
        )));

        let row = inner.height - 2;
        let buttons: [(&str, usize); 2] = [("[ Yes ]", 0), ("[ No ]", 1)];
        let mut spans = vec![Span::styled("  ", theme::text())];
        for (label, i) in buttons {
            let st = if self.focus == i {
                Style::default().fg(theme::CURSOR_FG).bg(theme::ACCENT).add_modifier(Modifier::BOLD)
            } else {
                theme::text()
            };
            spans.push(Span::styled(label, st));
            spans.push(Span::styled("    ", theme::text()));
        }
        spans.push(Span::styled("←→ move  Enter select  Esc no", theme::dim_text()));
        util::line(f, inner, row, Line::from(spans));
    }
}

impl ConfirmModal {
    fn activate(&mut self, app: &mut AppState) -> Action {
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
