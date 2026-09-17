//! S09 render checks for the summary and chronicle block.

use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

use super::WorldGen;
use crate::sim::Params;
use crate::ui::app::AppState;
use crate::ui::screens::Screen;

#[test]
fn s09_preview_shows_the_summary_and_chronicle() {
    let app = AppState::new(Params::default());
    let s = WorldGen::new();
    let backend = TestBackend::new(155, 45);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| s.render(&app, f, Rect::new(0, 0, 155, 45))).unwrap();
    let preview: String = (0..45).map(|y| (66..155).map(|x| terminal.backend().buffer()[(x, y)].symbol().to_string()).collect::<String>() + "\n").collect();
    assert!(preview.contains("─ World ─"), "summary divider:\n{preview}");
    assert!(preview.contains(" coast ") && preview.contains("longest river"), "summary line:\n{preview}");
    assert!(preview.contains("─ Chronicle ─"), "chronicle divider:\n{preview}");
    assert!(preview.contains(" winds "), "the first chronicle line names the wind:\n{preview}");
    assert!(preview.contains(" runs from ") || preview.contains(" winds through "), "a river line:\n{preview}");
}
