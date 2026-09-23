//! S03 — the genome column: traits, derived values and the offspring forecast.

use crate::sim::creatures::Creature;
use crate::sim::{Genome, Kind, Sim, TRAIT_NAMES};
use crate::widgets::Constraint::{Fill, Fixed};
use crate::widgets::{Bar, Block, Columns, RangeBar, Row, Rows, Spacer, Text};
use crate::{glyphs, theme};
use super::style::{delta_style, line, one, section, sp, trait_color};

/// The genome column's rows.
pub(super) fn rows(sim: &Sim, c: &Creature) -> Rows<'static> {
    let stats = &sim.species[c.species.index()];
    let (mean, min, max) = (stats.mean, stats.min, stats.max);
    let mut rows = vec![one(format!(" {:<13}{:^12} {:<4} {:<6} {}", "trait", "individual", "own", "delta", "sp. range"), theme::dim_text())];
    // One row per trait (thirteen since Diet breadth, whose name sets the
    // 12-cell label): label, own bar, value, delta against the species mean,
    // then the species min/mean/max range.
    let cols = Columns::new(&[Fixed(14), Fixed(12), Fixed(1), Fixed(4), Fixed(1), Fixed(6), Fixed(1), Fixed(11), Fill(1)]);
    let table: Vec<Row<'static>> = (0..Genome::LEN)
        .map(|t| {
            let v = c.genome.0[t];
            let d = v - mean.0[t];
            let color = trait_color(t);
            Row::new()
                .cell(Text::new(format!(" {:<13}", TRAIT_NAMES[t])))
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
    // No spacers anywhere in this column: thirteen traits, their forecast rows
    // and the diet line fill the 41 rows exactly, and the section rules
    // separate well enough.
    rows.push(section(if c.mutations.is_empty() { "Mutation history: none recorded" } else { "Mutation history" }));
    for m in &c.mutations {
        rows.push(one(format!(" {} {} {:+.2} (gen {})", glyphs::MUTATION, TRAIT_NAMES[m.trait_idx], m.delta, m.generation), theme::text()));
    }
    // This animal's own effective numbers (paired with an average mate), not
    // the world setting: Mutability scales both.
    let (rate, sd) = sim.params.genetics.effective_mutation((c.genome.mutability() + mean.mutability()) / 2.0);
    rows.push(one(format!(" from {} lines; rate {rate:.3}  sd {sd:.3}  mut {:.2}", c.generation, c.genome.mutability()), theme::dim_text()));

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
        ("adult at".into(), format!("{} days", c.adult_age_days(sim.species_params(c.species), &sim.params.genetics))),
        ("max lifespan".into(), format!("{} days", c.max_age_days(&sim.params.creatures, &sim.params.genetics))),
        (
            "litter size".into(),
            if c.sterile {
                "sterile: never breeds".into()
            } else {
                format!(
                    "{} (fert {:.2}, mat {:.2})",
                    sim.params.genetics.litter_size(sim.species_params(c.species).litter_max, g.fertility(), g.maturity()),
                    g.fertility(),
                    g.maturity()
                )
            },
        ),
        ("mate cooldown".into(), format!("{} days", sim.species_params(c.species).mate_cooldown_days)),
        ("resistance cost".into(), format!("+{} % food", crate::cast!((100.0 * sim.params.disease.resist_hunger_cost * g.resistance()).round() => u32))),
        ("kin nearby".into(), format!("{} ({})", c.kin_nearby, group_word)),
        ("diet".into(), diet_line(sim, c)),
    ];
    let mut rows = vec![section("Derived")];
    for (k, v) in derived {
        rows.push(line(vec![sp(format!(" {k:<18}"), theme::dim_text()), sp(v, theme::text())]));
    }
    rows
}

/// Diet breadth as the reader sees it: the furthest terrain this animal eats
/// fully and its bite rate. Predators never graze, so theirs is inert.
fn diet_line(sim: &Sim, c: &Creature) -> String {
    if sim.roster().kind(c.species) == Kind::Predator {
        return "carnivore (breadth inert)".into();
    }
    let diet = &sim.params.diet;
    let b = c.genome.diet_breadth();
    let reach = diet.reach_name(b).unwrap_or("nothing fully");
    format!("grazes up to {reach}; bite x{:.2}", diet.bite(b))
}

/// The offspring-trait forecast against an average mate.
fn forecast(sim: &Sim, c: &Creature, mean: &Genome) -> Rows<'static> {
    // Offspring forecast (C4 FR10): with an average mate, each trait is drawn
    // from either parent and mutates with the pair's effective sd (the
    // parents' mean Mutability scales `mutation_strength`).
    let (_, sd) = sim.params.genetics.effective_mutation((c.genome.mutability() + mean.mutability()) / 2.0);
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
