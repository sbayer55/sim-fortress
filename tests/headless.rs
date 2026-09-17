//! Integration tests exercising the library target (the same surface the
//! `--headless` binary uses), and, for the C9 AI flags, the binary itself
//! (ai-requirements R10: exit codes are the contract).

// Test crates are separate compilation roots, so they do not inherit the allow
// list in `src/lib.rs`. The same `indexing_slicing` justification applies here
// (indices come from checked `0..len()` loops over fixed-size arrays), and the
// subprocess helpers below are test scaffolding where a panic is the failure.
#![allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use sim_fortress::sim::{Params, Sim};

#[test]
fn lib_determinism() {
    let mut a = Sim::new(1234, Params::default());
    let mut b = Sim::new(1234, Params::default());
    for _ in 0..10_000 {
        a.step();
        b.step();
    }
    assert_eq!(a.checksum(), b.checksum());
}

#[test]
fn params_round_trip_via_lib() {
    let p = Params::default();
    let text = p.to_toml().unwrap();
    assert_eq!(Params::from_toml(&text).unwrap(), p);
}

#[test]
fn season_events_are_emitted() {
    let mut sim = Sim::new(42, Params::default());
    for _ in 0..4320 {
        sim.step();
    }
    // The oldest event is the Spring season announcement; Summer follows.
    let seasons: Vec<_> = sim.events.iter().filter(|e| e.kind == sim_fortress::sim::EventKind::Season).collect();
    assert!(seasons.len() >= 2, "expected at least Spring and Summer season events, got {}", seasons.len());
    assert_eq!(seasons[0].text, "Spring returns to the valley; regrowth quickens");
}

use std::path::PathBuf;
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sim-fortress"))
}

/// A scratch dir with `sim-fortress/` inside, usable as `XDG_CONFIG_HOME` and as cwd.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("simf-headless-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("sim-fortress")).unwrap();
    dir
}

fn write_ui_toml(dir: &std::path::Path, base_url: &str, chronicle: &str, designer: &str) {
    let text = format!(
        "[ai]\nenabled = true\nbase_url = \"{base_url}\"\ntimeout_secs = 5\n\n[ai.features]\nchronicle = \"{chronicle}\"\ndesigner = \"{designer}\"\n"
    );
    std::fs::write(dir.join("sim-fortress").join("ui.toml"), text).unwrap();
}

fn closed_port_url() -> String {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    drop(l);
    format!("http://127.0.0.1:{port}/v1")
}

/// R10: an AI flag with no reachable gateway exits non-zero with one line; it
/// never hangs and never silently falls back. Holds in both builds: without
/// the feature the line says so, with it the line names the URL.
#[test]
fn ai_flag_without_gateway_exits_nonzero() {
    let dir = scratch("nogateway");
    let url = closed_port_url();
    write_ui_toml(&dir, &url, "fake/m", "fake/m");
    let out = bin()
        .args(["--headless", "--seed", "1", "--ticks", "24", "--chronicle"])
        .env("XDG_CONFIG_HOME", &dir)
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(err.trim().lines().count(), 1, "one line: {err}");
    assert!(err.contains("built without the ai feature") || err.contains(&url), "{err}");
    assert!(!dir.join("chronicle.md").exists());

    let out = bin().args(["--design-species", "a boar"]).env("XDG_CONFIG_HOME", &dir).current_dir(&dir).output().unwrap();
    assert!(!out.status.success());

    // AI off in ui.toml: the same shape of failure.
    std::fs::write(dir.join("sim-fortress").join("ui.toml"), "[ai]\nenabled = false\n").unwrap();
    let out = bin().args(["--headless", "--seed", "1", "--ticks", "24", "--chronicle"]).env("XDG_CONFIG_HOME", &dir).current_dir(&dir).output().unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("not enabled") || err.contains("built without the ai feature"), "{err}");
}

#[cfg(feature = "ai")]
mod with_gateway {
    use super::*;
    use sim_fortress::ai::fake::Fake;
    use sim_fortress::sim::Params;

