//! S03 — the genome column: traits, derived values and the offspring forecast.

use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::buffer::Buffer;
use crate::sim::creatures::{adult_age_days, Creature};
use crate::sim::{Genome, Kind, TRAIT_NAMES};
use crate::widgets::{bars, panel, util};
use crate::{glyphs, theme};
use super::style::{delta_style, sp, trait_color};

/// Draw the genome column into `inner`; returns the rows used.
pub(super) fn genome(buf: &mut Buffer, inner: Rect, sim: &crate::sim::Sim, c: &Creature) -> u16 {
    let stats = &sim.species[c.species.index()];
    let mean = stats.mean;
    let min = stats.min;
    let max = stats.max;
    let mut row = 0u16;
    util::line_in(buf, inner, row, Line::from(sp(
        format!(" {:<11}{:^12} {:<4} {:<6} {}", "trait", "individual", "own", "delta", "species range"),
        theme::dim_text(),
    )));
    row += 1;
    // One row per trait (twelve since Mutability): label, own bar, value,
    // delta against the species mean, then the species min/mean/max range.
    for t in 0..Genome::LEN {
        let v = c.genome.0[t];
        let d = v - mean.0[t];
        let color = trait_color(t);
        let y = inner.y + row;

        buf.set_stringn(inner.x, y, format!(" {:<11}", TRAIT_NAMES[t]), 12, theme::text());
        bars::bar(buf, inner.x + 12, y, 12, v, color);
        buf.set_stringn(inner.x + 25, y, format!("{v:.2}"), 4, theme::text());
        buf.set_stringn(inner.x + 30, y, format!("{}{:+.2}", glyphs::PLUS_MINUS, d), 6, delta_style(d));
        bars::range(buf, inner.x + 37, y, 11, min.0[t], mean.0[t], max.0[t], color);
        row += 1;
    }
    // No spacer here: the twelfth trait and its forecast row use the column's
    // last two spare rows, and the section rule separates well enough.

    panel::section_in(buf, inner, row, "Mutation history");
    row += 1;
    if c.mutations.is_empty() {
        util::line_in(buf, inner, row, Line::from(sp(" none recorded", theme::dim_text())));
        row += 1;
    }
    for m in &c.mutations {
        util::line_in(buf, inner, row, Line::from(sp(format!(" {} {} {:+.2} (gen {})", glyphs::MUTATION, TRAIT_NAMES[m.trait_idx], m.delta, m.generation), theme::text())));
        row += 1;
    }
    // This animal's own effective numbers (paired with an average mate), not
    // the world setting: Mutability scales both.
    let (rate, sd) = sim.params.genetics.effective_mutation((c.genome.mutability() + mean.mutability()) / 2.0);
    util::line_in(buf, inner, row, Line::from(sp(
        format!(" from {} lines; rate {rate:.3}  sd {sd:.3}  mut {:.2}", c.generation, c.genome.mutability()),
        theme::dim_text(),
    )));
    row += 2;

    row = genome_derived(buf, inner, row, sim, c);

    genome_forecast(buf, inner, row, sim, c, &mean)
}

/// The derived-trait table (sense, speed, food need, maturity, ...).
fn genome_derived(buf: &mut Buffer, inner: Rect, mut row: u16, sim: &crate::sim::Sim, c: &Creature) -> u16 {
    panel::section_in(buf, inner, row, "Derived");
    row += 1;
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
    ];
    for (k, v) in derived {
        util::line_in(buf, inner, row, Line::from(vec![
            sp(format!(" {k:<18}"), theme::dim_text()),
            sp(v, theme::text()),
        ]));
        row += 1;
    }
    row += 1;
    row
}

/// The offspring-trait forecast against an average mate.
fn genome_forecast(buf: &mut Buffer, inner: Rect, mut row: u16, sim: &crate::sim::Sim, c: &Creature, mean: &Genome) -> u16 {
    // Offspring forecast (C4 FR10): with an average mate, each trait is drawn
    // from either parent and mutates with the pair's effective sd (the
    // parents' mean Mutability scales `mutation_strength`).
    panel::section_in(buf, inner, row, "Offspring forecast (with an average mate)");
    row += 1;
    let (_, sd) = sim.params.genetics.effective_mutation((c.genome.mutability() + mean.mutability()) / 2.0);
    for t in 0..Genome::LEN {
        if row >= inner.height {
            break;
        }
        let a = c.genome.0[t];
        let b = mean.0[t];
        let centre = (a + b) / 2.0;
        let lo = a.min(b) - sd;
        let hi = a.max(b) + sd;
        let y = inner.y + row;

        buf.set_stringn(inner.x, y, format!(" {:<12}", TRAIT_NAMES[t]), 13, theme::text());
        bars::range(buf, inner.x + 13, y, 22, lo.max(0.0), centre, hi.min(1.0), trait_color(t));
        buf.set_stringn(inner.x + 36, y, format!("{:.2}..{:.2}", lo.max(0.0), hi.min(1.0)), 12, theme::dim_text());
        row += 1;
    }
    row
}
