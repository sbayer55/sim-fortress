//! Sim Fortress: a terminal predator/prey/evolution/resource simulation.
//!
//! The crate is split into a pure, deterministic `sim` core (no terminal code)
//! and a `ui` layer that renders it with `ratatui`. The `fixtures` module keeps
//! the static prototype data alive behind `--prototypes` until C6 removes it.

#![allow(clippy::needless_range_loop, clippy::too_many_arguments, clippy::unnecessary_sort_by)]

pub mod fixtures;
pub mod glyphs;
pub mod prototypes;
pub mod sim;
pub mod theme;
pub mod ui;
pub mod widgets;
