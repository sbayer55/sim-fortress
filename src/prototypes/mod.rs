//! Prototype registry. Each screen file exposes one or more `Prototype`
//! implementations; `all()` lists them in the order they are cycled.

use ratatui::layout::Rect;
use ratatui::Frame;

pub mod s00_title;
pub mod s01_map;
pub mod s02_overlay;
pub mod s03_inspector;
pub mod s04_species;
pub mod s05_charts;
pub mod s06_ecology;
pub mod s07_log;
pub mod s08_lineage;
pub mod s09_worldgen;
pub mod s10_controls;
pub mod s11_help;
pub mod s12_alert;
pub mod s13_zoom;

pub trait Prototype {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn variant(&self) -> &'static str;
    /// Render into `area`, which is the full frame minus the header row (155x44).
    fn render(&self, f: &mut Frame, area: Rect);
}

pub fn all() -> Vec<Box<dyn Prototype>> {
    let mut v: Vec<Box<dyn Prototype>> = Vec::new();
    v.extend(s00_title::all());
    v.extend(s01_map::all());
    v.extend(s02_overlay::all());
    v.extend(s03_inspector::all());
    v.extend(s04_species::all());
    v.extend(s05_charts::all());
    v.extend(s06_ecology::all());
    v.extend(s07_log::all());
    v.extend(s08_lineage::all());
    v.extend(s09_worldgen::all());
    v.extend(s10_controls::all());
    v.extend(s11_help::all());
    v.extend(s12_alert::all());
    v.extend(s13_zoom::all());
    v
}
