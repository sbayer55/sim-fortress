//! The overlay stack (S14): one base heatmap composed with any number of marks,
//! plus the remembered sub-picks (species, pathogen, sense subject) that a row
//! shows even while its layer is off.

use crate::sim::creatures::CreatureId;
use crate::sim::disease::{DiseaseState, PathogenId};
use crate::sim::species::SpeciesId;
use crate::sim::Roster;

/// The base heatmap: at most one, because it recolours every cell.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Base {
    #[default]
    None,
    Vegetation,
    Pressure,
    Moisture,
    /// Population density of the stack's remembered species (S02f).
    Species,
    Parasites,
    /// C5 FR13: the scent grid of the stack's remembered species (S02j).
    Scent,
}

impl Base {
    /// Every base row of the S14 Base heatmap tab, in row order.
    pub const ALL: [Self; 7] = [Self::None, Self::Vegetation, Self::Pressure, Self::Moisture, Self::Species, Self::Parasites, Self::Scent];

    /// The row name as the S14 tab and the sidebar Stack section write it.
    pub const fn name(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Vegetation => "Vegetation",
            Self::Pressure => "Pressure",
            Self::Moisture => "Moisture",
            Self::Species => "Species",
            Self::Parasites => "Parasites",
            Self::Scent => "Scent",
        }
    }
}

/// One layer of the stack, in composition order (S14 item 16).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Layer {
    Base(Base),
    Sense,
    Regions,
    Health,
    Disease,
}

impl Layer {
    /// The four marks of the S14 Marks tab, in row order.
    pub const MARKS: [Self; 4] = [Self::Sense, Self::Regions, Self::Health, Self::Disease];

    /// The layer name as the sidebar Stack section writes it.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Base(b) => b.name(),
            Self::Sense => "Sense",
            Self::Regions => "Regions",
            Self::Health => "Health",
            Self::Disease => "Disease",
        }
    }

    /// `base` or `mark`.
    pub const fn kind(self) -> &'static str {
        match self {
            Self::Base(_) => "base",
            _ => "mark",
        }
    }

    /// The one-line description from the S14 layer table.
    pub const fn description(self) -> &'static str {
        match self {
            Self::Base(Base::None) => "plain terrain",
            Self::Base(Base::Vegetation) => "standing biomass per cell",
            Self::Base(Base::Pressure) => "prey and predator traffic",
            Self::Base(Base::Moisture) => "soil moisture, open water saturated",
            Self::Base(Base::Species) => "population density of one species",
            Self::Base(Base::Parasites) => "parasite load heatmap",
            Self::Base(Base::Scent) => "one predator species' scent and holders",
            Self::Sense => "one predator's sense-range rings",
            Self::Regions => "named regions, tinted with labels",
            Self::Health => "creatures by their weakest vital",
            Self::Disease => "infection state per pathogen",
        }
    }

    /// Whether the row carries a sub-pick list (Species, Scent, Sense, Disease).
    /// Species and Scent share the stack's remembered species.
    pub const fn has_sub_pick(self) -> bool {
        matches!(self, Self::Base(Base::Species | Base::Scent) | Self::Sense | Self::Disease)
    }
}

/// The Disease mark: off, or on for every pathogen (`On(None)`) or one slot.
/// Carrying the on/off state here keeps [`OverlayStack`] at three `bool`s.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Disease {
    #[default]
    Off,
    On(Option<PathogenId>),
}

impl Disease {
    pub const fn is_on(self) -> bool {
        matches!(self, Self::On(_))
    }
}

/// What the map composes this frame, owned by `AppState` so the S14 modal
/// edits it in place and the map reads it every frame.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct OverlayStack {
    pub base: Base,
    /// Remembered even while the base is neither Species nor Scent.
    pub species: SpeciesId,
    pub sense: bool,
    /// Remembered even while `sense` is off; a dead subject is replaced by the
    /// S02d default rule the next time the row is turned on (S14 edge cases).
    pub sense_subject: Option<CreatureId>,
    pub regions: bool,
    pub health: bool,
    pub disease: Disease,
    /// Remembered pathogen while Disease is off (`None` = `All pathogens`).
    pub pathogen: Option<PathogenId>,
}

impl Default for OverlayStack {
    fn default() -> Self {
        Self::PLAIN
    }
}

impl OverlayStack {
    /// Nothing on: the plain map (S01a).
    pub const PLAIN: Self = Self { base: Base::None, species: SpeciesId(0), sense: false, sense_subject: None, regions: false, health: false, disease: Disease::Off, pathogen: None };

    /// Every layer off; the remembered sub-picks survive (S14 State).
    pub const fn clear(&mut self) {
        self.base = Base::None;
        self.sense = false;
        self.regions = false;
        self.health = false;
        self.disease = Disease::Off;
    }

    pub const fn is_empty(&self) -> bool {
        matches!(self.base, Base::None) && !self.sense && !self.regions && !self.health && !self.disease.is_on()
    }

