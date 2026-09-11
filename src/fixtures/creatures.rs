//! Individual creatures placed on the world, including three "hero"
//! individuals with hand-written detail for the inspector screens.

use crate::sim::rng::Rng;
use crate::sim::species::{Genome, Kind, SpeciesId};
use crate::sim::world::{Terrain, World};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sex {
    Male,
    Female,
}

#[derive(Clone, Debug)]
pub struct Creature {
    pub id: u32,
    pub name: String,
    pub species: SpeciesId,
    pub x: usize,
    pub y: usize,
    pub adult: bool,
    pub sex: Sex,
    pub alive: bool,
    pub age_days: u32,
    pub max_age_days: u32,
    pub hp: f32,
    pub hunger: f32,
    pub thirst: f32,
    pub energy: f32,
    pub genome: Genome,
    pub generation: u32,
    pub goal: String,
    pub target: Option<(usize, usize)>,
    pub kills: u32,
    pub offspring: u32,
    pub mutations: Vec<String>,
    pub parents: (String, String),
    /// Recent positions, oldest first.
    pub trail: Vec<(usize, usize)>,
    pub cause_of_death: Option<String>,
    pub decay: f32,
}

impl Creature {
    pub fn glyph(&self) -> char {
        let g = self.species.glyph();
        if self.adult { g.to_ascii_uppercase() } else { g }
    }
    pub fn tag(&self) -> String {
        format!("{}#{:03}", self.species.glyph(), self.id)
    }
    pub fn kind(&self) -> Kind {
        self.species.kind()
    }
}

const PREY_NAMES: &[&str] = &[
    "Clover", "Moss", "Fern", "Sorrel", "Rowan", "Willow", "Hazel", "Birch",
    "Tansy", "Yarrow", "Nettle", "Sedge", "Rush", "Burdock", "Mallow", "Vetch", "Cress", "Dill",
];
const PRED_NAMES: &[&str] = &[
    "Greymaw", "Ember", "Sable", "Rook", "Cinder", "Fenrir", "Shade", "Talon", "Brindle",
    "Scorch", "Howl", "Umber", "Flint", "Gloam", "Rime", "Vex", "Snarl", "Dusk", "Kestrel",
];

fn goals_for(kind: Kind, rng: &mut Rng) -> &'static str {
    match kind {
        Kind::Prey => rng.pick(&[
            "grazing", "seeking water", "fleeing", "resting in den", "wandering", "seeking mate", "foraging",
        ]),
        Kind::Predator => rng.pick(&[
            "hunting", "stalking", "resting", "patrolling", "seeking water", "returning to den", "scavenging",
        ]),
    }
}

