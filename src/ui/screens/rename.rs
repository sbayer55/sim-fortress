//! Rename modal: the player names an animal (S03 and S16 `n`) or a dynasty
//! (S16 `n` on a line). One text field; Enter keeps it, an empty name restores
//! the generated one, Esc leaves everything as it was.
//!
//! While it is up it consumes every key, so typing a name never pauses the
//! world or opens another screen.

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::Frame;

use crate::sim::naming::{ANIMAL_NAME_MAX, DYNASTY_NAME_MAX};
use crate::sim::{CreatureId, Sim};
use crate::theme;
use crate::ui::app::AppState;
use crate::ui::screens::common::{clip, edit_text};
use crate::ui::screens::s16_dynasties::founder_line;
use crate::ui::screens::{Action, Screen};
use crate::widgets::{Component, Modal, StatusBar, Text, TextField};

#[cfg(test)]
mod tests;

const W: u16 = 52;
const H: u16 = 9;

/// What is being named.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenameTarget {
    /// A creature, living or still in the lineage.
    Animal(CreatureId),
    /// A dynasty, by its root.
    Dynasty(CreatureId),
}

impl RenameTarget {
    const fn max(self) -> usize {
        match self {
            Self::Animal(_) => ANIMAL_NAME_MAX,
            Self::Dynasty(_) => DYNASTY_NAME_MAX,
        }
    }
}

#[derive(Debug)]
pub struct RenameModal {
    pub target: RenameTarget,
    /// The name being typed, seeded with the current player name.
    pub text: String,
    /// `Ash w#003 · wolf` or `Ash line · wolves`: what is being named.
    subject: String,
    /// The generated name an empty field restores.
    default: String,
}

impl RenameModal {
    pub fn new(target: RenameTarget, sim: &Sim) -> Self {
        let roster = sim.roster();
        let (text, subject, default) = match target {
            RenameTarget::Animal(id) => {
                // (player name, label, species, name-pool id), living first, then the lineage.
                let known = sim.creatures.get(id).map(|c| (c.nickname.clone(), c.label(roster), c.species, c.name));
                let known = known.or_else(|| sim.lineage.get(id).map(|n| (n.nickname.clone(), format!("{} {}", n.name_str(roster), n.tag), n.species, n.name)));
                match known {
                    Some((nick, label, species, name)) => {
                        (nick.unwrap_or_default(), format!("{label} {} {}", crate::glyphs::DOT, roster.name(species)), roster.name_for(species, name).to_string())
                    }
                    None => (String::new(), format!("#{}", id.0), String::new()),
                }
            }
            RenameTarget::Dynasty(root) => {
                let line = sim.lineage.dynasties().get(root);
                let default = line.map_or_else(String::new, |d| founder_line(&d.founder));
                let subject = line.map_or_else(|| default.clone(), |d| format!("{default} {} {}", crate::glyphs::DOT, roster.plural(d.species).to_lowercase()));
                (line.and_then(|d| d.name.clone()).unwrap_or_default(), subject, default)
            }
        };
        Self { target, text, subject, default }
    }

    /// Write the typed name into the sim.
    fn apply(&self, app: &mut AppState) {
        let Some(sim) = app.sim.as_mut() else { return };
        let _ = match self.target {
            RenameTarget::Animal(id) => sim.rename_creature(id, &self.text),
            RenameTarget::Dynasty(root) => sim.rename_dynasty(root, &self.text),
        };
    }
}

impl Screen for RenameModal {
    fn opaque(&self) -> bool {
        false
    }

    fn handle_key(&mut self, key: KeyEvent, app: &mut AppState) -> Action {
        match key.code {
            KeyCode::Enter => {
                self.apply(app);
                Action::Pop
            }
            KeyCode::Esc => Action::Pop,
            KeyCode::Delete => {
                self.text.clear();
                Action::None
            }
            // Names are printable ASCII (CP437-safe); anything else is dropped here
            // so the field shows exactly what will be kept.
            KeyCode::Char(c) if !(c.is_ascii_graphic() || c == ' ') => Action::None,
            code => {
                edit_text(&mut self.text, code, self.target.max());
                Action::None
            }
        }
    }

    fn render(&self, _app: &AppState, f: &mut Frame<'_>, area: Rect) {
        let title = match self.target {
            RenameTarget::Animal(_) => "Name this animal",
            RenameTarget::Dynasty(_) => "Name this dynasty",
        };
        let modal = Modal::new(W.min(area.width.saturating_sub(2)), H.min(area.height.saturating_sub(2))).title(title).hint("Enter keep  Del clear  Esc cancel");
        let buf = f.buffer_mut();
        let body = modal.render(buf, area);
        let width = usize::from(body.width.saturating_sub(2));
        let row = |dy: u16| Rect::new(body.x, body.y + dy, body.width, 1).intersection(body);
        Text::new(format!(" {}", clip(&self.subject, width))).style(theme::dim_text()).render(buf, row(0));
        let box_w = crate::cast!(self.target.max() => u16) + 4;
        TextField::new("Name", format!("{}_", self.text)).label_w(6).box_w(box_w).focused(true).render(buf, row(2));
        let restore = if self.default.is_empty() { String::new() } else { format!(" Empty restores \"{}\".", self.default) };
        Text::new(clip(&restore, width)).style(Style::default().fg(theme::DIM).bg(theme::PANEL_BG)).render(buf, row(4));

        let status_row = area.y + area.height - 1;
        StatusBar::new(&[("Enter", "keep"), ("Del", "clear"), ("Esc", "cancel")]).right("name").render(f.buffer_mut(), Rect::new(area.x, status_row, area.width, 1));
    }
}
