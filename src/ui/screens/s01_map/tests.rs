//! Tests for the S01 map screen.

use super::WorldMap;
use super::disease_overlay::disease_tints;
use super::parasites::parasite_tints;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crate::sim::creatures::CreatureId;
use crate::sim::disease::{self, PathogenId};
use crate::sim::Sim;
use crate::ui::app::AppState;
use crate::ui::screens::{Action, Screen};
use crate::ui::style::{SpeciesStyle};
use crate::widgets::map::{self, Base, Disease, OverlayStack};
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

/// The sidebar's last rows are the Stack section: the rule, `rows`, the hint.
fn assert_stack_section(side: &[String], rows: &[&str]) {
    let at = side.iter().position(|l| l.contains("─ Stack ─")).expect("a Stack section");
    for (i, row) in rows.iter().enumerate() {
        assert!(side[at + 1 + i].contains(row), "row {i}: {:?}", side[at + 1 + i]);
    }
    assert!(side[at + 1 + rows.len()].contains("o edits the stack   Esc clears all"), "hint row: {side:#?}");
    let blank = |l: &String| l.chars().skip(1).take(41).all(|c| c == ' ');
    assert!(side[at + 2 + rows.len()..].iter().all(blank), "nothing follows the Stack section: {side:?}");
}

/// The 41-column sidebar text, one string per inner row.
fn sidebar_rows(buf: &Buffer) -> Vec<String> {
    (1..41).map(|y| row_text(buf, y).chars().skip(112).collect::<String>()).collect()
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
    app.overlay.show_disease(None);
    let mut screen = WorldMap::new("Test".into());
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
    s02h_assert_show_outbreak(&mut screen, &mut app);
}

/// The all-pathogens sidebar content.
fn s02h_assert_sidebar(buf: &Buffer) {
    assert!(row_text(buf, 0).contains("overlay: disease"), "map title names the overlay");
    assert!(row_text(buf, 0).contains("Overlay"), "sidebar title");
    let side: Vec<String> = (1..41).map(|y| row_text(buf, y).chars().skip(112).collect::<String>()).collect();
    assert!(side.iter().any(|l| l.contains("Pathogens · all")));
    assert!(side.iter().any(|l| l.contains("└ Strain7")), "strains are indented under their parent");
    assert!(side.iter().any(|l| l.contains("new")), "a strain born today carries the new tag");
    assert!(side.iter().any(|l| l.contains("Tab pathogen")), "the reading note survives with all eight slots");
    assert_stack_section(&side, &["1. Disease   (mark)"]);
}

/// Tab walks all → slot 0 … slot 7 → all; Shift+Tab walks back.
fn s02h_assert_tab_cycle(screen: &mut WorldMap, app: &mut AppState) {
    screen.handle_key(key(KeyCode::Tab), app);
    assert_eq!(app.overlay.disease, Disease::On(Some(PathogenId(0))));
    for i in 1..8u8 {
        screen.handle_key(key(KeyCode::Tab), app);
        assert_eq!(app.overlay.disease, Disease::On(Some(PathogenId(i))));
        let buf = draw(screen, app);
        let name = app.sim.as_ref().unwrap().disease.name(PathogenId(i)).to_lowercase();
        assert!(row_text(&buf, 0).contains(&format!("overlay: disease: {name}")), "{}", row_text(&buf, 0));
    }
    screen.handle_key(key(KeyCode::Tab), app);
    assert_eq!(app.overlay.disease, Disease::On(None));
    screen.handle_key(key(KeyCode::BackTab), app);
    assert_eq!(app.overlay.disease, Disease::On(Some(PathogenId(7))));
    assert_eq!(app.overlay.pathogen, Some(PathogenId(7)), "the slot is remembered");
}

/// The S12b hand-off: the Disease mark comes on for the slot, over whatever
/// base is up, and a vanished slot falls back to every pathogen.
fn s02h_assert_show_outbreak(screen: &mut WorldMap, app: &mut AppState) {
    app.overlay.clear();
    app.overlay.base = Base::Moisture;
    app.overlay.show_disease(Some(PathogenId(2)));
    let buf = draw(screen, app);
    let name = app.sim.as_ref().unwrap().disease.name(PathogenId(2)).to_lowercase();
    assert!(row_text(&buf, 0).contains(&format!("overlay: moisture + disease: {name}")), "{}", row_text(&buf, 0));
    // A slot that no longer exists: every pathogen, from the next key on.
    app.sim.as_mut().unwrap().disease.pathogens.truncate(2);
    screen.handle_key(key(KeyCode::Right), app);
    assert_eq!(app.overlay.disease, Disease::On(None));
    assert_eq!(app.overlay.pathogen, None);
}

