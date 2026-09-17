//! The two tabs' row tables, the sub-pick lists (species, predators,
//! pathogens) read from the sim each frame, and the chosen-entry rule.

use ratatui::style::Color;

use crate::sim::creatures::CreatureId;
use crate::sim::disease::PathogenId;
use crate::sim::{Sim, SpeciesId};
use crate::ui::app::AppState;
use crate::ui::screens::s01_map::{living_predators, pathogen_stops, sub_pick_text};
use crate::ui::style::SpeciesStyle;
use crate::widgets::map::{Base, Layer, OverlayStack};

/// Predators listed before `… and N more`.
pub const PREDATOR_ROWS: usize = 10;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tab {
    Base,
    Marks,
}

impl Tab {
    #[must_use]
    pub const fn other(self) -> Self {
        match self {
            Self::Base => Self::Marks,
            Self::Marks => Self::Base,
        }
    }

    pub const fn title(self) -> &'static str {
        match self {
            Self::Base => "Base heatmap",
            Self::Marks => "Marks",
        }
    }

    /// The word on the tab rule.
    pub const fn rule_word(self) -> &'static str {
        match self {
            Self::Base => "pick one",
            Self::Marks => "any number",
        }
    }

    /// The layers of this tab, in row order.
    pub fn layers(self) -> Vec<Layer> {
        match self {
            Self::Base => Base::ALL.iter().map(|b| Layer::Base(*b)).collect(),
            Self::Marks => Layer::MARKS.to_vec(),
        }
    }
}

/// One layer row of the active tab.
#[derive(Clone, Debug)]
pub struct RowSpec {
    pub layer: Layer,
    /// Cannot be turned on: Sense with no living predator (S14 item 4).
    pub disabled: bool,
    /// The remembered sub-pick, for rows that have one.
    pub value: Option<String>,
}

impl RowSpec {
    /// Whether the row's layer is on; a base row is on when it *is* the base,
    /// `None` included.
    pub fn is_on(&self, stack: &OverlayStack) -> bool {
        match self.layer {
            Layer::Base(b) => stack.base == b,
            other => stack.is_on(other),
        }
    }
}

/// The rows of `tab` for the current sim and stack.
pub fn rows(tab: Tab, app: &AppState) -> Vec<RowSpec> {
    let no_predators = app.sim.as_ref().is_none_or(|sim| living_predators(sim).is_empty());
    tab.layers()
        .into_iter()
        .map(|layer| RowSpec {
            layer,
            disabled: layer == Layer::Sense && no_predators,
            value: (layer.has_sub_pick()).then(|| app.sim.as_ref().map_or_else(String::new, |sim| sub_pick_text(layer, &app.overlay, sim))),
        })
        .collect()
}

/// What a sub-pick entry stands for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pick {
    Species(SpeciesId),
    Predator(CreatureId),
    /// `None` = every pathogen.
    Pathogen(Option<PathogenId>),
}

/// One row of a sub-pick list.
#[derive(Clone, Debug)]
pub struct Entry {
    pub pick: Pick,
    /// A species glyph in the roster colour, when the entry has one.
    pub glyph: Option<(char, Color)>,
    pub label: String,
    pub right: String,
    /// Drawn dim: an extinct species.
    pub absent: bool,
}

/// A sub-pick list: its accent header, its entries and how many predators
/// the cap hid.
#[derive(Clone, Debug)]
pub struct SubPick {
    pub header: &'static str,
    pub entries: Vec<Entry>,
    pub more: usize,
}

impl SubPick {
    /// The index of `pick` in the list.
    pub fn position(&self, pick: Option<Pick>) -> Option<usize> {
        self.entries.iter().position(|e| Some(e.pick) == pick)
    }
}

/// The entry the stack remembers for `layer` (S14 item 10).
pub fn chosen(layer: Layer, stack: &OverlayStack) -> Option<Pick> {
    match layer {
        Layer::Base(Base::Species) => Some(Pick::Species(stack.species)),
        Layer::Sense => stack.sense_subject.map(Pick::Predator),
        Layer::Disease => Some(Pick::Pathogen(stack.pathogen)),
        _ => None,
    }
}

/// The sub-pick list of `layer`, read from the sim each frame; `None` for
/// layers without one or before a world exists.
pub fn sub_pick(layer: Layer, app: &AppState) -> Option<SubPick> {
    let sim = app.sim.as_ref()?;
    match layer {
        Layer::Base(Base::Species) => Some(species_list(sim)),
        Layer::Sense => Some(predator_list(sim, app.overlay.sense_subject)),
        Layer::Disease => Some(pathogen_list(sim)),
        _ => None,
    }
}

/// Every roster species in roster order with its living count (S14 item 7).
fn species_list(sim: &Sim) -> SubPick {
    let roster = sim.roster();
    let entries = roster
        .ids()
        .map(|id| {
            let n = sim.creatures.living().filter(|c| c.species == id).count();
            Entry {
                pick: Pick::Species(id),
                glyph: Some((roster.glyph(id), roster.color(id))),
                label: roster.name(id).to_string(),
                right: if n == 0 { "extinct".to_string() } else { format!("{n} alive") },
                absent: n == 0,
            }
        })
        .collect();
    SubPick { header: "species · which species", entries, more: 0 }
}

/// Living predators by id, at most `PREDATOR_ROWS`, the current subject
/// always among them (S14 item 8).
fn predator_list(sim: &Sim, subject: Option<CreatureId>) -> SubPick {
    let all = living_predators(sim);
    let mut shown: Vec<CreatureId> = all.iter().copied().take(PREDATOR_ROWS).collect();
    if let Some(id) = subject {
        if all.contains(&id) && !shown.contains(&id) {
            shown.pop();
            shown.push(id);
        }
    }
    let roster = sim.roster();
    let entries = shown
        .iter()
        .filter_map(|&id| sim.creatures.get(id))
        .map(|c| Entry {
            pick: Pick::Predator(c.id),
            glyph: Some((roster.adult_glyph(c.species), roster.color(c.species))),
            label: format!("{} ({})", c.name_str(roster), roster.name(c.species)),
            right: format!("sense {}", c.genome.sense_cells()),
            absent: false,
        })
        .collect();
    SubPick { header: "sense · which predator", entries, more: all.len().saturating_sub(shown.len()) }
}

/// `All pathogens`, then every non-extinct slot with strains indented (S14 item 9).
fn pathogen_list(sim: &Sim) -> SubPick {
    let sick = |slot: Option<PathogenId>| sim.creatures.living().filter(|c| c.infection.is_some_and(|i| slot.is_none_or(|p| p == i.pathogen))).count();
    let entries = pathogen_stops(sim)
        .into_iter()
        .map(|slot| {
            let label = match slot.and_then(|p| sim.disease.pathogen(p)) {
                None => "All pathogens".to_string(),
                Some(p) if p.is_strain() => format!("└ {}", p.name()),
                Some(p) => p.name().to_string(),
            };
            Entry { pick: Pick::Pathogen(slot), glyph: None, label, right: format!("{} sick", sick(slot)), absent: false }
        })
        .collect();
    SubPick { header: "disease · which pathogen", entries, more: 0 }
}
