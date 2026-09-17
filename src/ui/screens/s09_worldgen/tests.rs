//! S09 tests: the designer overlay hand-back (C9) and the summary and
//! chronicle block on the preview.

use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

use super::*;
use crate::ui::screens::Screen;

#[test]
fn apply_species_overlay_appends_by_name_and_edits_in_place() {
    let mut form = WorldGenForm::new(&Params::default());
    let n = form.counts.len();
    form.focus = form.field_count() - 1;
    form.apply_species_overlay("[[species]]\nname = \"boar\"\nplural = \"Boars\"\nglyph = \"b\"\ninitial_count = 30\n").unwrap();
    assert_eq!(form.counts.len(), n + 1);
    assert_eq!(form.species.0.last().map(|s| s.name.as_str()), Some("boar"));
    assert_eq!(form.counts.last(), Some(&30));
    assert!(!form.dirty, "the preview does not depend on the roster");
    form.apply_species_overlay("[[species]]\nname = \"boar\"\ninitial_count = 12\n").unwrap();
    assert_eq!(form.counts.len(), n + 1, "a known name edits in place");
    assert_eq!(form.counts.last(), Some(&12));
    assert!(form.apply_species_overlay("[[species]]\nname = \"BAD\"\n").is_err());
    assert_eq!(form.counts.len(), n + 1, "a rejected overlay changes nothing");
    let p = form.build_params();
    assert_eq!(p.species.0.last().map(|s| s.initial_count), Some(12));
}

#[test]
fn designer_button_exists_only_while_the_feature_is_on() {
    let mut form = WorldGenForm::new(&Params::default());
    let app = AppState::new(Params::default());
    form.sync_designer(&app);
    assert!(!form.designer);
    assert_eq!(form.field_count(), form.tail() + T_COUNT - 1);
    form.designer = true;
    assert_eq!(form.field_count(), form.tail() + T_COUNT);
}

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
