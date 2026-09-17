//! C6 UI tests: title navigation, load-list ordering, confirm-modal keys and
//! options persistence. C9: the chronicle trigger and, with `--features ai`,
//! the R1/R2 tests against the fake gateway.

use std::path::PathBuf;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::sim::params::{DayNightTint, UiParams};
use crate::sim::{save, Params, Sim};
use crate::ui::app::{AppState, ConfirmRequest, ConfirmYes};
use crate::ui::config::{load_ui_from, save_ui_to};
use crate::ui::screens::confirm::ConfirmModal;
use crate::ui::screens::s00_title::Title;
use crate::ui::screens::{Action, Screen};

fn key(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

fn tmpdir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("simf-ui-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn title_menu_navigation() {
    let dir = tmpdir("title");
    let mut app = AppState::new(Params::default());
    app.saves_dir = dir;
    let mut title = Title::new();
    assert_eq!(title.selection, 0);

    // With no saves, Down skips "Load World" (index 1).
    title.handle_key(key(KeyCode::Down), &mut app);
    assert_eq!(title.selection, 2, "Load World is skipped when there are no saves");
    title.handle_key(key(KeyCode::Down), &mut app);
    assert_eq!(title.selection, 3);
    title.handle_key(key(KeyCode::Up), &mut app);
    assert_eq!(title.selection, 2);
    title.handle_key(key(KeyCode::Up), &mut app);
    assert_eq!(title.selection, 0);

    // Enter on New World opens the world generation form.
    assert!(matches!(title.handle_key(key(KeyCode::Enter), &mut app), Action::Push(_)));

    // Enter on Options opens the controls modal.
    title.selection = 2;
    assert!(matches!(title.handle_key(key(KeyCode::Enter), &mut app), Action::Push(_)));

    // Quit with no world exits directly (no confirm).
    title.selection = 3;
    assert!(matches!(title.handle_key(key(KeyCode::Enter), &mut app), Action::Quit));
}

#[test]
fn load_list_sorted() {
    let dir = tmpdir("loadlist");
    let mut a = Sim::new(1, Params::default());
    for _ in 0..100 {
        a.step();
    }
    save::save(&a, "Alpha World", &dir).unwrap();
    let mut b = Sim::new(2, Params::default());
    for _ in 0..300 {
        b.step();
    }
    save::save(&b, "Beta World", &dir).unwrap();

    let saves = save::list_saves(&dir);
    assert_eq!(saves.len(), 2);
    assert_eq!(saves[0].header.world_name, "Beta World", "newest save first");
    assert_eq!(saves[1].header.world_name, "Alpha World");
}

#[test]
fn confirm_modal_keys() {
    let mut app = AppState::new(Params::default());
    let mut m = ConfirmModal::new();
    assert_eq!(m.focus, 0);

    // Esc = No: pops and clears the request.
    app.confirm = Some(ConfirmRequest { question: "really?".into(), yes: ConfirmYes::QuitApp });
    assert!(matches!(m.handle_key(key(KeyCode::Esc), &mut app), Action::Pop));
    assert!(app.confirm.is_none());

    // Yes (focus 0) on QuitApp quits.
    app.confirm = Some(ConfirmRequest { question: "really?".into(), yes: ConfirmYes::QuitApp });
    m.focus = 0;
    assert!(matches!(m.handle_key(key(KeyCode::Enter), &mut app), Action::Quit));

    // No (focus 1) pops without acting.
    app.confirm = Some(ConfirmRequest { question: "really?".into(), yes: ConfirmYes::QuitApp });
    m.focus = 1;
    assert!(matches!(m.handle_key(key(KeyCode::Enter), &mut app), Action::Pop));
    assert!(app.confirm.is_none());

    // Left/Right move the focus.
    app.confirm = Some(ConfirmRequest { question: "really?".into(), yes: ConfirmYes::QuitApp });
    m.focus = 0;
    m.handle_key(key(KeyCode::Right), &mut app);
    assert_eq!(m.focus, 1);
    m.handle_key(key(KeyCode::Left), &mut app);
    assert_eq!(m.focus, 0);
}

#[test]
fn options_persist() {
    let dir = tmpdir("options");
    let path = dir.join("ui.toml");
    let ui = UiParams {
        autosave_days: 7,
        day_night_tint: DayNightTint::Off,
        log_births: true,
        pause_on_follow_death: false,
        ..UiParams::default()
    };
    save_ui_to(&path, &ui).unwrap();
    assert_eq!(load_ui_from(&path).unwrap(), ui);
}

/// `ui.toml` files written before the three-state setting stored a bool.
#[test]
fn legacy_bool_day_night_tint_loads() {
    let dir = tmpdir("legacy-tint");
    let path = dir.join("ui.toml");
    std::fs::write(&path, "day_night_tint = true\nlog_births = true\n").unwrap();
    let ui = load_ui_from(&path).unwrap();
    assert_eq!(ui.day_night_tint, DayNightTint::Map);
    assert!(ui.log_births);
    std::fs::write(&path, "day_night_tint = false\n").unwrap();
    assert_eq!(load_ui_from(&path).unwrap().day_night_tint, DayNightTint::Off);
    std::fs::write(&path, "day_night_tint = \"status_text\"\n").unwrap();
    assert_eq!(load_ui_from(&path).unwrap().day_night_tint, DayNightTint::StatusText);
    assert_eq!(UiParams::default().day_night_tint, DayNightTint::StatusText, "status text is the default");
}

/// Regenerate the text renders of the live data screens, the way the C6 snapshot
/// set was produced: draw the screen over a deterministic `Sim` on a
/// `ratatui::backend::TestBackend` at the fixed 155x45 frame and write the buffer.
///
/// `cargo test --lib -- --ignored regenerate_screen_renders`
#[test]
#[ignore = "writes docs/screens/renders/*.txt; run explicitly to refresh the snapshots"]
fn regenerate_screen_renders() {
    use crate::ui::screens::s03_inspector::Inspector;
    use crate::ui::screens::s04_species::{SpeciesBrowser, SpeciesDetail};
    use crate::ui::screens::Screen;
    use crate::sim::{SpeciesId, Kind};
    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;
    use ratatui::Terminal;

    // One deterministic year of a default world: populated, mid-drift.
    let mut sim = Sim::new(7, Params::default());
    for _ in 0..8640 {
        sim.step();
    }
    // S03a: the oldest living prey (the render is titled with its species).
    let prey = sim
        .creatures
        .living()
        .filter(|c| sim.roster().kind(c.species) == Kind::Prey)
        .min_by_key(|c| (c.born_day, c.id))
        .map(|c| (c.id, c.species))
        .expect("the default world keeps prey alive for a year");

    // S04b: a predator with living members if there is one, else the most numerous.
    let detail_species = sim
        .roster()
        .ids()
        .filter(|s| sim.species[s.index()].count > 0)
        .max_by_key(|s| (sim.roster().kind(*s) == Kind::Predator, sim.species[s.index()].count))
        .unwrap_or(SpeciesId(0));
    let prey_name = sim.roster().display_name(prey.1);
    let detail_name = sim.roster().display_name(detail_species);

    let mut app = AppState::new(Params::default());
    app.sim = Some(sim);

    // The snapshot convention from docs/screens/README.md: the id/title on row 0,
    // the screen in a 155x44 frame below it (as the prototype binary did).
    let snap = |app: &AppState, screen: &dyn Screen, title: &str| -> String {
        let backend = TestBackend::new(155, 45);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                f.buffer_mut().set_stringn(0, 0, format!(" {title:<154}"), 155, ratatui::style::Style::default());
                screen.render(app, f, Rect::new(0, 1, 155, 44));
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        let mut out = String::new();
        for y in 0..45 {
            for x in 0..155 {
                out.push_str(buf[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    };

    let s03_title = format!("S03a  Creature Inspector - prey ({prey_name})");
    let s04b_title = format!("S04b  Species Browser - species detail ({detail_name})");
    let files = [
        ("docs/screens/renders/S03a.txt", s03_title.clone(), snap(&app, &Inspector::new(prey.0), &s03_title)),
        (
            "docs/screens/renders/S04a.txt",
            "S04a  Species Browser - species table".to_string(),
            snap(&app, &SpeciesBrowser::new(), "S04a  Species Browser - species table"),
        ),
        ("docs/screens/renders/S04b.txt", s04b_title.clone(), snap(&app, &SpeciesDetail::new(detail_species), &s04b_title)),
    ];
    for (path, title, text) in files {
        std::fs::write(path, text).unwrap_or_else(|e| panic!("write {path} ({title}): {e}"));
    }
}

/// The map with the S14 stack: S01a (plain), S02a–i (one layer each) and
/// S14a–c (the switcher over the S02c state). Same frame convention as
/// `regenerate_screen_renders`.
///
/// `cargo test --lib -- --ignored regenerate_screen_renders`
#[test]
#[ignore = "writes docs/screens/renders/S01a, S02*, S14*.txt; run explicitly to refresh the snapshots"]
fn regenerate_screen_renders_overlays() {
    use crate::ui::screens::s01_map::WorldMap;
    use crate::ui::screens::s14_switcher::OverlaySwitcher;
    use crate::ui::screens::{render_stack, Screen, Stack};
    use crate::sim::SpeciesId;
    use crate::widgets::map::{Base, OverlayStack};
    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;
    use ratatui::Terminal;

    let mut sim = Sim::new(7, Params::default());
    for _ in 0..8640 {
        sim.step();
    }
    let mut app = AppState::new(Params::default());
    app.sim = Some(sim);
    app.viewport_origin = (20, 0);
    let world = "The Valley of Sunfall";

    let snap = |app: &AppState, screens: Vec<Box<dyn Screen>>, title: &str| -> String {
        let stack = Stack { screens };
        let backend = TestBackend::new(155, 45);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                f.buffer_mut().set_stringn(0, 0, format!(" {title:<154}"), 155, ratatui::style::Style::default());
                render_stack(&stack, app, f, Rect::new(0, 1, 155, 44));
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        (0..45).map(|y| (0..155).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>() + "\n").collect()
    };
    let map = || -> Box<dyn Screen> { Box::new(WorldMap::new(world.to_string())) };
    let mut files: Vec<(&str, String)> = Vec::new();
    let single: [(&str, &str, OverlayStack); 9] = [
        ("S01a", "World Map - default", OverlayStack::PLAIN),
        ("S02a", "Map Overlay - vegetation density", OverlayStack { base: Base::Vegetation, ..OverlayStack::PLAIN }),
        ("S02b", "Map Overlay - population pressure", OverlayStack { base: Base::Pressure, ..OverlayStack::PLAIN }),
        ("S02c", "Map Overlay - water & moisture", OverlayStack { base: Base::Moisture, ..OverlayStack::PLAIN }),
        ("S02e", "Map Overlay - regions", OverlayStack { regions: true, ..OverlayStack::PLAIN }),
        ("S02f", "Map Overlay - species density", OverlayStack { base: Base::Species, species: SpeciesId(1), ..OverlayStack::PLAIN }),
        ("S02g", "Map Overlay - health", OverlayStack { health: true, ..OverlayStack::PLAIN }),
        ("S02h", "Map Overlay - disease", OverlayStack { disease: crate::widgets::map::Disease::On(None), ..OverlayStack::PLAIN }),
        ("S02i", "Map Overlay - parasites", OverlayStack { base: Base::Parasites, ..OverlayStack::PLAIN }),
    ];
    for (id, title, stack) in single {
        app.overlay = stack;
        files.push((id, snap(&app, vec![map()], &format!("{id}  {title}"))));
    }
    app.overlay = OverlayStack::PLAIN;
    if WorldMap::turn_sense_on(&mut app) {
        files.push(("S02d", snap(&app, vec![map()], "S02d  Map Overlay - sense range of selected predator")));
    }
    // S14 over the S02c state: the Base tab, the Marks tab, the species list.
    app.overlay = OverlayStack { base: Base::Moisture, ..OverlayStack::PLAIN };
    let k = |c| KeyEvent::new(c, KeyModifiers::NONE);
    let s14a = OverlaySwitcher::open(&mut app);
    files.push(("S14a", snap(&app, vec![map(), Box::new(s14a)], "S14a  Overlay Switcher - base heatmap tab")));
    let mut s14b = OverlaySwitcher::open(&mut app);
    s14b.handle_key(k(KeyCode::Tab), &mut app);
    files.push(("S14b", snap(&app, vec![map(), Box::new(s14b)], "S14b  Overlay Switcher - marks tab")));
    let mut s14c = OverlaySwitcher::open(&mut app);
    for _ in 0..4 {
        s14c.handle_key(k(KeyCode::Down), &mut app);
    }
    s14c.handle_key(k(KeyCode::Right), &mut app);
    files.push(("S14c", snap(&app, vec![map(), Box::new(s14c)], "S14c  Overlay Switcher - sub-pick list focused")));
    for (id, text) in files {
        let path = format!("docs/screens/renders/{id}.txt");
        std::fs::write(&path, text).unwrap_or_else(|e| panic!("write {path}: {e}"));
    }
}

/// C9 renders, AI off: S07c with a year of tally entries and S10a with the AI
/// section. Same frame convention as `regenerate_screen_renders`.
///
/// `cargo test --lib -- --ignored regenerate_screen_renders`
#[test]
#[ignore = "writes docs/screens/renders/S07c.txt and S10a.txt; run explicitly to refresh the snapshots"]
fn regenerate_screen_renders_c9() {
    use crate::ui::screens::s01_map::WorldMap;
    use crate::ui::screens::s07_log::EventLog;
    use crate::ui::screens::s10_controls::Controls;
    use crate::ui::screens::{render_stack, Screen, Stack};
    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;
    use ratatui::Terminal;

    // One year through the app's own batch loop, so every season boundary
    // leaves a template chronicle entry.
    let mut app = AppState::new(Params::default());
    app.sim = Some(Sim::new(7, Params::default()));
    for _ in 0..360 {
        app.step_ticks(24);
        app.enqueue_chronicle();
    }
    app.world_name = Some("The Valley of Sunfall".to_string());
    app.paused = true;

    let snap = |app: &AppState, screens: Vec<Box<dyn Screen>>, title: &str| -> String {
        let stack = Stack { screens };
        let backend = TestBackend::new(155, 45);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                f.buffer_mut().set_stringn(0, 0, format!(" {title:<154}"), 155, ratatui::style::Style::default());
                render_stack(&stack, app, f, Rect::new(0, 1, 155, 44));
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        let mut out = String::new();
        for y in 0..45 {
            for x in 0..155 {
                out.push_str(buf[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    };

    let mut log = EventLog::new();
    log.handle_key(key(KeyCode::Char('c')), &mut app);
    let s07c = snap(&app, vec![Box::new(log)], "S07c  Event Log — chronicle");
    let s10a = snap(
        &app,
        vec![Box::new(WorldMap::new("The Valley of Sunfall".to_string())), Box::new(Controls::new())],
        "S10a  Simulation Controls — modal over world map",
    );
    for (path, text) in [("docs/screens/renders/S07c.txt", s07c), ("docs/screens/renders/S10a.txt", s10a)] {
        std::fs::write(path, text).unwrap_or_else(|e| panic!("write {path}: {e}"));
    }
}

/// The status-bar clock is the day/night cue only under `StatusText`: moon blue at
/// night, accent otherwise. The text itself always reports the real sky.
#[test]
fn clock_status_colour_by_tint_mode() {
    use crate::sim::Time;
    use crate::theme;
    use crate::ui::style::clock_status;
    let day = Time::new(8, 30, 24, 6, 20);
    let night = Time::new(22, 30, 24, 6, 20);
    assert!(!day.is_night() && night.is_night());
    for mode in [DayNightTint::Off, DayNightTint::Map, DayNightTint::StatusText] {
        let (text, fg) = clock_status(&day, mode);
        assert!(text.ends_with(" day"), "{text}");
        assert_eq!(fg, theme::ACCENT, "{mode:?} by day");
    }
    for mode in [DayNightTint::Off, DayNightTint::Map] {
        let (text, fg) = clock_status(&night, mode);
        assert!(text.ends_with(" night"), "{text}");
        assert_eq!(fg, theme::ACCENT, "{mode:?} at night");
    }
    let (text, fg) = clock_status(&night, DayNightTint::StatusText);
    assert!(text.ends_with(" night"), "{text}");
    assert_eq!(fg, theme::INFO);
    assert_eq!(DayNightTint::Off.next(), DayNightTint::Map);
    assert_eq!(DayNightTint::Map.next(), DayNightTint::StatusText);
    assert_eq!(DayNightTint::StatusText.next(), DayNightTint::Off);
}

/// C9: a batch that crosses a season boundary pushes one template entry; no
/// gateway is involved with AI off.
#[test]
fn chronicle_template_pushed_at_season_boundary() {
    use crate::sim::chronicle::Source;
    let mut app = AppState::new(Params::default());
    app.sim = Some(Sim::new(7, Params::default()));
    let season_ticks = 90 * 24;
    let mut ran = 0;
    while ran < season_ticks + 200 {
        app.step_ticks(200);
        ran += 200;
        app.enqueue_chronicle();
        assert!(!app.pump_ai());
    }
    let sim = app.sim.as_ref().unwrap();
    assert_eq!(sim.chronicle.len(), 1, "one season ended");
    let e = &sim.chronicle[0];
    assert_eq!((e.year, e.season), (1, crate::sim::Season::Spring));
    assert_eq!(e.source, Source::Template);
    assert!(e.text.contains("births"), "{}", e.text);
    assert!(app.season_ended.is_none());
}

#[cfg(feature = "ai")]
mod ai {
    use std::time::{Duration, Instant};

    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::layout::Rect;
    use ratatui::Terminal;

    use crate::ai::fake::Fake;
    use crate::ai::{Ai, Feature, Status};
    use crate::sim::chronicle::Source;
    use crate::sim::params::AiConfig;
    use crate::sim::{Params, Sim};
    use crate::ui::app::AppState;
    use crate::ui::config;
    use crate::ui::screens::s07_log::EventLog;
    use crate::ui::screens::s09_worldgen::WorldGen;
    use crate::ui::screens::s10_controls::Controls;
    use crate::ui::screens::{Action, Screen};

    fn key(c: KeyCode) -> KeyEvent {
        KeyEvent::new(c, KeyModifiers::NONE)
    }

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("simf-ui-ai-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sim-fortress")).unwrap();
        dir
    }

    fn write_ui_toml(dir: &std::path::Path, cfg: &AiConfig) {
        let ui = crate::sim::params::UiParams { ai: cfg.clone(), ..Default::default() };
        config::save_ui_to(&dir.join("sim-fortress").join("ui.toml"), &ui).unwrap();
    }

    fn render(app: &AppState, screen: &dyn Screen) -> String {
        let backend = TestBackend::new(155, 45);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| screen.render(app, f, Rect::new(0, 0, 155, 45))).unwrap();
        let buf = terminal.backend().buffer();
        (0..45).map(|y| (0..155).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>() + "\n").collect()
    }

    /// Two seasons of stepping through the app's own batch loop, pumping replies.
    fn two_seasons(app: &mut AppState) {
        let mut ran = 0;
        while ran < 2 * 90 * 24 + 200 {
            app.step_ticks(200);
            ran += 200;
            app.enqueue_chronicle();
            app.pump_ai();
        }
    }

    /// Visit every screen the app has, as R1 asks, and draw each once.
    fn visit_every_screen(app: &mut AppState) {
        let mut log = EventLog::new();
        render(app, &log);
        log.handle_key(key(KeyCode::Char('c')), app);
        render(app, &log);
        let mut gen = WorldGen::from_params(&Params::default());
        gen.handle_key(key(KeyCode::Tab), app);
        render(app, &gen);
        render(app, &Controls::new());
    }

    fn wait_until(app: &mut AppState, pred: impl Fn(&AppState) -> bool) -> bool {
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(8) {
            app.pump_ai();
            if pred(app) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(15));
        }
        false
    }

    /// R1: no `[ai]` table (a reachable gateway is not a signal): zero requests.
    #[test]
    fn ai_off_makes_no_requests() {
        let _env = config::env_lock();
        let fake = Fake::start("happy").unwrap();
        let dir = scratch("off");
        let mut cfg = fake.config("fake/m", "fake/m", 5);
        cfg.enabled = false;
        write_ui_toml(&dir, &cfg);
        std::env::set_var("XDG_CONFIG_HOME", &dir);
        let mut app = AppState::new(Params::default());
        app.ai = Ai::start(&config::load_ai());
        assert!(!app.ai.is_on());
        app.sim = Some(Sim::new(7, Params::default()));
        two_seasons(&mut app);
        visit_every_screen(&mut app);
        assert_eq!(app.sim.as_ref().unwrap().chronicle.len(), 2);
        assert!(app.sim.as_ref().unwrap().chronicle.iter().all(|e| e.source == Source::Template));
        assert!(fake.requests().is_empty());
    }

    /// R1: the master on with no feature naming a model: probe only, no request.
    #[test]
    fn ai_master_on_without_features_makes_no_requests() {
        let _env = config::env_lock();
        let fake = Fake::start("happy").unwrap();
        let dir = scratch("nofeat");
        write_ui_toml(&dir, &fake.config("", "", 5));
        std::env::set_var("XDG_CONFIG_HOME", &dir);
        let mut app = AppState::new(Params::default());
        app.ai = Ai::start(&config::load_ai());
        assert!(app.ai.is_on());
        assert!(!app.ai.feature_on(Feature::Chronicle));
        app.sim = Some(Sim::new(7, Params::default()));
        two_seasons(&mut app);
        visit_every_screen(&mut app);
        assert!(wait_until(&mut app, |a| a.ai.status() == Status::Ready));
        assert!(fake.requests().is_empty(), "{:?}", fake.requests());
        assert!(app.chronicle_pending.is_none());
    }

    /// R2: enabled but unreachable: the map is reached, the sim steps, the
    /// chronicle keeps its template entries and the status note says offline.
    #[test]
    fn ai_unreachable_reaches_map_and_steps() {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://127.0.0.1:{}/v1", l.local_addr().unwrap().port());
        drop(l);
        let mut cfg = AiConfig { enabled: true, base_url: url, timeout_secs: 2, ..AiConfig::default() };
        cfg.features.chronicle = "fake/m".into();
        let mut app = AppState::new(Params::default());
        app.ai = Ai::start(&cfg);
        app.sim = Some(Sim::new(7, Params::default()));
        two_seasons(&mut app);
        assert!(wait_until(&mut app, |a| a.chronicle_pending.is_none() && a.ai_notice.is_some()));
        assert_eq!(app.ai.status_note(), "AI: offline");
        let sim = app.sim.as_ref().unwrap();
        assert_eq!(sim.chronicle.len(), 2);
        assert!(sim.chronicle.iter().all(|e| e.source == Source::Template && e.text.contains("births")));
        let text = render(&app, &crate::ui::screens::s01_map::WorldMap::new("Test".into()));
        assert!(text.contains("AI: offline"), "{text}");
    }

    /// The chronicle arrives streamed from the fake and replaces the template.
    #[test]
    fn chronicle_streams_in_from_the_fake() {
        let fake = Fake::start("happy").unwrap();
        let mut app = AppState::new(Params::default());
        app.ai = Ai::start(&fake.config("fake/m", "", 5));
        app.sim = Some(Sim::new(7, Params::default()));
        let mut ran = 0;
        while ran < 90 * 24 + 200 {
            app.step_ticks(200);
            ran += 200;
            app.enqueue_chronicle();
        }
        assert!(app.chronicle_pending.is_some());
        assert!(wait_until(&mut app, |a| a.sim.as_ref().unwrap().chronicle[0].source == Source::Model));
        let e = &app.sim.as_ref().unwrap().chronicle[0];
        assert!(e.text.contains("\"quiet valley\""), "sanitised: {}", e.text);
        assert!(!e.text.contains('\u{201c}'));
        assert_eq!(fake.requests().len(), 1);
        let body = &fake.requests()[0]["body"];
        assert_eq!(body["stream"], true);
        assert!(body["messages"][1]["content"].as_str().unwrap().starts_with("Year 1, Spring."));
        let mut log = EventLog::new();
        log.handle_key(key(KeyCode::Char('c')), &mut app);
        let text = render(&app, &log);
        assert!(text.contains("Year 1, Spring") && text.contains("chronicled") && text.contains("quiet valley"), "{text}");
    }

    /// Open the S09 designer modal from the form's last button.
    fn open_designer(gen: &mut WorldGen, app: &mut AppState) -> Box<dyn Screen> {
        // The form opens on Map width (focus 2); three BackTabs reach the last
        // field, which is the designer button while the feature is on.
        for _ in 0..3 {
            gen.handle_key(key(KeyCode::BackTab), app);
        }
        match gen.handle_key(key(KeyCode::Enter), app) {
            Action::Push(m) => m,
            other => panic!("the designer button should open the modal, got {other:?}"),
        }
    }

    /// Type a sentence, send it, and render until the preview names `boar`.
    fn design_boar(modal: &mut Box<dyn Screen>, app: &mut AppState, sentence: &str) -> String {
        for c in sentence.chars() {
            modal.handle_key(key(KeyCode::Char(c)), app);
        }
        modal.handle_key(key(KeyCode::Enter), app);
        let start = Instant::now();
        let mut shown = String::new();
        while start.elapsed() < Duration::from_secs(8) && !shown.contains("validated: boar") {
            app.pump_ai();
            shown = render(app, modal.as_ref());
            std::thread::sleep(Duration::from_millis(15));
        }
        shown
    }

    /// The designer round trip: sentence in, validated overlay handed to S09,
    /// and the overlay file written under the config dir.
    #[test]
    fn designer_modal_round_trip() {
        let _env = config::env_lock();
        let fake = Fake::start("happy").unwrap();
        let dir = scratch("designer");
        std::env::set_var("XDG_CONFIG_HOME", &dir);
        let mut app = AppState::new(Params::default());
        app.ai = Ai::start(&fake.config("", "fake/m", 5));
        let mut gen = WorldGen::from_params(&Params::default());
        let text = render(&app, &gen);
        assert!(text.contains("[ Design species ]"), "{text}");
        let mut modal = open_designer(&mut gen, &mut app);
        let shown = design_boar(&mut modal, &mut app, "a boar");
        assert!(shown.contains("validated: boar"), "{shown}");
        assert!(shown.contains("boar / Boars"), "{shown}");
        assert!(matches!(modal.handle_key(key(KeyCode::Enter), &mut app), Action::Pop));
        assert!(app.pending_species_overlay.is_some());
        gen.handle_key(key(KeyCode::Tab), &mut app);
        let p = gen.form_params();
        assert_eq!(p.species.0.last().map(|s| s.name.as_str()), Some("boar"));
        assert_eq!(p.species.0.last().map(|s| s.initial_count), Some(30));
        let file = dir.join("sim-fortress").join("species").join("boar.toml");
        let written = std::fs::read_to_string(&file).unwrap();
        assert!(written.contains("[[species]]") && written.contains("name = \"boar\""), "{written}");
        assert_eq!(fake.requests().len(), 1);
        assert_eq!(fake.requests()[0]["body"]["response_format"]["type"], "json_schema");
    }

    /// The single correction round: the first reply fails the loader, the
    /// second passes; a third is never sent.
    #[test]
    fn designer_retries_once_on_invalid_overlay() {
        let _env = config::env_lock();
        let fake = Fake::start("invalid_then_valid").unwrap();
        let dir = scratch("designer-retry");
        std::env::set_var("XDG_CONFIG_HOME", &dir);
        let mut app = AppState::new(Params::default());
        app.ai = Ai::start(&fake.config("", "fake/m", 5));
        let mut gen = WorldGen::from_params(&Params::default());
        let mut modal = open_designer(&mut gen, &mut app);
        let shown = design_boar(&mut modal, &mut app, "x");
        assert!(shown.contains("validated: boar"), "{shown}");
        let reqs = fake.requests();
        assert_eq!(reqs.len(), 2, "exactly one correction round");
        let second = reqs[1]["body"]["messages"][1]["content"].as_str().unwrap();
        assert!(second.contains("rejected by the loader"), "{second}");
    }
}
