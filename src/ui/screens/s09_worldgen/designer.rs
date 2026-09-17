//! S09b: the species designer modal (C9, upstream). A sentence goes to the
//! model, the JSON reply becomes a `[[species]]` overlay, the ordinary loader
//! validates it (one correction round at most, R8), and on accept the overlay
//! is written to `<config dir>/species/<name>.toml` and handed to the form.

use std::cell::RefCell;

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use crate::ai::prompt::designer::{self as prompt, Overlay};
use crate::ai::{AiError, Feature, Output, Reply, RequestId};
use crate::sim::Params;
use crate::ui::app::AppState;
use crate::ui::screens::common::{clip, edit_text};
use crate::ui::screens::{Action, Screen};
use crate::widgets::{panel, util, Component, StatusBar};
use crate::{glyphs, theme};

const W: u16 = 78;
const H: u16 = 18;
const PROMPT_MAX: usize = 200;

#[derive(Debug)]
enum Phase {
    Typing,
    Waiting(RequestId),
    Preview { overlay: Overlay, lines: Vec<String> },
    Failed(String),
}

#[derive(Debug)]
struct State {
    base: Params,
    prompt: String,
    phase: Phase,
    retry_used: bool,
}

#[derive(Debug)]
pub(super) struct Designer {
    st: RefCell<State>,
}

impl Designer {
    /// `base` is what the form would generate with now; the overlay is merged
    /// onto it for validation and for the preview.
    pub(super) const fn new(base: Params) -> Self {
        Self { st: RefCell::new(State { base, prompt: String::new(), phase: Phase::Typing, retry_used: false }) }
    }

    fn send(st: &mut State, app: &AppState, previous: Option<(&str, &str)>) {
        let msgs = prompt::messages(&st.prompt, &st.base.species, previous);
        st.phase = match app.ai.request(Feature::Designer, msgs, Some(prompt::SCHEMA.to_string()), false) {
            Ok(id) => Phase::Waiting(id),
            Err(e) => Phase::Failed(e.to_string()),
        };
    }

    /// Route replies from the inbox into the phase machine.
    fn absorb(&self, app: &AppState) {
        let replies: Vec<Reply> = app.designer_inbox.borrow_mut().drain(..).collect();
        for r in replies {
            let mut st = self.st.borrow_mut();
            let Phase::Waiting(id) = st.phase else { continue };
            if r.id != id {
                continue;
            }
            match r.result {
                Ok(Output::Text(text)) => Self::on_text(&mut st, app, &text),
                Ok(Output::Chunk(_) | Output::Done) => {}
                Err(AiError::Timeout | AiError::Unreachable(_)) => st.phase = Phase::Failed("the gateway did not answer".to_string()),
                Err(e) => st.phase = Phase::Failed(e.to_string()),
            }
        }
    }

    fn on_text(st: &mut State, app: &AppState, text: &str) {
        let checked = prompt::overlay_from_reply(text).and_then(|o| prompt::validated(&st.base, &o.toml).map(|p| (o, p)));
        match checked {
            Ok((overlay, params)) => {
                let lines = prompt::summary(&params, &overlay.name);
                st.phase = Phase::Preview { overlay, lines };
            }
            Err(e) if !st.retry_used => {
                st.retry_used = true;
                Self::send(st, app, Some((text, &e)));
            }
            Err(e) => st.phase = Phase::Failed(e),
        }
    }

    /// Write the accepted overlay where the player can read and edit it (R4).
    fn write_file(prompt_text: &str, overlay: &Overlay) -> Result<std::path::PathBuf, String> {
        let dir = crate::ui::config::ui_config_path().parent().map(|p| p.join("species")).ok_or("no config dir")?;
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = dir.join(format!("{}.toml", overlay.name));
        let text = format!("# Written by the species designer from: {prompt_text:?}\n# Pass it back with --params, or edit it.\n{}", overlay.toml);
        std::fs::write(&path, text).map_err(|e| e.to_string())?;
        Ok(path)
    }
}

impl Screen for Designer {
    fn opaque(&self) -> bool {
        false
    }

