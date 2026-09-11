//! Daily time series: a ring buffer of per-day snapshots.

use serde::{Deserialize, Serialize};

/// One day's snapshot of the world's ecological state.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sample {
    /// Absolute 0-based day index.
    pub day: u32,
    pub biomass_total: f32,
    /// Mean vegetation over land cells (water excluded, rock included).
    pub veg_mean: f32,
    pub water_cells: usize,
    pub water_level: f32,
    /// Mean moisture over all cells, water treated as 1.0.
    pub moisture_mean: f32,
    pub seeds: usize,
    pub dens: usize,
    pub carcasses: usize,
    pub drought_regions: usize,
    pub drought_flags: [bool; 8],
    /// Mean vegetation over land cells, per region.
    pub region_veg: [f32; 8],
    /// Mean moisture over all cells (water = 1.0), per region.
    pub region_moist: [f32; 8],
    /// Per-species population (zero until C3).
    pub population: [u32; 6],
}

/// Daily ring buffer; oldest samples are evicted once `cap` is exceeded.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Series {
    cap: usize,
    buf: Vec<Sample>,
    /// Absolute day index of the first (oldest) sample.
    day0: u32,
}

impl Series {
    pub fn new(cap: usize) -> Self {
        Series { cap, buf: Vec::new(), day0: 0 }
    }

    pub fn push(&mut self, s: Sample) {
        if self.cap == 0 {
            return;
        }
        if self.buf.is_empty() {
            self.day0 = s.day;
        }
        self.buf.push(s);
        let excess = self.buf.len().saturating_sub(self.cap);
        if excess > 0 {
            self.buf.drain(0..excess);
            self.day0 = self.buf.first().map(|s| s.day).unwrap_or(self.day0);
        }
    }

    pub fn len(&self) -> usize {
        self.buf.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// All samples, oldest first.
    pub fn samples(&self) -> &[Sample] {
        &self.buf
    }

    pub fn last(&self) -> Option<&Sample> {
        self.buf.last()
    }

    /// Absolute day index of the first (oldest) sample.
    pub fn day0(&self) -> u32 {
        self.day0
    }

    /// Serialize the series as CSV: a header row plus one row per sample.
    pub fn to_csv(&self, region_names: &[String]) -> String {
        let mut out = String::from("day,biomass_total,veg_mean,water_cells,water_level,moisture_mean,seeds,drought_regions");
        for name in region_names.iter().take(8) {
            out.push_str(&format!(",veg_{name}"));
        }
        for name in region_names.iter().take(8) {
            out.push_str(&format!(",moist_{name}"));
        }
        out.push('\n');
        for s in &self.buf {
            out.push_str(&format!(
                "{},{},{},{},{},{},{},{}",
                s.day, s.biomass_total, s.veg_mean, s.water_cells, s.water_level, s.moisture_mean, s.seeds, s.drought_regions
            ));
            for i in 0..8 {
                out.push_str(&format!(",{}", s.region_veg[i]));
            }
            for i in 0..8 {
                out.push_str(&format!(",{}", s.region_moist[i]));
            }
            out.push('\n');
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(day: u32) -> Sample {
        Sample {
            day,
            biomass_total: 0.0,
            veg_mean: 0.0,
            water_cells: 0,
            water_level: 0.0,
            moisture_mean: 0.0,
            seeds: 0,
            dens: 0,
            carcasses: 0,
            drought_regions: 0,
            drought_flags: [false; 8],
            region_veg: [0.0; 8],
            region_moist: [0.0; 8],
            population: [0; 6],
        }
    }

    #[test]
    fn series_ring_buffer() {
        let mut s = Series::new(3);
        assert!(s.is_empty());
        for day in 0..5 {
            s.push(sample(day));
        }
        // Only the last 3 samples remain, oldest first.
        assert_eq!(s.len(), 3);
        assert_eq!(s.day0(), 2);
        let days: Vec<u32> = s.samples().iter().map(|s| s.day).collect();
        assert_eq!(days, vec![2, 3, 4]);
    }

    #[test]
    fn daily_sampling_fields() {
        let mut s = Series::new(720);
        let mut x = sample(1);
        x.biomass_total = 12.5;
        x.veg_mean = 0.4;
        x.water_cells = 300;
        x.water_level = 0.5;
        x.drought_regions = 2;
        x.drought_flags[0] = true;
        s.push(x);
        let last = s.last().unwrap();
        assert_eq!(last.day, 1);
        assert_eq!(last.biomass_total, 12.5);
        assert_eq!(last.veg_mean, 0.4);
        assert_eq!(last.water_cells, 300);
        assert_eq!(last.water_level, 0.5);
        assert_eq!(last.drought_regions, 2);
        assert!(last.drought_flags[0]);
        assert!(!last.drought_flags[1]);
    }
}
