//! Built-in parameter presets.


/// A named parameter preset (C6 FR7). `overlay` is a partial TOML table merged
/// over the current parameters; empty for Balanced (the defaults).
#[derive(Clone, Copy, Debug)]
pub struct Preset {
    pub name: &'static str,
    pub description: &'static str,
    pub overlay: &'static str,
}

pub const PRESETS: [Preset; 6] = [
    Preset { name: "Balanced", description: "default values, gentle seasons", overlay: "" },
    Preset {
        name: "Harsh winter",
        description: "180-day seasons, regrowth 0.6",
        overlay: "time.season_days = 180\necology.regrowth_rate = 0.6\n",
    },
    Preset {
        name: "Lush",
        description: "forest 30%, regrowth 1.4, predation hard",
        overlay: "world.forest_pct = 30\necology.regrowth_rate = 1.4\npredation.difficulty = \"hard\"\n",
    },
    Preset { name: "Archipelago", description: "water 55%, islands isolate lineages", overlay: "world.water_pct = 55\n" },
    Preset {
        name: "Fast evolution",
        description: "mutation rate 0.10, strength 0.12",
        overlay: "genetics.mutation_rate = 0.10\ngenetics.mutation_strength = 0.12\n",
    },
    Preset {
        name: "Plague years",
        description: "outbreaks 3x as often, short reservoir, mutation 0.06",
        overlay: "disease.emergence_per_day = 0.012\ndisease.reservoir_days = 45\ngenetics.mutation_rate = 0.06\n",
    },
];