#[test]
fn o_pushes_the_switcher_from_every_mode() {
    let mut app = AppState::new(Params::default());
    app.sim = Some(Sim::new(7, Params::default()));
    let mut screen = WorldMap::new("Test".into());
    assert!(matches!(screen.handle_key(key(KeyCode::Char('o')), &mut app), Action::Push(_)));
    app.enter_look((5, 5));
    assert!(matches!(screen.handle_key(key(KeyCode::Char('o')), &mut app), Action::Push(_)));
    assert!(app.look_cursor.is_some(), "look mode is kept");
    app.leave_look();
    app.follow = app.sim.as_ref().unwrap().creatures.living_ids().first().copied();
    assert!(matches!(screen.handle_key(key(KeyCode::Char('o')), &mut app), Action::Push(_)));
    assert!(app.follow.is_some(), "follow mode is kept");
}

#[test]
fn digits_do_nothing_on_the_map() {
    let mut app = AppState::new(Params::default());
    app.sim = Some(Sim::new(7, Params::default()));
    let mut screen = WorldMap::new("Test".into());
    for c in '0'..='9' {
        assert!(matches!(screen.handle_key(key(KeyCode::Char(c)), &mut app), Action::Unhandled), "{c}");
        assert_eq!(app.overlay, OverlayStack::PLAIN);
    }
    app.enter_look((5, 5));
    assert!(matches!(screen.handle_key(key(KeyCode::Char('7')), &mut app), Action::Unhandled));
    assert_eq!(app.overlay, OverlayStack::PLAIN);
}

#[test]
fn dead_sense_subject_turns_the_mark_off() {
    let mut app = AppState::new(Params::default());
    app.sim = Some(Sim::new(7, Params::default()));
    let mut screen = WorldMap::new("Test".into());
    assert!(WorldMap::turn_sense_on(&mut app));
    let id = app.overlay.sense_subject.expect("a predator lives");
    app.sim.as_mut().unwrap().creatures.get_mut(id).unwrap().alive = false;
    // The next frame draws without the ring; the next key clears the subject.
    let buf = draw(&screen, &app);
    assert!(!row_text(&buf, 0).contains("sense"), "{}", row_text(&buf, 0));
    screen.handle_key(key(KeyCode::Right), &mut app);
    assert!(!app.overlay.sense);
    assert_eq!(app.overlay.sense_subject, None);
    // Turning it on again picks a new subject by the S02d rule.
    assert!(WorldMap::turn_sense_on(&mut app));
    assert_ne!(app.overlay.sense_subject, Some(id));
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
    app.overlay.regions = true;
    let screen = WorldMap::new("Test".into());
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
    app.overlay.base = Base::Parasites;
    let screen = WorldMap::new("Test".into());
    let buf = draw(&screen, &app);
    assert!(row_text(&buf, 0).contains("overlay: parasites"));
    let (x, y) = (crate::cast!((idx % w) => u16) + 1, crate::cast!((idx.div_euclid(w)) => u16) + 1);
    assert_eq!(buf[(x, y)].fg, theme::parasite(0.9), "the fouled cell is shaded on the parasite ramp");
    let side: Vec<String> = (1..41).map(|y| row_text(&buf, y).chars().skip(112).collect::<String>()).collect();
    assert!(side.iter().any(|l| l.contains("By region")));
    assert!(side.iter().any(|l| l.contains("Carriers")));
    assert!(side.iter().any(|l| l.contains("cells ≥25%")));
    assert!(side.iter().any(|l| l.contains("litter")));
    assert!(side.iter().any(|l| l.contains("k look")), "the reading note is kept");
    assert_stack_section(&side, &["1. Parasites (base)"]);
    assert!(row_text(&buf, 44).contains("[o] overlay"), "status bar names the switcher");
}

#[test]
fn single_layer_sidebar_ends_with_stack_section() {
    let mut app = AppState::new(Params::default());
    app.sim = Some(Sim::new(7, Params::default()));
    let screen = WorldMap::new("Test".into());
    for (stack, row) in [
        (OverlayStack { base: Base::Moisture, ..OverlayStack::PLAIN }, "1. Moisture  (base)"),
        (OverlayStack { base: Base::Species, ..OverlayStack::PLAIN }, "1. Species   (base)"),
        (OverlayStack { regions: true, ..OverlayStack::PLAIN }, "1. Regions   (mark)"),
        (OverlayStack { health: true, ..OverlayStack::PLAIN }, "1. Health    (mark)"),
    ] {
        app.overlay = stack;
        let side = sidebar_rows(&draw(&screen, &app));
        assert!(!side.iter().any(|l| l.contains("Overlays")), "the selector is gone: {side:?}");
        assert_stack_section(&side, &[row]);
    }
    app.overlay = OverlayStack::PLAIN;
    assert!(WorldMap::turn_sense_on(&mut app));
    let side = sidebar_rows(&draw(&screen, &app));
    assert_stack_section(&side, &["1. Sense     (mark)"]);
}

