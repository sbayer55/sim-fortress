#![allow(clippy::needless_range_loop, clippy::too_many_arguments, clippy::unnecessary_sort_by)]

mod app;
mod fixtures;
mod glyphs;
mod prototypes;
mod theme;
mod widgets;

fn main() -> std::io::Result<()> {
    let mut terminal = ratatui::init();
    let result = app::run(&mut terminal);
    ratatui::restore();
    result
}
