//! Tests for the S01 map screen.

use super::WorldMap;
use super::disease_overlay::disease_tints;
use super::parasites::parasite_tints;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crate::sim::creatures::CreatureId;
use crate::sim::disease::{self, PathogenId};
use crate::sim::Sim;
use crate::ui::app::AppState;
use crate::ui::screens::Screen;
use crate::ui::style::{SpeciesStyle};
use crate::widgets::map::{self, Overlay};
use crate::{glyphs, theme};
use crate::sim::disease::{Infection, Stage};
use crate::sim::world::{Cell, Terrain};
use crate::sim::Params;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::Terminal;

fn key(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

fn draw(screen: &WorldMap, app: &AppState) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(155, 45)).unwrap();
    terminal.draw(|f| screen.render(app, f, f.area())).unwrap();
    terminal.backend().buffer().clone()
}

fn row_text(buf: &Buffer, y: u16) -> String {
    (0..buf.area.width).map(|x| buf[(x, y)].symbol().chars().next().unwrap_or(' ')).collect()
}

/// A sim with every pathogen slot filled (the roster plus strains of slot 0).
fn full_roster_sim() -> Sim {
    let mut sim = Sim::new(7, Params::default());
    let day = crate::cast!(sim.time.day_index() => u32);
    while sim.disease.pathogens.len() < disease::MAX_PATHOGENS {
        let mut p = sim.disease.pathogens[0].clone();
        p.parent = Some(PathogenId(0));
        p.born_day = Some(day);
        p.params.name = format!("Strain{}", sim.disease.pathogens.len());
        sim.disease.pathogens.push(p);
    }
    sim
}

/// A roster with one infectious, one incubating, one slot-7-immune and one
/// parasite-heavy creature, in living-id order.
fn disease_fixture() -> (Sim, CreatureId, CreatureId, CreatureId, CreatureId) {
    let mut sim = full_roster_sim();
    let day = crate::cast!(sim.time.day_index() => u32);
    let ids = sim.creatures.living_ids();
    assert!(ids.len() >= 4, "the default world starts with founders");
    let (a, b, c, d) = (ids[0], ids[1], ids[2], ids[3]);
    let infection = |stage: Stage, p: u8| Infection { pathogen: PathogenId(p), stage, since_day: day, ends_day: day + 5, severity: 0.5, source: None, outbreak: 0 };
    sim.creatures.get_mut(a).unwrap().infection = Some(infection(Stage::Infectious, 0));
    sim.creatures.get_mut(b).unwrap().infection = Some(infection(Stage::Incubating, 0));
    let cc = sim.creatures.get_mut(c).unwrap();
    cc.infection = None;
    cc.immune_until[7] = u32::MAX;
    let dd = sim.creatures.get_mut(d).unwrap();
    dd.infection = None;
    dd.parasite_load = 0.8;
    (sim, a, b, c, d)
}

#[test]
fn s02h_disease_tint_bands() {
    let (sim, a, b, c, d) = disease_fixture();
    let species_of = |sim: &Sim, id: CreatureId| sim.roster().color(sim.creatures.get(id).unwrap().species);

    // All pathogens shown: every band present.
    let tints = disease_tints(&sim, None);
    assert_eq!(tints[&a], (theme::SICK, true), "infectious is SICK bold");
    assert_eq!(tints[&b], (theme::dim(theme::SICK, 0.4), false), "incubating is SICK dimmed 40%");
    assert_eq!(tints[&c].0, theme::IMMUNE, "immune to any slot reads IMMUNE when all are shown");
    assert_eq!(tints[&d].0, theme::WARN, "heavy parasite load reads WARN");
    // One slot shown: an infection with another pathogen counts as healthy,
    // and immunity is only to that slot.
    let tints7 = disease_tints(&sim, Some(PathogenId(7)));
    assert_eq!(tints7[&a].0, theme::dim(species_of(&sim, a), map::HEALTHY_FADE));
    assert_eq!(tints7[&c].0, theme::IMMUNE);
    let tints0 = disease_tints(&sim, Some(PathogenId(0)));
    assert_eq!(tints0[&a].0, theme::SICK);
    assert_eq!(tints0[&c].0, theme::dim(species_of(&sim, c), map::HEALTHY_FADE), "immune to slot 7 is not immune to slot 0");
}

