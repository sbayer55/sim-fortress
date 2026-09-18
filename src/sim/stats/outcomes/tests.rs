use super::*;
use crate::sim::species::N_TRAITS;

/// A life whose trait 0 is `v`, every other trait 0.5.
fn life(v: f32, age: u32, fate: Fate) -> Life {
    let mut g = [0.5; N_TRAITS];
    g[0] = v;
    Life { genome: Genome(g), age, fate, crowded: Some(false), young_per_year: None, hunt_rate: None }
}

fn ramp(n: usize) -> impl Iterator<Item = f32> {
    (0..n).map(move |i| crate::cast!(i => f32) / crate::cast!(n => f32))
}

#[test]
fn lifespan_that_grows_with_the_trait_gives_r_one() {
    let lives: Vec<Life> = ramp(40).map(|v| life(v, 10 + crate::cast!((v * 100.0) => u32), Fate::Died(Cause::Age))).collect();
    let m = matrix(&lives);
    let c = m.cell(0, 0);
    assert_eq!(c.n, 40);
    assert!(c.r > 0.99, "r = {}", c.r);
    assert!(c.shown());
    // A constant trait has no variance and so no correlation.
    assert_eq!(m.cell(1, 0).r, 0.0);
    // Nobody died of disease: the column is flat, not "no link", and not shown.
    let disease = OUTCOMES.iter().position(|&o| o == Outcome::Died(Cause::Disease)).unwrap_or(99);
    assert!(m.cell(0, disease).flat && !m.cell(0, disease).shown());
}

#[test]
fn cause_columns_count_the_dead_only_and_small_n_is_hidden() {
    let mut lives: Vec<Life> = ramp(20).map(|v| life(v, 5, if v < 0.5 { Fate::Died(Cause::Predation) } else { Fate::Died(Cause::Starved) })).collect();
    lives.extend(ramp(50).map(|v| life(v, 5, Fate::Alive)));
    let m = matrix(&lives);
    let preyed = OUTCOMES.iter().position(|&o| o == Outcome::Died(Cause::Predation)).unwrap_or(99);
    let c = m.cell(0, preyed);
    assert_eq!(c.n, 20, "the living are not in a cause column");
    assert!(c.r < -0.8, "slow ones were eaten: r = {}", c.r);
    assert!(!c.shown(), "20 lives is under MIN_N");
}

#[test]
fn thirds_split_evenly_and_average_the_outcome() {
    let lives: Vec<Life> = ramp(30).map(|v| life(v, crate::cast!((v * 30.0) => u32), Fate::Died(Cause::Age))).collect();
    let cuts = third_cuts(&lives, 0);
    assert_eq!(third_of(cuts, 0.0), 0);
    assert_eq!(third_of(cuts, 0.99), 2);
    let t = thirds(&lives, 0, Outcome::Lifespan);
    assert_eq!(t.map(|s| s.n), [10, 10, 10]);
    assert!(t[0].mean < t[1].mean && t[1].mean < t[2].mean);
}

#[test]
fn age_profile_bands_the_living_and_names_the_commonest_death() {
    let mut lives: Vec<Life> = (0..10).map(|i| life(0.2, i % 5, Fate::Alive)).collect();
    lives.extend((0..10).map(|i| life(0.8, 100 + i % 3, Fate::Alive)));
    lives.extend((0..3).map(|_| life(0.5, 0, Fate::Died(Cause::Predation))));
    lives.push(life(0.5, 1, Fate::Died(Cause::Starved)));
    let p = age_profile(&lives, 0);
    assert!(p.band_days >= 5);
    assert_eq!(p.bands[0].living, 10);
    let near = |a: Option<f32>, b: f32| a.is_some_and(|a| (a - b).abs() < 1e-5);
    assert!(near(p.bands[0].trait_mean, 0.2));
    assert_eq!(p.bands[0].top, Some((Cause::Predation, 3)));
    let old = p.bands.iter().rev().find(|b| b.living > 0).and_then(|b| b.trait_mean);
    assert!(near(old, 0.8));
    let trend = age_trend(&lives, 0);
    assert!(near(trend.map(|t| t.young), 0.2) && near(trend.map(|t| t.old), 0.8));
    assert!(trend.is_some_and(AgeTrend::significant), "a .2 against .8 split is not noise");
    assert_eq!(age_trend(lives.get(..19).unwrap_or(&[]), 0), None, "19 living is too few");
}

#[test]
fn strongest_orders_by_size_and_skips_hidden_cells() {
    let lives: Vec<Life> = ramp(40).map(|v| life(v, crate::cast!((v * 50.0) => u32), Fate::Died(Cause::Age))).collect();
    let top = strongest(&matrix(&lives), 3);
    assert_eq!(top.first().map(|x| (x.0, x.1)), Some((0, 0)));
    assert!(top.windows(2).all(|w| w[0].2.abs() >= w[1].2.abs()));
}

#[test]
fn crowding_admits_by_flag_and_unknown_days_only_in_all() {
    assert!(Crowding::All.admits(None));
    assert!(!Crowding::Crowded.admits(None));
    assert!(Crowding::Crowded.admits(Some(true)));
    assert!(Crowding::Sparse.admits(Some(false)));
    assert_eq!(Crowding::All.next().next().next(), Crowding::All);
    assert_eq!(median(&[3, 1, 2]), Some(2.0));
    assert_eq!(median(&[4, 1, 2, 3]), Some(2.5));
    assert_eq!(median(&[]), None);
}

/// End to end on a real world: the lives of a species are its living members
/// plus its logged deaths, and the crowded and sparse halves partition the
/// lives whose day is in the series.
#[test]
fn collect_lives_reads_the_living_and_the_log() {
    let mut sim = Sim::new(7, crate::sim::Params::default());
    for _ in 0..24 * 90 {
        sim.step();
    }
    let sp = SpeciesId::from_index(0);
    let all = collect_lives(&sim, sp, Crowding::All);
    let living = sim.creatures.living().filter(|c| c.species == sp).count();
    let logged = sim.lineage.lives().records().filter(|r| r.species == sp).count();
    assert_eq!(all.len(), living + logged);
    let known = all.iter().filter(|l| l.crowded.is_some()).count();
    let split = collect_lives(&sim, sp, Crowding::Crowded).len() + collect_lives(&sim, sp, Crowding::Sparse).len();
    assert_eq!(split, known);
    assert!(all.iter().filter(|l| l.dead()).all(|l| Outcome::Lifespan.value(l).is_some()));
}

/// A trait that does not change with age: the fifths differ only by noise, so
/// the gap stays inside two standard errors.
#[test]
fn an_age_trend_from_noise_is_not_significant() {
    let wobble = [0.40, 0.50, 0.45, 0.55, 0.42, 0.48, 0.52, 0.46, 0.44, 0.58];
    let lives: Vec<Life> = (0..100u32).map(|i| life(wobble[crate::cast!(i % 10 => usize)], i, Fate::Alive)).collect();
    let t = age_trend(&lives, 0);
    assert!(t.is_some_and(|t| !t.significant()), "{t:?}");
}

#[test]
fn old_age_is_neutral_and_the_rest_have_a_side() {
    assert_eq!(Outcome::Died(Cause::Age).tone(), Tone::Neutral);
    assert_eq!(Outcome::Lifespan.tone(), Tone::Good);
    assert_eq!(Outcome::Died(Cause::Predation).tone(), Tone::Bad);
}