#[test]
fn compact_sidebar_lists_layers_in_order() {
    let mut app = AppState::new(Params::default());
    app.sim = Some(Sim::new(7, Params::default()));
    app.overlay = OverlayStack { base: Base::Species, species: crate::sim::SpeciesId(2), regions: true, health: true, ..OverlayStack::PLAIN };
    app.overlay.show_disease(None);
    let screen = WorldMap::new("Test".into());
    let buf = draw(&screen, &app);
    assert!(row_text(&buf, 0).contains("overlay: deer + regions + health + disease"), "{}", row_text(&buf, 0));
    let side = sidebar_rows(&buf);
    let find = |s: &str| side.iter().position(|l| l.contains(s)).unwrap_or_else(|| panic!("{s} in {side:?}"));
    let (sp, re, he, di) = (find("─ Species · deer ─"), find("─ Regions ─"), find("─ Health ─"), find("─ Disease · All pathogens ─"));
    assert!(sp < re && re < he && he < di, "sections in composition order");
    assert!(side[sp + 1].contains("population density of one species"));
    assert!(side[he + 1].contains("creatures by their weakest vital"));
    assert!(side[he + 2].contains("fit") && side[he + 2].contains("critical"));
    assert!(side[di + 2].contains("sick") && side[di + 2].contains("immune"));
    assert_stack_section(&side, &["1. Species   (base)", "2. Regions   (mark)", "3. Health    (mark)", "4. Disease   (mark)"]);
}

#[test]
fn long_title_is_cut_before_the_hint() {
    let mut app = AppState::new(Params::default());
    app.sim = Some(Sim::new(7, Params::default()));
    app.overlay = OverlayStack { base: Base::Species, sense: false, regions: true, health: true, ..OverlayStack::PLAIN };
    app.overlay.show_disease(Some(PathogenId(0)));
    let screen = WorldMap::new("The Valley of Sunfall and the Long Road Beyond It".into());
    let top = row_text(&draw(&screen, &app), 0);
    assert!(top.contains("← → scroll ╗"), "the scroll hint survives: {top}");
    assert!(top.contains(&format!("{}", glyphs::DOT)), "the title is cut with a dot: {top}");
    assert!(!top.contains("greyfever"), "the tail of the title is gone: {top}");
}

#[test]
fn s02j_scent_overlay_render() {
    // C5 FR13: a fox-held cell in view, shaded on the fox ramp, with the
    // holder listed in the sidebar.
    let mut sim = Sim::new(7, Params::default());
    let w = sim.world.width();
    let fox = crate::sim::SpeciesId(3);
    let holder = sim.creatures.living().find(|c| c.species == fox).map(|c| c.id).expect("a living fox");
    let idx = (0..sim.world.cells.len())
        .find(|&i| {
            let t = sim.world.cells[i].terrain;
            let (x, y) = (i % w, i.div_euclid(w));
            !t.is_water() && t != Terrain::Rock && x < 110 && y < 40
        })
        .expect("a land cell in the viewport");
    let (x, y) = (idx % w, idx.div_euclid(w));
    *sim.world.mark_mut(fox, x, y).unwrap() = crate::sim::world::Mark { strength: 0.9, holder };
    let color = sim.roster().color(fox);

    let mut app = AppState::new(Params::default());
    app.sim = Some(sim);
    app.overlay = OverlayStack { base: Base::Scent, species: fox, ..OverlayStack::PLAIN };
    let screen = WorldMap::new("Test".into());
    let buf = draw(&screen, &app);
    assert!(row_text(&buf, 0).contains("overlay: fox scent"), "{}", row_text(&buf, 0));
    let (sx, sy) = (crate::cast!(x => u16) + 1, crate::cast!(y => u16) + 1);
    let occupied = app.sim.as_ref().unwrap().creatures.living().any(|c| (c.x, c.y) == (x, y));
    if !occupied {
        assert_eq!(buf[(sx, sy)].fg, theme::species_ramp(color, 0.9), "the held cell is shaded on the species ramp");
    }
    let side: Vec<String> = (1..41).map(|y| row_text(&buf, y).chars().skip(112).collect::<String>()).collect();
    assert!(side.iter().any(|l| l.contains("Fox scent")), "{side:?}");
    assert!(side.iter().any(|l| l.contains("By region")));
    assert!(side.iter().any(|l| l.contains("Holders")));
    assert!(side.iter().any(|l| l.contains("1 cells")), "the holder's one cell is listed: {side:?}");
    assert!(side.iter().any(|l| l.contains("Tab next species")));
    assert_stack_section(&side, &["1. Scent     (base)"]);
    // Tab cycles the species under the Scent base, as under Species.
    let mut screen = screen;
    screen.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE), &mut app);
    assert_eq!(app.overlay.species, crate::sim::SpeciesId(4));
    assert_eq!(app.overlay.base, Base::Scent);
}