pub fn generate(world: &World, seed: u64) -> (Vec<Creature>, usize, usize, usize) {
    let (w, h) = (world.width(), world.height());
    let mut rng = Rng::new(seed);
    let mut out = Vec::new();
    let mut next_id: u32 = 1;

    let counts = [
        (SpeciesId::Vole, 44),
        (SpeciesId::Hare, 34),
        (SpeciesId::Deer, 18),
        (SpeciesId::Fox, 11),
        (SpeciesId::Wolf, 8),
        (SpeciesId::Lynx, 5),
    ];

    for (species, n) in counts {
        let base = species.base_genome();
        let mut placed = 0;
        let mut tries = 0;
        while placed < n && tries < 4000 {
            tries += 1;
            let x = rng.below(w);
            let y = rng.below(h);
            let cell = world.cell(x, y);
            if !cell.terrain.walkable() || cell.terrain.is_water() {
                continue;
            }
            // Prey like vegetation, predators like edges of cover.
            let want = match species.kind() {
                Kind::Prey => 0.3 + cell.vegetation * 0.7,
                Kind::Predator => 0.5 + cell.pred_pressure * 0.5,
            };
            if !rng.chance(want) {
                continue;
            }
            let mut g = base;
            for v in g.0.iter_mut() {
                *v = (*v + rng.gauss(0.0, 0.12)).clamp(0.02, 0.98);
            }
            let adult = rng.chance(0.7);
            let kind = species.kind();
            let names = if kind == Kind::Prey { PREY_NAMES } else { PRED_NAMES };
            let max_age = 200 + (g.longevity() * 900.0) as u32;
            out.push(Creature {
                id: next_id,
                name: (*rng.pick(names)).to_string(),
                species,
                x,
                y,
                adult,
                sex: if rng.chance(0.5) { Sex::Male } else { Sex::Female },
                alive: true,
                age_days: if adult { 120 + rng.below(400) as u32 } else { rng.below(90) as u32 },
                max_age_days: max_age,
                hp: (0.4 + rng.f32() * 0.6).min(1.0),
                hunger: rng.f32(),
                thirst: rng.f32(),
                energy: rng.f32(),
                genome: g,
                generation: 10 + rng.below(40) as u32,
                goal: goals_for(kind, &mut rng).to_string(),
                target: None,
                kills: if kind == Kind::Predator { rng.below(30) as u32 } else { 0 },
                offspring: rng.below(12) as u32,
                mutations: Vec::new(),
                parents: (String::new(), String::new()),
                trail: Vec::new(),
                cause_of_death: None,
                decay: 0.0,
            });
            next_id += 1;
            placed += 1;
        }
    }

    // ---- hero prey: Bramble the hare, near the long meadow ------------------
    let (hx, hy) = find_near(world, 78, 20, |t| matches!(t, Terrain::Grass | Terrain::GrassDense));
    let hero_prey_idx = out.len();
    out.push(Creature {
        id: 217,
        name: "Bramble".into(),
        species: SpeciesId::Hare,
        x: hx,
        y: hy,
        adult: true,
        sex: Sex::Female,
        alive: true,
        age_days: 412,
        max_age_days: 640,
        hp: 0.82,
        hunger: 0.38,
        thirst: 0.61,
        energy: 0.47,
        genome: Genome([0.86, 0.22, 0.71, 0.58, 0.08, 0.63, 0.79, 0.41]),
        generation: 47,
        goal: "seeking water".into(),
        target: Some((hx.saturating_sub(9), hy + 5)),
        kills: 0,
        offspring: 9,
        mutations: vec![
            "Speed +0.06 (gen 44)".into(),
            "Camouflage +0.04 (gen 47)".into(),
        ],
        parents: ("Clover h#188".into(), "Rowan h#173".into()),
        trail: trail_from(world, hx, hy, &[(1, 0), (1, 0), (1, -1), (0, -1), (1, 0), (1, 0), (0, 1), (1, 0), (1, 0), (1, 1), (0, 1), (1, 0)]),
        cause_of_death: None,
        decay: 0.0,
    });

    // ---- hero predator: Ashfang the wolf, on Ashen Ridge -------------------
    let (wx, wy) = find_near(world, 68, 15, |t| t.walkable() && !t.is_water());
    let hero_pred_idx = out.len();
    out.push(Creature {
        id: 42,
        name: "Ashfang".into(),
        species: SpeciesId::Wolf,
        x: wx,
        y: wy,
        adult: true,
        sex: Sex::Male,
        alive: true,
        age_days: 1_103,
        max_age_days: 1_460,
        hp: 0.67,
        hunger: 0.74,
        thirst: 0.22,
        energy: 0.55,
        genome: Genome([0.81, 0.74, 0.77, 0.52, 0.91, 0.19, 0.36, 0.66]),
        generation: 23,
        goal: "stalking Bramble h#217".into(),
        target: Some((hx, hy)),
        kills: 61,
        offspring: 14,
        mutations: vec![
            "Aggression +0.09 (gen 19)".into(),
            "Sense +0.05 (gen 21)".into(),
            "Camouflage -0.07 (gen 23)".into(),
        ],
        parents: ("Greymaw w#017".into(), "Sable w#021".into()),
        trail: trail_from(world, wx, wy, &[(1, 0), (0, 1), (1, 0), (1, 0), (0, 1), (1, 0), (1, 1), (1, 0), (0, 1), (1, 0)]),
        cause_of_death: None,
        decay: 0.0,
    });

    // ---- corpse: Thistle the deer, killed in the fenlands ------------------
    let (cx, cy) = find_near(world, 80, 33, |t| t.walkable() && !t.is_water());
    let corpse_idx = out.len();
    out.push(Creature {
        id: 133,
        name: "Thistle".into(),
        species: SpeciesId::Deer,
        x: cx,
        y: cy,
        adult: true,
        sex: Sex::Male,
        alive: false,
        age_days: 866,
        max_age_days: 1_100,
        hp: 0.0,
        hunger: 0.0,
        thirst: 0.0,
        energy: 0.0,
        genome: Genome([0.61, 0.84, 0.49, 0.38, 0.24, 0.31, 0.33, 0.72]),
        generation: 31,
        goal: "—".into(),
        target: None,
        kills: 0,
        offspring: 6,
        mutations: vec!["Size +0.08 (gen 29)".into()],
        parents: ("Fern d#101".into(), "Willow d#097".into()),
        trail: Vec::new(),
        cause_of_death: Some("killed by Ashfang w#042 (Year 12, Day 2)".into()),
        decay: 0.35,
    });

    (out, hero_prey_idx, hero_pred_idx, corpse_idx)
}

fn find_near(world: &World, x: usize, y: usize, ok: impl Fn(Terrain) -> bool) -> (usize, usize) {
    for r in 0..40i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                if world.in_bounds(nx, ny) && ok(world.cell(nx as usize, ny as usize).terrain) {
                    return (nx as usize, ny as usize);
                }
            }
        }
    }
    (x, y)
}

/// Build a trail that ends at (x, y), walking the given steps backwards.
fn trail_from(world: &World, x: usize, y: usize, steps: &[(i32, i32)]) -> Vec<(usize, usize)> {
    let mut pts = Vec::new();
    let (mut cx, mut cy) = (x as i32, y as i32);
    for (dx, dy) in steps {
        cx -= dx;
        cy -= dy;
        if world.in_bounds(cx, cy) {
            pts.push((cx as usize, cy as usize));
        }
    }
    pts.reverse();
    pts
}
