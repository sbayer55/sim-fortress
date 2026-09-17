//! S10 / Options modal (C6 FR5): drawn over the dimmed world map (or the title
//! screen).
//!
//! The Options section is six rows and persists to `ui.toml`; the AI section
//! (C9) is the master switch, the gateway status and one row per feature.

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::Frame;

use crate::ai::{Ai, Feature};
use crate::sim::params::DayNightTint;
use crate::ui::app::AppState;
use crate::ui::config;
use crate::ui::screens::common::sp;
use crate::ui::screens::{Action, Screen};
use crate::ui::style::{clock_status, SeasonStyle};
use crate::widgets::Constraint::Fill;
use crate::widgets::{Checkbox, Component, Divider, HStack, KeyHint, Modal, Rows, Spacer, StatusBar, Stepper, Text, VStack};
use crate::{glyphs, theme};

const SPEEDS: [u32; 5] = [1, 2, 5, 10, 25];
const STEPS: [(&str, u64); 3] = [("1 tick", 1), ("6 hours", 6), ("1 day", 24)];

#[derive(Debug)]
pub struct Controls {
    step_idx: usize,
}

impl Default for Controls {
    fn default() -> Self {
        Self::new()
    }
}

impl Controls {
    pub const fn new() -> Self {
        Self { step_idx: 0 }
    }
}

/// Persist the current UI options to `ui.toml` (best-effort; failures only log).
fn persist(app: &AppState) {
    if let Err(e) = config::save_ui(&app.params.ui) {
        eprintln!("ui.toml write error: {e}");
    }
}

impl Screen for Controls {
    fn opaque(&self) -> bool {
        false
    }

    fn handle_key(&mut self, key: KeyEvent, app: &mut AppState) -> Action {
        match key.code {
            KeyCode::Esc => Action::Pop,
            KeyCode::Char(' ') => {
                app.toggle_pause();
                Action::None
            }
            KeyCode::Char('+') => {
                app.speed_up();
                Action::None
            }
            KeyCode::Char('-') => {
                app.speed_down();
                Action::None
            }
            KeyCode::Char(c @ '1'..='5') => {
                app.set_speed(crate::cast!(c.to_digit(10).unwrap_or(1) - 1 => usize));
                Action::None
            }
            KeyCode::Char('.') => {
                app.paused = true;
                app.step_ticks(STEPS[self.step_idx].1);
                Action::None
            }
            KeyCode::Char('<') => {
                self.step_idx = self.step_idx.saturating_sub(1);
                Action::None
            }
            KeyCode::Char('>') => {
                if self.step_idx + 1 < STEPS.len() {
                    self.step_idx += 1;
                }
                Action::None
            }
            KeyCode::Char('a') => {
                app.params.ui.auto_pause_on_extinction = !app.params.ui.auto_pause_on_extinction;
                persist(app);
                Action::None
            }
            KeyCode::Char('b') => {
                app.params.ui.log_births = !app.params.ui.log_births;
                persist(app);
                Action::None
            }
            KeyCode::Char('c') => {
                app.params.ui.pause_on_follow_death = !app.params.ui.pause_on_follow_death;
                persist(app);
                Action::None
            }
            KeyCode::Char('t') => {
                app.params.ui.day_night_tint = app.params.ui.day_night_tint.next();
                persist(app);
                Action::None
            }
            KeyCode::Char('d') => {
                app.params.ui.auto_pause_on_epidemic = !app.params.ui.auto_pause_on_epidemic;
                persist(app);
                Action::None
            }
            KeyCode::Char('m') => {
                // The master switch (ai-requirements R1/R7): persist first, then
                // rebuild the handle from ui.toml alone. Dropping the old handle
                // stops the worker and discards in-flight replies.
                app.params.ui.ai.enabled = !app.params.ui.ai.enabled;
                persist(app);
                app.chronicle_pending = None;
                app.ai = Ai::start(&config::load_ai());
                Action::None
            }
            KeyCode::Left => {
                app.params.ui.autosave_days = app.params.ui.autosave_days.saturating_sub(1);
                persist(app);
                Action::None
            }
            KeyCode::Right => {
                app.params.ui.autosave_days = app.params.ui.autosave_days.saturating_add(1).min(365);
                persist(app);
                Action::None
            }
            _ => Action::Unhandled,
        }
    }

