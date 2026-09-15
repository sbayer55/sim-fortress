//! S05: the live charts screen — S05a prey/predator lines, S05b phase plot,
//! S05c stacked species over vegetation (C4 FR9) and S05d infections (C7 FR13).

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::Frame;
use crate::sim::{Sample, Sim};
use crate::ui::app::AppState;
use crate::ui::screens::common::day_stamp;
use crate::ui::screens::{Action, Screen};
use crate::widgets::status;

use time::{time_chart, time_sidebar};
use phase::{phase_chart, phase_sidebar};
use stacked::{stacked_chart, stacked_sidebar};
use infections::{infection_chart, infection_sidebar};
use groups::{group_chart, group_sidebar};

const CHART_W: u16 = 112;

#[derive(Clone, Copy, PartialEq, Eq)]
#[derive(Debug)]
pub enum Variant {
    Time,
    Phase,
    Stacked,
    Infections,
    Groups,
}

#[derive(Debug)]
pub struct Charts {
    variant: Variant,
    zoom: usize,
}

impl Default for Charts {
    fn default() -> Self {
        Self::new()
    }
}

impl Charts {
    pub const fn new() -> Self {
        Self { variant: Variant::Time, zoom: 240 }
    }
}

impl Screen for Charts {
    fn opaque(&self) -> bool {
        true
    }

    fn handle_key(&mut self, key: KeyEvent, _app: &mut AppState) -> Action {
        match key.code {
            KeyCode::Char('g') => {
                self.variant = match self.variant {
                    Variant::Time => Variant::Phase,
                    Variant::Phase => Variant::Stacked,
                    Variant::Stacked => Variant::Infections,
                    Variant::Infections => Variant::Groups,
                    Variant::Groups => Variant::Time,
                };
                Action::None
            }
            KeyCode::Char('1') => {
                self.variant = Variant::Time;
                Action::None
            }
            KeyCode::Char('2') => {
                self.variant = Variant::Phase;
                Action::None
            }
            KeyCode::Char('3') => {
                self.variant = Variant::Stacked;
                Action::None
            }
            KeyCode::Char('4') => {
                self.variant = Variant::Infections;
                Action::None
            }
            KeyCode::Char('5') => {
                self.variant = Variant::Groups;
                Action::None
            }
            KeyCode::Char('+') => {
                self.zoom = match self.zoom {
                    60 => 240,
                    _ => 720,
                };
                Action::None
            }
            KeyCode::Char('-') => {
                self.zoom = match self.zoom {
                    720 => 240,
                    _ => 60,
                };
                Action::None
            }
            KeyCode::Esc => Action::Pop,
            _ => Action::Unhandled,
        }
    }

    fn render(&self, app: &AppState, f: &mut Frame<'_>, area: Rect) {
        let Some(sim) = &app.sim else {
            return;
        };
        let status_row = area.y + area.height - 1;
        let body_h = area.height - 1;
        let chart_w = CHART_W.min(area.width.saturating_sub(20));
        let chart_area = Rect::new(area.x, area.y, chart_w, body_h);
        let side_area = Rect::new(area.x + chart_w, area.y, area.width - chart_w, body_h);
        let window = Window::new(sim, self.zoom);

        match self.variant {
            Variant::Time => {
                time_chart(f, chart_area, sim, &window);
                time_sidebar(f, side_area, sim, &window);
            }
            Variant::Phase => {
                phase_chart(f, chart_area, sim, &window);
                phase_sidebar(f, side_area, sim, &window);
            }
            Variant::Stacked => {
                stacked_chart(f, chart_area, sim, &window);
                stacked_sidebar(f, side_area, sim, &window);
            }
            Variant::Infections => {
                infection_chart(f, chart_area, sim, &window);
                infection_sidebar(f, side_area, sim, &window);
            }
            Variant::Groups => {
                group_chart(f, chart_area, sim);
                group_sidebar(f, side_area, sim);
            }
        }

        let view = match self.variant {
            Variant::Time => "chart 1/5  populations",
            Variant::Phase => "chart 2/5  phase plot",
            Variant::Stacked => "chart 3/5  stacked species",
            Variant::Infections => "chart 4/5  infections",
            Variant::Groups => "chart 5/5  group sizes",
        };
        status::render(f, Rect::new(area.x, status_row, area.width, 1), &[("g", "next chart"), ("1-5", "pick"), ("+/-", "zoom"), ("Esc", "back")], view);
    }
}