#[test]
fn s02h_disease_overlay_navigation() {
    let (sim, a, _, _, _) = disease_fixture();
    // Render at 155×45 with all eight slots, for every Tab stop.
    let mut app = AppState::new(Params::default());
    app.sim = Some(sim);
    let mut screen = WorldMap::new("Test".into());
    screen.overlay = Overlay::Disease(None);
    let buf = draw(&screen, &app);
    s02h_assert_sidebar(&buf);
    // The infectious creature draws in SICK when it is inside the viewport
    // and nothing else stands on its cell (a stacked creature draws on top).
    let sim = app.sim.as_ref().unwrap();
    let ca = sim.creatures.get(a).unwrap();
    let alone = sim.creatures.living().filter(|c| c.x == ca.x && c.y == ca.y).count() == 1;
    if alone && ca.x < 110 && ca.y < 40 {
        assert_eq!(buf[(crate::cast!(ca.x => u16) + 1, crate::cast!(ca.y => u16) + 1)].fg, theme::SICK);
    }
    s02h_assert_tab_cycle(&mut screen, &mut app);
    s02h_assert_pending(&mut screen, &mut app);
    s02h_assert_cycle(&mut screen, &mut app);
}

/// The all-pathogens sidebar content.
fn s02h_assert_sidebar(buf: &Buffer) {
    assert!(row_text(buf, 0).contains("overlay: disease"), "map title names the overlay");
    assert!(row_text(buf, 0).contains("Overlay"), "sidebar title");
    let side: Vec<String> = (1..41).map(|y| row_text(buf, y).chars().skip(112).collect::<String>()).collect();
    assert!(side.iter().any(|l| l.contains("Pathogens · all")));
    assert!(side.iter().any(|l| l.contains("└ Strain7")), "strains are indented under their parent");
    assert!(side.iter().any(|l| l.contains("new")), "a strain born today carries the new tag");
    assert!(side.iter().any(|l| l.contains("9 ") && l.contains("parasites")), "selector lists 9 rows");
    assert!(side[39].contains("Tab pathogen"), "with all eight slots the reading note is the 40th sidebar row");
}

/// Tab walks all → slot 0 … slot 7 → all; Shift+Tab walks back.
fn s02h_assert_tab_cycle(screen: &mut WorldMap, app: &mut AppState) {
    screen.handle_key(key(KeyCode::Tab), app);
    assert_eq!(screen.overlay, Overlay::Disease(Some(PathogenId(0))));
    for i in 1..8u8 {
        screen.handle_key(key(KeyCode::Tab), app);
        assert_eq!(screen.overlay, Overlay::Disease(Some(PathogenId(i))));
        let buf = draw(screen, app);
        assert!(row_text(&buf, 0).contains("overlay: disease"));
    }
    screen.handle_key(key(KeyCode::Tab), app);
    assert_eq!(screen.overlay, Overlay::Disease(None));
    screen.handle_key(key(KeyCode::BackTab), app);
    assert_eq!(screen.overlay, Overlay::Disease(Some(PathogenId(7))));
}

/// The S12b hook: a pending slot renders at once and is taken by the next key.
fn s02h_assert_pending(screen: &mut WorldMap, app: &mut AppState) {
    screen.overlay = Overlay::None;
    app.pending_overlay = Some(PathogenId(2));
    let buf = draw(screen, app);
    assert!(row_text(&buf, 0).contains("overlay: disease"));
    screen.handle_key(key(KeyCode::Right), app);
    assert_eq!(screen.overlay, Overlay::Disease(Some(PathogenId(2))));
    assert!(app.pending_overlay.is_none());
}

/// `o` walks health → disease → parasites → plain map; `8` and `9` go direct.
fn s02h_assert_cycle(screen: &mut WorldMap, app: &mut AppState) {
    screen.overlay = Overlay::Health;
    screen.handle_key(key(KeyCode::Char('o')), app);
    assert_eq!(screen.overlay, Overlay::Disease(None));
    screen.handle_key(key(KeyCode::Char('o')), app);
    assert_eq!(screen.overlay, Overlay::Parasites);
    screen.handle_key(key(KeyCode::Char('o')), app);
    assert_eq!(screen.overlay, Overlay::None);
    screen.handle_key(key(KeyCode::Char('9')), app);
    assert_eq!(screen.overlay, Overlay::Parasites);
    screen.handle_key(key(KeyCode::Char('8')), app);
    assert_eq!(screen.overlay, Overlay::Disease(None));
}

