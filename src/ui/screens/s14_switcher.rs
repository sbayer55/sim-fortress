//! S14: the overlay switcher, a modal over the map that composes the stack on
//! `AppState`. The map beneath is the live preview, so the backdrop is not
//! dimmed.

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::Frame;

use crate::ui::app::AppState;
use crate::ui::screens::s01_map::{subject_alive, WorldMap};
use crate::ui::screens::{Action, Screen};
use crate::widgets::map::Base;

#[derive(Debug, Default)]
pub struct OverlaySwitcher {}

impl OverlaySwitcher {
    pub const fn new() -> Self {
        Self {}
    }

    /// Open from the map: a sub-pick whose layer is off is refreshed by the
    /// S02f / S02d default rules so every row says what would come on.
    pub fn open(app: &mut AppState) -> Self {
        if app.overlay.base != Base::Species {
            app.overlay.species = WorldMap::default_species(app);
        }
        if !app.overlay.sense && !subject_alive(app, app.overlay.sense_subject) {
            app.overlay.sense_subject = WorldMap::default_sense(app);
        }
        Self::new()
    }
}

impl Screen for OverlaySwitcher {
    fn opaque(&self) -> bool {
        false
    }

    fn dims_backdrop(&self) -> bool {
        false
    }

    fn handle_key(&mut self, key: KeyEvent, app: &mut AppState) -> Action {
        match key.code {
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('o') => Action::Pop,
            KeyCode::Backspace => {
                app.overlay.clear();
                Action::None
            }
            _ => Action::Unhandled,
        }
    }

    fn render(&self, _app: &AppState, _f: &mut Frame<'_>, _area: Rect) {}
}
