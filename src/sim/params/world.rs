//! World, time, UI, events and stats tunables.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Rainfall {
    Dry,
    Normal,
    Wet,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WorldParams {
    pub width: usize,
    pub height: usize,
    pub water_pct: u8,
    pub forest_pct: u8,
    pub rock_pct: u8,
    pub rainfall: Rainfall,
}

impl Default for WorldParams {
    fn default() -> Self {
        Self {
            width: 150,
            height: 40,
            water_pct: 20,
            forest_pct: 15,
            rock_pct: 5,
            rainfall: Rainfall::Normal,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TimeParams {
    pub season_days: u32,
    pub ticks_per_day: u32,
    pub start_hour: u32,
    pub sunrise_hour: u32,
    pub sunset_hour: u32,
}

impl Default for TimeParams {
    fn default() -> Self {
        Self { season_days: 90, ticks_per_day: 24, start_hour: 6, sunrise_hour: 6, sunset_hour: 20 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ScarcityThresholds {
    pub scarce: f32,
    pub strained: f32,
    pub plenty: f32,
    pub plenty_min_prey: u32,
    pub crowded_prey_per_veg: f32,
}

impl Default for ScarcityThresholds {
    fn default() -> Self {
        Self {
            scarce: 0.365,
            strained: 0.40,
            plenty: 0.45,
            plenty_min_prey: 8,
            crowded_prey_per_veg: 30.0,
        }
    }
}

/// Where the day/night cycle is shown (C6 FR5).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DayNightTint {
    /// No visual day/night cue beyond the sky glyph.
    Off,
    /// Blue-shift the map palette at night.
    Map,
    /// Colour the clock text in the status bar by day and night; the map is untouched.
    #[default]
    StatusText,
}

impl DayNightTint {
    /// The next state in the `[t]` cycle: off → map → status text → off.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Off => Self::Map,
            Self::Map => Self::StatusText,
            Self::StatusText => Self::Off,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Map => "map",
            Self::StatusText => "status text",
        }
    }
}

/// Older `ui.toml` files stored `day_night_tint` as a bool; `true` meant the map
/// tint and `false` meant off. The bool-or-name form is only readable from
/// self-describing formats (TOML); binary saves (postcard) use the plain enum.
impl<'de> Deserialize<'de> for DayNightTint {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(rename_all = "snake_case")]
        enum Named {
            Off,
            Map,
            StatusText,
        }
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Legacy(bool),
            Named(Named),
        }
        let named = if d.is_human_readable() {
            match Raw::deserialize(d)? {
                Raw::Legacy(true) => Named::Map,
                Raw::Legacy(false) => Named::Off,
                Raw::Named(n) => n,
            }
        } else {
            Named::deserialize(d)?
        };
        Ok(match named {
            Named::Off => Self::Off,
            Named::Map => Self::Map,
            Named::StatusText => Self::StatusText,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct UiParams {
    pub speeds: Vec<u32>,
    pub base_ticks_per_second: f32,
    pub auto_pause_on_extinction: bool,
    pub log_births: bool,
    pub pause_on_follow_death: bool,
    /// Autosave every N days (0 = off), C6 FR1/FR5.
    pub autosave_days: u32,
    /// How the day/night cycle is shown: off, map tint, or status-bar clock colour (C6 FR5).
    pub day_night_tint: DayNightTint,
    /// Pause when a pathogen becomes epidemic (C7 FR9).
    pub auto_pause_on_epidemic: bool,
    pub scarcity_thresholds: ScarcityThresholds,
}

impl Default for UiParams {
    fn default() -> Self {
        Self {
            speeds: vec![1, 2, 5, 10, 25],
            base_ticks_per_second: 2.0,
            auto_pause_on_extinction: true,
            log_births: false,
            pause_on_follow_death: true,
            autosave_days: 0,
            day_night_tint: DayNightTint::StatusText,
            auto_pause_on_epidemic: true,
            scarcity_thresholds: ScarcityThresholds::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EventsParams {
    pub capacity: usize,
}

impl Default for EventsParams {
    fn default() -> Self {
        Self { capacity: 5000 }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct StatsParams {
    pub series_days: usize,
}

impl Default for StatsParams {
    fn default() -> Self {
        Self { series_days: 720 }
    }
}
