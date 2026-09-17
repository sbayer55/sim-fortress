//! S03 — the genome column: traits, derived values and the offspring forecast.

use crate::sim::creatures::{adult_age_days, Creature};
use crate::sim::{Genome, Kind, Sim, TRAIT_NAMES};
use crate::widgets::Constraint::{Fill, Fixed};
use crate::widgets::{Bar, Block, Columns, RangeBar, Row, Rows, Spacer, Text};
use crate::{glyphs, theme};
use super::style::{blank, delta_style, line, one, section, sp, trait_color};

/// The genome column's rows.
pub(super) fn rows(sim: &Sim, c: &Creature) -> Rows<'static> {
    let stats = &sim.species[c.species.index()];
    let (mean, min, max) = (stats.mean, stats.min, stats.max);
    let mut rows = vec![one(format!(" {:<11}{:^12} {:<4} {:<6} {}", "trait", "individual", "own", "delta", "species range"), theme::dim_text())];
    // One row per trait (C8 widened the genome to eleven): label, own bar, value,
    // delta against the species mean, then the species min/mean/max range.
    let cols = Columns::new(&[Fixed(12), Fixed(12), Fixed(1), Fixed(4), Fixed(1), Fixed(6), Fixed(1), Fixed(11), Fill(1)]);
    let table: Vec<Row<'static>> = (0..Genome::LEN)
        .map(|t| {
            let v = c.genome.0[t];
            let d = v - mean.0[t];
            let color = trait_color(t);
            Row::new()
                .cell(Text::new(format!(" {:<11}", TRAIT_NAMES[t])))
                .cell(Bar::new(v).color(color))
                .cell(Spacer::cols(1))
                .cell(Text::new(format!("{v:.2}")))
                .cell(Spacer::cols(1))
                .cell(Text::new(format!("{}{:+.2}", glyphs::PLUS_MINUS, d)).style(delta_style(d)))
                .cell(Spacer::cols(1))
                .cell(RangeBar::new(min.0[t], mean.0[t], max.0[t]).color(color).width(11))
        })
        .collect();
    rows.push(Box::new(Block::new(cols).rows(table)));
    rows.push(blank(1));

    rows.push(section("Mutation history"));
    if c.mutations.is_empty() {
        rows.push(one(" none recorded", theme::dim_text()));
    }
    for m in &c.mutations {
        rows.push(one(format!(" {} {} {:+.2} (gen {})", glyphs::MUTATION, TRAIT_NAMES[m.trait_idx], m.delta, m.generation), theme::text()));
    }
    rows.push(one(format!(" from {} lines; rate {:.2} per trait per birth", c.generation, sim.params.genetics.mutation_rate), theme::dim_text()));
    rows.push(blank(1));

    rows.extend(derived(sim, c));
    rows.extend(forecast(sim, c, &mean));
    rows
}

/// The derived-trait table (sense, speed, food need, maturity, ...).
fn derived(sim: &Sim, c: &Creature) -> Rows<'static> {
    let g = &c.genome;
    let social = &sim.params.social;
    let group_word = match sim.roster().kind(c.species) {
        Kind::Prey => {
            if social.herding(g.sociality(), c.kin_nearby) {
                "herd"
            } else {
                "scattered"
            }
        }
        Kind::Predator => {
            if social.herding(g.sociality(), c.kin_nearby) {
                "pack"
            } else {
                "alone"
            }
        }
    };
    let derived: Vec<(String, String)> = vec![
        ("sense range".into(), format!("{} cells", g.sense_cells())),
        ("move speed".into(), format!("{:.1} cells/tick", 0.5 + g.speed() * 2.0)),
        ("daily food need".into(), format!("{:.2} biomass", 24.0 * sim.params.creatures.hunger_per_hour(g.size(), g.metabolism(), 1.0))),
        ("adult at".into(), format!("{} days", adult_age_days(sim.species_params(c.species), g, &sim.params.genetics))),
        ("max lifespan".into(), format!("{} days", c.max_age_days(&sim.params.creatures, &sim.params.genetics))),
        (
            "litter size".into(),
            format!(
                "{} (fert {:.2}, mat {:.2})",
                sim.params.genetics.litter_size(sim.species_params(c.species).litter_max, g.fertility(), g.maturity()),
                g.fertility(),
                g.maturity()
            ),
        ),
        ("mate cooldown".into(), format!("{} days", sim.species_params(c.species).mate_cooldown_days)),
        ("resistance cost".into(), format!("+{} % food", crate::cast!((100.0 * sim.params.disease.resist_hunger_cost * g.resistance()).round() => u32))),
        ("kin nearby".into(), format!("{} ({})", c.kin_nearby, group_word)),
    ];
    let mut rows = vec![section("Derived")];
    for (k, v) in derived {
        rows.push(line(vec![sp(format!(" {k:<18}"), theme::dim_text()), sp(v, theme::text())]));
    }
    rows.push(blank(1));
    rows
}

/// The offspring-trait forecast against an average mate.
fn forecast(sim: &Sim, c: &Creature, mean: &Genome) -> Rows<'static> {
    // Offspring forecast (C4 FR10): with an average mate, each trait is drawn
    // from either parent and mutates with sd `mutation_strength`.
    let sd = sim.params.genetics.mutation_strength;
    let table: Vec<Row<'static>> = (0..Genome::LEN)
        .map(|t| {
            let a = c.genome.0[t];
            let b = mean.0[t];
            let centre = (a + b) / 2.0;
            let lo = a.min(b) - sd;
            let hi = a.max(b) + sd;
            Row::new()
                .cell(Text::new(format!(" {:<12}", TRAIT_NAMES[t])))
                .cell(RangeBar::new(lo.max(0.0), centre, hi.min(1.0)).color(trait_color(t)).width(22).text())
        })
        .collect();
    vec![section("Offspring forecast (with an average mate)"), Box::new(Block::new(Columns::new(&[Fixed(13), Fill(1)])).rows(table))]
}