    fn handle_key(&mut self, key: KeyEvent, app: &mut AppState) -> Action {
        self.absorb(app);
        let mut st = self.st.borrow_mut();
        match (&st.phase, key.code) {
            (Phase::Typing | Phase::Failed(_), KeyCode::Esc) => Action::Pop,
            (Phase::Typing, KeyCode::Enter) => {
                if !st.prompt.trim().is_empty() {
                    st.retry_used = false;
                    Self::send(&mut st, app, None);
                }
                Action::None
            }
            (Phase::Typing, code) => {
                edit_text(&mut st.prompt, code, PROMPT_MAX);
                Action::None
            }
            (Phase::Waiting(id), KeyCode::Esc) => {
                app.ai.cancel(*id);
                Action::Pop
            }
            (Phase::Preview { overlay, .. }, KeyCode::Enter) => {
                match Self::write_file(&st.prompt, overlay) {
                    Ok(path) => eprintln!("species overlay written to {}", path.display()),
                    Err(e) => eprintln!("species overlay not written: {e}"),
                }
                app.pending_species_overlay = Some(overlay.toml.clone());
                Action::Pop
            }
            (Phase::Preview { .. } | Phase::Failed(_), _) => {
                st.phase = Phase::Typing;
                Action::None
            }
            _ => Action::None,
        }
    }

    fn render(&self, app: &AppState, f: &mut Frame<'_>, area: Rect) {
        self.absorb(app);
        let st = self.st.borrow();
        let modal = util::centered(area, W.min(area.width.saturating_sub(2)), H.min(area.height.saturating_sub(2)));
        let inner = panel::draw_with_hint(f, modal, "Species designer", "Esc closes", panel::Kind::Focus);
        let width = crate::cast!(inner.width.saturating_sub(2) => usize);
        util::line(f, inner, 0, Line::from(Span::styled(" Describe one species in a sentence; the model writes its [[species]] entry.", theme::dim_text())));
        let caret = if matches!(st.phase, Phase::Typing) { "_" } else { "" };
        let shown: String = st.prompt.chars().rev().take(width.saturating_sub(4)).collect::<Vec<_>>().into_iter().rev().collect();
        util::line(f, inner, 2, Line::from(vec![
            Span::styled(" > ", theme::key()),
            Span::styled(format!("{shown}{caret}"), Style::default().fg(theme::TEXT_BRIGHT).bg(theme::PANEL_BG)),
        ]));
        let mut row = 4u16;
        let body: Vec<(String, Style)> = match &st.phase {
            Phase::Typing => vec![(" Examples: \"a boar: omnivore, big litters, forest dweller\"".to_string(), theme::dim_text())],
            Phase::Waiting(_) => vec![
                (format!(" {} asking {} ...", glyphs::DOT, app.ai.model_of(Feature::Designer).unwrap_or_default()), theme::text()),
                (if st.retry_used { " (one correction round after the loader rejected the first reply)".to_string() } else { String::new() }, theme::dim_text()),
            ],
            Phase::Preview { overlay, lines } => {
                let mut v = vec![(format!(" {} validated: {}", glyphs::HAPPY, overlay.name), Style::default().fg(theme::GOOD).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD))];
                v.extend(lines.iter().map(|l| (format!("   {}", clip(l, width.saturating_sub(3))), theme::text())));
                v
            }
            Phase::Failed(e) => vec![(format!(" {} {}", glyphs::UNHAPPY, clip(e, width.saturating_sub(3))), Style::default().fg(theme::WARN).bg(theme::PANEL_BG))],
        };
        for (text, style) in body {
            util::line(f, inner, row, Line::from(Span::styled(text, style)));
            row += 1;
        }
        let hint: &[(&str, &str)] = match st.phase {
            Phase::Typing => &[("Enter", "design"), ("Esc", "close")],
            Phase::Waiting(_) => &[("Esc", "cancel")],
            Phase::Preview { .. } => &[("Enter", "add to roster"), ("Esc", "try again")],
            Phase::Failed(_) => &[("Enter", "try again"), ("Esc", "close")],
        };
        let status_row = area.y + area.height - 1;
        util::fill(f.buffer_mut(), Rect::new(area.x, status_row, area.width, 1), Style::default().bg(theme::STATUS_BG));
        StatusBar::new(hint).right("species designer").note(app.ai.status_note()).render(f.buffer_mut(), Rect::new(area.x, status_row, area.width, 1));
    }
}
