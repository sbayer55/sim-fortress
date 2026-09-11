//! Synthetic time series: population per species, vegetation, water.

use crate::sim::species::SpeciesId;

pub const LEN: usize = 240;

pub struct Series {
    /// One point per day for the last `LEN` days, per species (same order as `SpeciesId::ALL`).
    pub population: Vec<Vec<f32>>,
    pub vegetation: Vec<f32>,
    pub water: Vec<f32>,
    pub carcasses: Vec<f32>,
    /// Day index of each point (absolute, for axis labels).
    pub day0: u32,
}

impl Series {
    pub fn pop(&self, id: SpeciesId) -> &[f32] {
        let i = SpeciesId::ALL.iter().position(|&s| s == id).unwrap();
        &self.population[i]
    }
    pub fn prey_total(&self) -> Vec<f32> {
        (0..LEN).map(|i| self.population[0][i] + self.population[1][i] + self.population[2][i]).collect()
    }
    pub fn pred_total(&self) -> Vec<f32> {
        (0..LEN).map(|i| self.population[3][i] + self.population[4][i] + self.population[5][i]).collect()
    }
}

pub fn generate() -> Series {
    // Lotka-Volterra style oscillation with noise and a drought dip.
    let mut prey = 340.0f32;
    let mut pred = 48.0f32;
    let mut prey_s = Vec::with_capacity(LEN);
    let mut pred_s = Vec::with_capacity(LEN);
    let mut veg = Vec::with_capacity(LEN);
    let mut water = Vec::with_capacity(LEN);
    let mut carc = Vec::with_capacity(LEN);
    let mut rng = crate::sim::rng::Rng::new(0x7E57);
    for i in 0..LEN {
        let season = ((i as f32 / 90.0) * std::f32::consts::TAU).sin();
        let drought = if (150..190).contains(&i) { 0.55 } else { 1.0 };
        let v = (0.62 + 0.22 * season) * drought + rng.gauss(0.0, 0.02);
        veg.push(v.clamp(0.05, 1.0));
        water.push((0.7 + 0.15 * season) * (if drought < 1.0 { 0.6 } else { 1.0 }) + rng.gauss(0.0, 0.015));
        let dp = 0.030 * prey * v * drought - 0.00045 * prey * pred;
        let dq = 0.00016 * prey * pred - 0.028 * pred;
        prey = (prey + dp + rng.gauss(0.0, 3.0)).max(20.0);
        pred = (pred + dq + rng.gauss(0.0, 0.8)).max(4.0);
        prey_s.push(prey);
        pred_s.push(pred);
        carc.push((pred * 0.4 + rng.gauss(0.0, 2.0)).max(0.0));
    }
    let split = |total: &[f32], w: [f32; 3], wob: f32| -> Vec<Vec<f32>> {
        (0..3)
            .map(|k| {
                total
                    .iter()
                    .enumerate()
                    .map(|(i, t)| t * w[k] * (1.0 + wob * ((i as f32 * 0.07) + k as f32).sin()))
                    .collect()
            })
            .collect()
    };
    let mut population = split(&prey_s, [0.48, 0.34, 0.18], 0.12);
    population.extend(split(&pred_s, [0.42, 0.36, 0.22], 0.18));
    Series { population, vegetation: veg, water, carcasses: carc, day0: 12 * 360 + 4 - LEN as u32 }
}