#[test]
fn s02i_parasite_ramp_and_cells() {
    // The ramp and the cell shading.
    assert_eq!(theme::parasite(0.5), theme::WARN);
    assert_eq!(theme::parasite(1.0), theme::BAD);
    let cell = |terrain: Terrain, load: f32| Cell { terrain, biome: crate::sim::world::Biome::Grassland, elevation: 0.5, moisture: 0.5, temperature: 0.5, vegetation: 0.5, prey_pressure: 0.0, pred_pressure: 0.0, dried_from: None, parasite_load: load };
    let (g, fg, _) = map::parasite_cell(&cell(Terrain::Dirt, 0.9));
    assert_eq!((g, fg), (glyphs::shade(0.9), theme::parasite(0.9)));
    assert_eq!(map::parasite_cell(&cell(Terrain::Dirt, 0.0)).0, glyphs::DIRT, "an empty shade shows the dirt glyph");
    assert_eq!(map::parasite_cell(&cell(Terrain::DeepWater, 0.0)).0, glyphs::DEEP_WATER);
    assert_eq!(map::parasite_cell(&cell(Terrain::Rock, 0.9)).0, glyphs::ROCK);
    let (g, fg, _) = map::parasite_cell(&cell(Terrain::ShallowWater, 0.3));
    assert_eq!((g, fg), (glyphs::SHALLOW_WATER, theme::WARN), "fouled water draws ~ in WARN");
    // Creature bands.
    assert_eq!(map::parasite_tint(theme::TAN, 0.1), (theme::dim(theme::TAN, map::HEALTHY_FADE), false));
    assert_eq!(map::parasite_tint(theme::TAN, 0.3), (theme::WARN, false));
    assert_eq!(map::parasite_tint(theme::TAN, 0.7), (theme::BAD, true));
}

#[test]
fn s01c_look_sidebar_names_the_feature_under_the_cursor() {
    let sim = full_roster_sim();
    let lake = sim.world.features_of_kind(crate::sim::world::FeatureKind::Lake).next().expect("a named lake").clone();
    let mut app = AppState::new(Params::default());
    app.look_cursor = Some(lake.anchor);
    app.sim = Some(sim);
    let screen = WorldMap::new("Test".into());
    let buf = draw(&screen, &app);
    let side: Vec<String> = (1..41).map(|y| row_text(&buf, y).chars().skip(112).collect::<String>()).collect();
    assert!(side.iter().any(|l| l.contains(&lake.name)), "the lake's name is in the look sidebar: {side:?}");
    assert!(side.iter().any(|l| l.contains("lake ·") && l.contains("cells")), "its kind and size follow: {side:?}");
}

#[test]
fn s02e_region_overlay_draws_feature_labels() {
    let sim = full_roster_sim();
    let lake = sim.world.features_of_kind(crate::sim::world::FeatureKind::Lake).next().expect("a named lake").clone();
    let mut app = AppState::new(Params::default());
    app.sim = Some(sim);
    let mut screen = WorldMap::new("Test".into());
    screen.overlay = Overlay::Region;
    let buf = draw(&screen, &app);
    let (ax, ay) = lake.anchor;
    let row = row_text(&buf, crate::cast!(ay => u16) + 1);
    let map_row: String = row.chars().take(112).collect();
    if ax < 110 {
        assert!(map_row.contains(&lake.name), "the lake's label is on its anchor row: {map_row:?}");
    }
}

#[test]
fn s02i_parasite_overlay_render() {
    // A full-screen render with a fouled land cell in view.
    let mut sim = full_roster_sim();
    let w = sim.world.width();
    let occupied: std::collections::HashSet<(usize, usize)> = sim.creatures.living().map(|c| (c.x, c.y)).collect();
    let idx = (0..sim.world.cells.len())
        .find(|&i| {
            let t = sim.world.cells[i].terrain;
            let (x, y) = (i % w, i.div_euclid(w));
            !t.is_water() && t != Terrain::Rock && x < 110 && y < 40 && !occupied.contains(&(x, y))
        })
        .expect("a free land cell in the viewport");
    sim.world.cells[idx].parasite_load = 0.9;
    let ids = sim.creatures.living_ids();
    sim.creatures.get_mut(ids[0]).unwrap().parasite_load = 0.7;
    sim.creatures.get_mut(ids[1]).unwrap().parasite_load = 0.3;
    let tints = parasite_tints(&sim);
    assert_eq!(tints[&ids[0]], (theme::BAD, true));
    assert_eq!(tints[&ids[1]], (theme::WARN, false));

    let mut app = AppState::new(Params::default());
    app.sim = Some(sim);
    let mut screen = WorldMap::new("Test".into());
    screen.overlay = Overlay::Parasites;
    let buf = draw(&screen, &app);
    assert!(row_text(&buf, 0).contains("overlay: parasites"));
    let (x, y) = (crate::cast!((idx % w) => u16) + 1, crate::cast!((idx.div_euclid(w)) => u16) + 1);
    assert_eq!(buf[(x, y)].fg, theme::parasite(0.9), "the fouled cell is shaded on the parasite ramp");
    let side: Vec<String> = (1..41).map(|y| row_text(&buf, y).chars().skip(112).collect::<String>()).collect();
    assert!(side.iter().any(|l| l.contains("By region")));
    assert!(side.iter().any(|l| l.contains("Carriers")));
    assert!(side.iter().any(|l| l.contains("cells ≥25%")));
    assert!(side.iter().any(|l| l.contains("litter")));
    assert!(side[39].contains("k look"), "the reading note is the 40th sidebar row");
    assert!(row_text(&buf, 44).contains("1-9"), "status bar hints cover the nine overlays");
}