    /// R10: `--seeds`, `--summary`, `--row` and a plain headless run never
    /// touch the network, whatever ui.toml says.
    #[test]
    fn ai_flags_absent_runs_are_offline() {
        let fake = Fake::start("happy").unwrap();
        let dir = scratch("absent");
        write_ui_toml(&dir, &fake.url(), "fake/m", "fake/m");
        for args in [
            vec!["--headless", "--seed", "1", "--ticks", "2400"],
            vec!["--seeds", "1-2", "--ticks", "24", "--summary"],
            vec!["--headless", "--seed", "3", "--ticks", "24", "--row"],
            vec!["--header"],
        ] {
            let out = bin().args(&args).env("XDG_CONFIG_HOME", &dir).current_dir(&dir).output().unwrap();
            assert!(out.status.success(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
        }
        assert!(fake.requests().is_empty(), "{:?}", fake.requests());
    }

    /// `--chronicle` narrates each completed season and writes Markdown.
    #[test]
    fn chronicle_writes_markdown_against_fake() {
        let fake = Fake::start("happy").unwrap();
        let dir = scratch("chronicle");
        write_ui_toml(&dir, &fake.url(), "fake/m", "");
        let md = dir.join("run.md");
        let out = bin()
            .args(["--headless", "--seed", "1", "--ticks", "2200", "--chronicle", "--out", md.to_str().unwrap()])
            .env("XDG_CONFIG_HOME", &dir)
            .current_dir(&dir)
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let text = std::fs::read_to_string(&md).unwrap();
        assert!(text.contains("## Year 1, Spring"), "{text}");
        assert!(text.contains("quiet valley"), "{text}");
        assert!(text.contains("births"), "the tally follows the entry: {text}");
        assert_eq!(fake.requests().len(), 1, "one season completed in 2200 ticks");
        assert_eq!(fake.requests()[0]["feature"], "chronicle");

        // Without a model for the feature the flag names the key and exits 1.
        write_ui_toml(&dir, &fake.url(), "", "");
        let out = bin().args(["--headless", "--seed", "1", "--ticks", "24", "--chronicle"]).env("XDG_CONFIG_HOME", &dir).current_dir(&dir).output().unwrap();
        assert!(!out.status.success());
        assert!(String::from_utf8_lossy(&out.stderr).contains("ai.features.chronicle"));
    }

    /// `--design-species` writes an overlay the ordinary loader accepts, which
    /// then runs through `--params` like a hand-written file (R4).
    #[test]
    fn design_species_writes_overlay_against_fake() {
        let fake = Fake::start("happy").unwrap();
        let dir = scratch("design");
        write_ui_toml(&dir, &fake.url(), "", "fake/m");
        let out = bin().args(["--design-species", "a boar: omnivore, big litters"]).env("XDG_CONFIG_HOME", &dir).current_dir(&dir).output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(stdout.contains("species-boar.toml"), "{stdout}");
        assert!(stdout.contains("boar / Boars"), "{stdout}");
        let file = dir.join("species-boar.toml");
        let text = std::fs::read_to_string(&file).unwrap();
        let mut p = Params::default();
        p.apply_overlay(&text).unwrap();
        assert_eq!(p.species.0.last().map(|s| s.name.as_str()), Some("boar"));

        let out = bin()
            .args(["--headless", "--seed", "1", "--ticks", "48", "--params", file.to_str().unwrap()])
            .env("XDG_CONFIG_HOME", &dir)
            .current_dir(&dir)
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        assert!(String::from_utf8_lossy(&out.stdout).contains("checksum="));
        assert_eq!(fake.requests().len(), 1, "the --params run made no request");
    }

    /// The single correction round, headless: two requests, then success.
    #[test]
    fn design_species_retries_once() {
        let fake = Fake::start("invalid_then_valid").unwrap();
        let dir = scratch("design-retry");
        write_ui_toml(&dir, &fake.url(), "", "fake/m");
        let out = bin().args(["--design-species", "x", "--out", "boar.toml"]).env("XDG_CONFIG_HOME", &dir).current_dir(&dir).output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        assert!(String::from_utf8_lossy(&out.stderr).contains("rejected"), "the first rejection is reported");
        assert_eq!(fake.requests().len(), 2);
        assert!(dir.join("boar.toml").exists());

        let malformed = Fake::start("malformed").unwrap();
        write_ui_toml(&dir, &malformed.url(), "", "fake/m");
        let out = bin().args(["--design-species", "x"]).env("XDG_CONFIG_HOME", &dir).current_dir(&dir).output().unwrap();
        assert!(!out.status.success());
        assert_eq!(malformed.requests().len(), 2, "one correction round, never a third");
    }
}