    fn render(&self, app: &AppState, f: &mut Frame<'_>, area: Rect) {
        let hint = KeyHint::row(&[("Space", "pause"), ("+/-", "speed"), (".", "step"), ("Esc", "close")]).label_style(theme::dim_text()).center();
        let modal = Modal::new(60.min(area.width.saturating_sub(2)), 26.min(area.height.saturating_sub(2))).title("Simulation Controls").info("Esc closes").hint_with(hint);

        let running = !app.paused;
        let (state_glyph, state_txt, state_color) = if running {
            (glyphs::PLAY.to_string(), "RUNNING", theme::GOOD)
        } else {
            (glyphs::PAUSE_STR.to_string(), "PAUSED", theme::WARN)
        };
        let state = Text::spans(vec![
            sp(" state   ", theme::label()),
            sp(format!("{state_glyph} {state_txt}"), Style::default().fg(state_color).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            sp(format!("      {} x{}  ", glyphs::FAST_STR, app.speed()), theme::text()),
            sp(format!("{} {}", glyphs::PAUSE_STR, "Space toggles"), theme::dim_text()),
        ]);
        let mut spans = vec![sp(" speed   ", theme::label())];
        for s in SPEEDS {
            spans.push(sp(format!(" x{s} "), if s == app.speed() { theme::selected() } else { theme::text() }));
            spans.push(sp(" ", theme::text()));
        }
        spans.push(sp("   +/- or 1-5", theme::dim_text()));
        let speed = Text::spans(spans);
        let mut spans = vec![sp(" step    ", theme::label())];
        for (i, (s, _)) in STEPS.iter().enumerate() {
            let st = if i == self.step_idx { Style::default().fg(theme::TEXT_BRIGHT).bg(theme::PANEL_BG).add_modifier(Modifier::UNDERLINED) } else { theme::text() };
            spans.push(sp(format!(" {s} "), st));
            spans.push(sp(" ", theme::text()));
        }
        spans.push(sp(format!("   {} . steps once", glyphs::STEP_STR), theme::dim_text()));
        let step = Text::spans(spans);

        // Autosave row: `◄ N ►` days.
        let n = app.params.ui.autosave_days;
        let n_label = if n == 0 { "off".to_string() } else { format!("{n} days") };
        let (gap, autosave) = (Spacer::cols(2), Stepper::inline(format!("autosave every {n_label}")).key("[←→]"));
        let autosave_row = HStack::new().child(&gap).child_with(Fill(1), &autosave);

        let mut rows: Rows<'_> = vec![Box::new(state), Box::new(Spacer::rows(1)), Box::new(speed), Box::new(step), Box::new(Spacer::rows(1))];
        rows.extend(clock_rows(app));
        rows.extend(options_rows(app));
        rows.push(Box::new(autosave_row));
        rows.push(Box::new(Spacer::rows(1)));
        rows.extend(ai_rows(app));

        let buf = f.buffer_mut();
        let body = modal.render(buf, area);
        VStack::from_boxes(&rows).render(buf, body);

        // Repaint the status bar undimmed.
        let status_row = area.y + area.height - 1;
        let keys: &[(&str, &str)] = &[("Space", "pause"), ("+/-", "speed"), ("1-5", "set speed"), (".", "step"), ("a/b/c/t/d/m", "toggle"), ("Esc", "close")];
        let (right, right_fg) = match &app.sim {
            Some(sim) => clock_status(&sim.time, app.params.ui.day_night_tint),
            None => ("options".to_string(), theme::ACCENT),
        };
        StatusBar::new(keys).right(&right).right_color(right_fg).render(f.buffer_mut(), Rect::new(area.x, status_row, area.width, 1));
    }
}

/// The Clock section: tick, day, year and the tick-rate line.
fn clock_rows(app: &AppState) -> Rows<'static> {
    let mut rows: Rows<'static> = vec![Box::new(Divider::new("Clock"))];
    if let Some(sim) = &app.sim {
        let time = &sim.time;
        let season = time.season();
        rows.push(Box::new(Text::spans(vec![
            sp(" tick ", theme::dim_text()),
            sp(crate::ui::screens::s01_map::group(time.tick), theme::text()),
            sp("    day ", theme::dim_text()),
            sp(format!("{}", time.day_of_season()), theme::text()),
            sp(format!(" of {} ", season.name()), theme::dim_text()),
            sp(season.glyph().to_string(), Style::default().fg(season.color()).bg(theme::PANEL_BG)),
            sp("    year ", theme::dim_text()),
            sp(format!("{}", time.year()), theme::text()),
            sp(format!("    {} {}", time.hour_label(), glyphs::SUN), theme::text()),
        ])));
        rows.push(Box::new(
            Text::new(format!(" 1 tick = 1 hour   1 day = {} ticks   x{} = {} ticks/s", app.params.time.ticks_per_day, app.speed(), 2 * app.speed())).style(theme::dim_text()),
        ));
        rows.push(Box::new(Spacer::rows(1)));
    } else {
        rows.push(Box::new(Text::new(" no world loaded").style(theme::dim_text())));
        rows.push(Box::new(Spacer::rows(2)));
    }
    rows
}

/// The Options section: five Checkbox rows driven by their letters.
fn options_rows(app: &AppState) -> Rows<'static> {
    // `[t]` cycles three states; the mark is on for anything but `off`.
    let tint = app.params.ui.day_night_tint;
    let toggles: [(&str, bool, String); 5] = [
        ("a", app.params.ui.auto_pause_on_extinction, "auto-pause on extinction".into()),
        ("b", app.params.ui.log_births, "log births to the event log".into()),
        ("c", app.params.ui.pause_on_follow_death, "pause when a followed creature dies".into()),
        ("t", tint != DayNightTint::Off, format!("day/night tint: {}", tint.label())),
        ("d", app.params.ui.auto_pause_on_epidemic, "auto-pause on epidemic".into()),
    ];
    let mut rows: Rows<'static> = vec![Box::new(Divider::new("Options"))];
    for (key, on, label) in toggles {
        rows.push(Box::new(Checkbox::new(label, on).key(key)));
    }
    rows
}