/// The visible slice of the daily series.
struct Window<'a> {
    samples: &'a [Sample],
    season_days: u32,
    prey: Vec<f32>,
    pred: Vec<f32>,
    veg: Vec<f32>,
    drought: Vec<bool>,
    /// Days inside an epidemic window (C7): any outbreak flagged `epidemic`
    /// whose `started_day..=ended_day` (or today) covers the sample day.
    epidemic: Vec<bool>,
}

impl<'a> Window<'a> {
    fn new(sim: &'a Sim, zoom: usize) -> Self {
        let all = sim.series.samples();
        let start = all.len().saturating_sub(zoom);
        let samples = &all[start..];
        let today = crate::cast!(sim.time.day_index() => u32);
        let epidemics: Vec<(u32, u32)> = sim.disease.outbreaks.iter().filter(|o| o.epidemic).map(|o| (o.started_day, o.ended_day.unwrap_or(today))).collect();
        Window {
            samples,
            season_days: sim.time.season_days,
            prey: samples.iter().map(|s| crate::cast!((s.population[0] + s.population[1] + s.population[2]) => f32)).collect(),
            pred: samples.iter().map(|s| crate::cast!((s.population[3] + s.population[4] + s.population[5]) => f32)).collect(),
            veg: samples.iter().map(|s| s.veg_mean).collect(),
            drought: samples.iter().map(|s| s.drought_flags.iter().any(|&d| d)).collect(),
            epidemic: samples.iter().map(|s| epidemics.iter().any(|&(a, b)| s.day >= a && s.day <= b)).collect(),
        }
    }

    const fn len(&self) -> usize {
        self.samples.len()
    }

    fn day_label(&self, i: usize) -> String {
        match self.samples.get(i) {
            Some(s) => day_stamp(i64::from(s.day), self.season_days),
            None => String::new(),
        }
    }

    /// Column → sample index range for `cols` columns.
    fn bin(&self, c: usize, cols: usize) -> (usize, usize) {
        let n = self.len();
        let a = (c * n).div_euclid(cols);
        let b = (((c + 1) * n).div_euclid(cols)).max(a + 1).min(n);
        (a, b)
    }

    fn mean_over(&self, v: &[f32], c: usize, cols: usize) -> f32 {
        if self.len() == 0 {
            return 0.0;
        }
        let (a, b) = self.bin(c, cols);
        v[a..b].iter().sum::<f32>() / crate::cast!((b - a) => f32)
    }

    fn drought_in(&self, c: usize, cols: usize) -> bool {
        if self.len() == 0 {
            return false;
        }
        let (a, b) = self.bin(c, cols);
        self.drought[a..b].iter().any(|&d| d)
    }

    fn epidemic_in(&self, c: usize, cols: usize) -> bool {
        if self.len() == 0 {
            return false;
        }
        let (a, b) = self.bin(c, cols);
        self.epidemic[a..b].iter().any(|&d| d)
    }
}

fn round_up(v: f32, step: f32) -> f32 {
    ((v / step).ceil() * step).max(step)
}

fn stats_of(series: &[f32]) -> Option<(f32, usize, f32, usize, f32)> {
    if series.is_empty() {
        return None;
    }
    let (mut min, mut imin, mut max, mut imax) = (f32::MAX, 0, f32::MIN, 0);
    for (i, &v) in series.iter().enumerate() {
        if v < min {
            min = v;
            imin = i;
        }
        if v > max {
            max = v;
            imax = i;
        }
    }
    let mean = series.iter().sum::<f32>() / crate::cast!(series.len() => f32);
    Some((min, imin, max, imax, mean))
}

mod time;
mod phase;
mod stacked;
mod infections;
mod groups;
#[cfg(test)]
mod tests;