    /// The active layers in composition order (S14 item 16).
    pub fn layers(&self) -> impl Iterator<Item = Layer> + '_ {
        let base = (self.base != Base::None).then_some(Layer::Base(self.base));
        let marks = Layer::MARKS.into_iter().filter(move |m| self.is_on(*m));
        base.into_iter().chain(marks)
    }

    /// Whether `layer` is on. For a base row this means it *is* the base.
    pub fn is_on(&self, layer: Layer) -> bool {
        match layer {
            Layer::Base(b) => self.base == b && b != Base::None,
            Layer::Sense => self.sense,
            Layer::Regions => self.regions,
            Layer::Health => self.health,
            Layer::Disease => self.disease.is_on(),
        }
    }

    /// S14 item 18: creatures and resources fade under a base heatmap unless
    /// Health or Disease is on; Regions and Sense alone never fade anything.
    pub const fn fades_creatures(&self) -> bool {
        !matches!(self.base, Base::None) && !self.health && !self.disease.is_on()
    }

    /// Turn the Disease mark on for `slot` (`None` = every pathogen) and remember it.
    pub const fn show_disease(&mut self, slot: Option<PathogenId>) {
        self.disease = Disease::On(slot);
        self.pathogen = slot;
    }

    /// The active layers named as S02 names them, joined with ` + ` (S14 item 19).
    /// Empty when nothing is on.
    pub fn title(&self, roster: &Roster, disease: &DiseaseState) -> String {
        let names: Vec<String> = self.layers().map(|l| self.layer_title(l, roster, disease)).collect();
        names.join(" + ")
    }

    /// One layer's title word, lowercase, with the species plural or the
    /// pathogen name where the layer carries one.
    pub fn layer_title(&self, layer: Layer, roster: &Roster, disease: &DiseaseState) -> String {
        match layer {
            Layer::Base(Base::None) => String::new(),
            Layer::Base(Base::Vegetation) => "vegetation".into(),
            Layer::Base(Base::Pressure) => "pressure".into(),
            Layer::Base(Base::Moisture) => "moisture".into(),
            Layer::Base(Base::Species) => roster.plural(self.species).to_lowercase(),
            Layer::Base(Base::Parasites) => "parasites".into(),
            Layer::Base(Base::Scent) => format!("{} scent", roster.name(self.species)),
            Layer::Sense => "sense range".into(),
            Layer::Regions => "regions".into(),
            Layer::Health => "health".into(),
            Layer::Disease => match self.disease {
                Disease::On(Some(p)) => format!("disease: {}", disease.name(p).to_lowercase()),
                _ => "disease".into(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::Params;

    fn fixtures() -> (Roster, DiseaseState) {
        let p = Params::default();
        let ds = DiseaseState::new(&p.disease, &p.species);
        (p.species, ds)
    }

    #[test]
    fn plain_is_empty() {
        assert!(OverlayStack::PLAIN.is_empty());
        assert!(OverlayStack::default().is_empty());
        assert_eq!(OverlayStack::default().layers().count(), 0);
        let (roster, ds) = fixtures();
        assert_eq!(OverlayStack::default().title(&roster, &ds), "");
        assert!(!OverlayStack { regions: true, ..OverlayStack::PLAIN }.is_empty());
    }

    #[test]
    fn layers_are_in_composition_order() {
        let s = OverlayStack { base: Base::Moisture, sense: true, regions: true, health: true, disease: Disease::On(None), ..OverlayStack::PLAIN };
        let layers: Vec<Layer> = s.layers().collect();
        assert_eq!(layers, vec![Layer::Base(Base::Moisture), Layer::Sense, Layer::Regions, Layer::Health, Layer::Disease]);
        // A None base is not a layer.
        let s = OverlayStack { health: true, ..OverlayStack::PLAIN };
        assert_eq!(s.layers().collect::<Vec<_>>(), vec![Layer::Health]);
        assert!(!s.is_on(Layer::Base(Base::None)));
    }

    #[test]
    fn title_joins_layers_with_plus() {
        let (roster, ds) = fixtures();
        let s = OverlayStack { base: Base::Moisture, regions: true, health: true, disease: Disease::On(Some(PathogenId(0))), ..OverlayStack::PLAIN };
        assert_eq!(s.title(&roster, &ds), "moisture + regions + health + disease: greyfever");
        // Species uses the roster plural; Disease on every pathogen has no suffix.
        let s = OverlayStack { base: Base::Species, species: SpeciesId(1), sense: true, disease: Disease::On(None), ..OverlayStack::PLAIN };
        assert_eq!(s.title(&roster, &ds), "hares + sense range + disease");
    }

    #[test]
    fn fades_only_under_a_base_without_health_or_disease() {
        let mut s = OverlayStack { base: Base::Vegetation, ..OverlayStack::PLAIN };
        assert!(s.fades_creatures());
        s.regions = true;
        s.sense = true;
        assert!(s.fades_creatures(), "regions and sense do not stop the fade");
        s.health = true;
        assert!(!s.fades_creatures(), "health wants full-strength creatures");
        s.health = false;
        s.disease = Disease::On(None);
        assert!(!s.fades_creatures(), "so does disease");
        let s = OverlayStack { regions: true, sense: true, ..OverlayStack::PLAIN };
        assert!(!s.fades_creatures(), "marks alone never fade anything");
    }

    #[test]
    fn clear_keeps_the_sub_picks() {
        let mut s = OverlayStack { base: Base::Species, species: SpeciesId(3), sense: true, sense_subject: Some(CreatureId(9)), regions: true, health: true, disease: Disease::On(Some(PathogenId(1))), pathogen: Some(PathogenId(1)) };
        s.clear();
        assert!(s.is_empty());
        assert_eq!((s.species, s.sense_subject, s.pathogen), (SpeciesId(3), Some(CreatureId(9)), Some(PathogenId(1))));
    }

    #[test]
    fn show_disease_remembers_the_slot() {
        let mut s = OverlayStack::PLAIN;
        s.show_disease(Some(PathogenId(2)));
        assert_eq!(s.disease, Disease::On(Some(PathogenId(2))));
        assert_eq!(s.pathogen, Some(PathogenId(2)));
        s.disease = Disease::Off;
        assert_eq!(s.pathogen, Some(PathogenId(2)), "remembered while off");
    }
}