/// The AI section (C9): master switch, gateway status, one row per feature
/// showing its model string (set in `ui.toml`; greyed until the master is on).
fn ai_rows(app: &AppState) -> Rows<'static> {
    let cfg = &app.params.ui.ai;
    let status = if !cfg.enabled {
        "off"
    } else if app.ai.is_on() {
        app.ai.status().label()
    } else {
        "not compiled"
    };
    let mut rows: Rows<'static> = vec![Box::new(Divider::new("AI"))];
    rows.push(Box::new(Checkbox::new(format!("AI enabled   status: {status}"), cfg.enabled).key("m")));
    let row_style = if cfg.enabled { theme::text() } else { theme::dim_text() };
    for feature in [Feature::Chronicle, Feature::Designer] {
        let model = cfg.features.model(feature.key()).map_or_else(|| "(off)".to_string(), |m| crate::ui::screens::common::clip(m, 38));
        rows.push(Box::new(Text::spans(vec![sp(format!("     {:<10} ", feature.key()), theme::label()), sp(model, row_style)])));
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::Params;
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::KeyModifiers;
    use ratatui::Terminal;

    fn render(app: &AppState) -> String {
        let backend = TestBackend::new(155, 45);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| Controls::new().render(app, f, Rect::new(0, 0, 155, 45))).unwrap();
        let buf = terminal.backend().buffer();
        (0..45)
            .map(|y| (0..155).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>() + "\n")
            .collect()
    }

    #[test]
    fn s10_epidemic_toggle() {
        // `d` persists through `save_ui`; point it at a scratch dir so the test
        // never overwrites the user's real `~/.config/sim-fortress/ui.toml`.
        let _env = config::env_lock();
        let scratch = std::env::temp_dir().join(format!("sim-fortress-s10-test-{}", std::process::id()));
        std::env::set_var("XDG_CONFIG_HOME", &scratch);

        let mut app = AppState::new(Params::default());
        app.sim = Some(crate::sim::Sim::new(7, Params::default()));
        let before = app.params.ui.auto_pause_on_epidemic;
        assert!(before, "auto-pause on epidemic defaults to on");
        let text = render(&app);
        assert!(text.contains("[x] auto-pause on epidemic"), "row missing:\n{text}");
        assert!(text.contains("[d]"));
        assert!(text.contains("[Space] pause"), "key hint row must survive the extra option row:\n{text}");

        let mut c = Controls::new();
        // `d` toggles the option; the change is what the alert gate reads.
        let a = c.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE), &mut app);
        assert!(matches!(a, Action::None));
        assert_eq!(app.params.ui.auto_pause_on_epidemic, !before);
        assert!(render(&app).contains("[ ] auto-pause on epidemic"));
        // Toggle back so the persisted ui.toml is left as it was.
        c.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE), &mut app);
        assert_eq!(app.params.ui.auto_pause_on_epidemic, before);
    }

    /// C9: `m` flips the master switch, persists it and rebuilds the handle
    /// from ui.toml; the rows read off / offline / not compiled, never a URL.
    #[test]
    fn s10_ai_master_toggle() {
        let _env = config::env_lock();
        let scratch = std::env::temp_dir().join(format!("sim-fortress-s10-ai-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&scratch);
        std::env::set_var("XDG_CONFIG_HOME", &scratch);

        let mut app = AppState::new(Params::default());
        // A port nothing listens on, so the probe never reaches a real gateway.
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        drop(l);
        app.params.ui.ai.base_url = format!("http://127.0.0.1:{port}/v1");
        app.params.ui.ai.features.chronicle = "ollama/llama3.1:8b".into();
        let text = render(&app);
        assert!(text.contains("[ ] AI enabled   status: off"), "row missing:\n{text}");
        assert!(text.contains("chronicle  ollama/llama3.1:8b"), "{text}");
        assert!(text.contains("designer   (off)"), "{text}");
        assert!(text.contains("[Space] pause"), "hint row must survive the AI section:\n{text}");

        let mut c = Controls::new();
        c.handle_key(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE), &mut app);
        assert!(app.params.ui.ai.enabled);
        let text = render(&app);
        assert!(text.contains("[x] AI enabled   status: offline") || text.contains("[x] AI enabled   status: not compiled"), "{text}");
        assert!(config::load_ai().enabled, "persisted");
        assert_eq!(app.ai.is_on(), cfg!(feature = "ai"));

        c.handle_key(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE), &mut app);
        assert!(!app.params.ui.ai.enabled);
        assert!(!app.ai.is_on());
        assert!(render(&app).contains("[ ] AI enabled   status: off"));
    }
}
