//! The ratatui application shell: screen stack, key routing, viewport and the
//! live screens. Built on top of `sim` and the shared `widgets`.

pub mod ai_bridge;
pub mod app;
pub mod config;
pub mod screens;
pub mod style;
pub mod viewport;

#[cfg(test)]
mod tests;
