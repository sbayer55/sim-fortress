//! Simulation clock: ticks → hours, days, seasons, years.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Season {
    Spring,
    Summer,
    Autumn,
    Winter,
}

impl Season {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Spring => "Spring",
            Self::Summer => "Summer",
            Self::Autumn => "Autumn",
            Self::Winter => "Winter",
        }
    }

    /// The ticker/event text announced when this season begins.
    pub const fn event_text(self) -> &'static str {
        match self {
            Self::Spring => "Spring returns to the valley; regrowth quickens",
            Self::Summer => "Summer settles over the valley; evaporation peaks",
            Self::Autumn => "Autumn colours the valley; regrowth slows",
            Self::Winter => "Winter settles over the valley; vegetation regrowth halves",
        }
    }

    pub const ALL: [Self; 4] = [Self::Spring, Self::Summer, Self::Autumn, Self::Winter];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Time {
    pub tick: u64,
    pub start_hour: u32,
    pub season_days: u32,
    pub ticks_per_day: u32,
    pub sunrise_hour: u32,
    pub sunset_hour: u32,
}

impl Time {
    pub const fn new(start_hour: u32, season_days: u32, ticks_per_day: u32, sunrise_hour: u32, sunset_hour: u32) -> Self {
        Self { tick: 0, start_hour, season_days, ticks_per_day, sunrise_hour, sunset_hour }
    }

    pub fn hour(&self) -> u32 {
        crate::cast!(((self.tick + u64::from(self.start_hour)) % u64::from(self.ticks_per_day)) => u32)
    }

    /// 0-based absolute day index (tick + `start_hour`) / `ticks_per_day`.
    pub fn day_index(&self) -> u64 {
        (self.tick + u64::from(self.start_hour)).div_euclid(u64::from(self.ticks_per_day))
    }

    /// 1-based day within the current season.
    pub fn day_of_season(&self) -> u32 {
        crate::cast!((self.day_index() % u64::from(self.season_days)) => u32) + 1
    }

    pub fn season(&self) -> Season {
        match (self.day_index().div_euclid(u64::from(self.season_days))) % 4 {
            0 => Season::Spring,
            1 => Season::Summer,
            2 => Season::Autumn,
            _ => Season::Winter,
        }
    }

    /// 1-based day within the year (`1..=4×season_days`).
    pub fn day_of_year(&self) -> u32 {
        crate::cast!((self.day_index() % (4 * u64::from(self.season_days))) => u32) + 1
    }

    /// 1-based year.
    pub fn year(&self) -> u32 {
        crate::cast!(self.day_index().div_euclid(4 * u64::from(self.season_days)) => u32) + 1
    }

    pub fn is_night(&self) -> bool {
        self.hour() < self.sunrise_hour || self.hour() >= self.sunset_hour
    }

    /// Advance one tick. Returns the newly-entered season when this tick crosses
    /// a season boundary (the initial Spring at tick 0 is handled by `Sim`).
    pub fn advance(&mut self) -> Option<Season> {
        let was = self.season();
        self.tick += 1;
        let now = self.season();
        (now != was).then_some(now)
    }

    /// `Year 1, Day 1 of Spring  06:00` — the live clock label.
    pub fn clock_label(&self) -> String {
        format!(
            "Year {}, Day {} of {}  {:02}:00",
            self.year(),
            self.day_of_season(),
            self.season().name(),
            self.hour()
        )
    }

    /// `Y1 D001` — the compact day-of-year label (day zero-padded to 3).
    pub fn day_of_year_label(&self) -> String {
        format!("Y{} D{:03}", self.year(), self.day_of_year())
    }

    /// `06:00` — hour label.
    pub fn hour_label(&self) -> String {
        format!("{:02}:00", self.hour())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn time() -> Time {
        Time::new(6, 90, 24, 6, 20)
    }

    #[test]
    fn tick0_is_day1_spring_0600() {
        let t = time();
        assert_eq!(t.tick, 0);
        assert_eq!(t.hour(), 6);
        assert_eq!(t.day_index(), 0);
        assert_eq!(t.day_of_season(), 1);
        assert_eq!(t.season(), Season::Spring);
        assert_eq!(t.day_of_year(), 1);
        assert_eq!(t.year(), 1);
        assert!(!t.is_night());
    }

    #[test]
    fn season_rollover() {
        let mut t = time();
        // tick 2153 is the last tick of Spring (day_index 89); advancing crosses into Summer.
        t.tick = 2153;
        assert_eq!(t.season(), Season::Spring);
        assert_eq!(t.day_of_season(), 90);
        assert_eq!(t.advance(), Some(Season::Summer));
        assert_eq!(t.season(), Season::Summer);
        assert_eq!(t.day_of_season(), 1);
        assert_eq!(t.day_of_year(), 91);
    }

    #[test]
    fn night_hours() {
        let mut t = time();
        // hour 19 → day; hour 20 → night (>= sunset); hour 5 → night (< sunrise); hour 6 → day.
        t.tick = 13; // hour 19
        assert!(!t.is_night());
        t.tick = 14; // hour 20
        assert!(t.is_night());
        t.tick = 24 + 5 - 6; // hour 5
        assert!(t.is_night());
        t.tick = 24 + 6 - 6; // hour 6
        assert!(!t.is_night());
    }

    #[test]
    fn day_of_year_padding() {
        // Day 91 of year 1 → "Y1 D091".
        let mut t = time();
        t.tick = 90 * 24;
        assert_eq!(t.day_of_year(), 91);
        assert_eq!(t.day_of_year_label(), "Y1 D091");
        assert_eq!(t.hour_label(), "06:00");
    }
}
