//! Integration tests exercising the library target (the same surface the
//! `--headless` binary uses).

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
