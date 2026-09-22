//! S14: the overlay switcher, a modal over the map that composes the stack.
//!
//! The stack lives on `AppState`: one base heatmap, any number of marks, and
//! the species, pathogen or predator picked inside the dialog. The map
//! beneath is the live preview, so the backdrop is not dimmed.

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::Frame;

use crate::ui::app::AppState;
use crate::ui::screens::common::{sp, Rule};
use crate::ui::screens::s01_map::{layer_legend, live_stack, subject_alive, WorldMap};
use crate::ui::screens::{Action, Screen};
use crate::widgets::map::{Base, Layer, OverlayStack};
use crate::widgets::Constraint::Fixed;
use crate::widgets::{Checkbox, Column, Component, Divider, Modal, Rows, Spacer, StatusBar, Table, TableCell, TableRow, Text, VStack};
use crate::{glyphs, theme};

use rows::{chosen, rows, sub_pick, Pick, RowSpec, SubPick, Tab};

/// The modal box (S14 Layout).
const MODAL_W: u16 = 84;
const MODAL_H: u16 = 21;
/// The left column and the rows the two columns share.
const LEFT_W: u16 = 34;
const COLUMN_ROWS: u16 = 15;
/// The right column starts one blank after the divider.
const RIGHT_X: u16 = LEFT_W + 2;

const STATUS_KEYS: [(&str, &str); 8] = [("Tab", "base / marks"), ("↑↓", "move"), ("Space", "toggle"), ("→", "sub-pick"), ("←", "back"), ("Enter", "done"), ("Bksp", "clear all"), ("o Esc", "close")];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Focus {
    Rows,
    List,
}

#[derive(Debug)]
pub struct OverlaySwitcher {
    pub tab: Tab,
    /// The cursor row of the active tab.
    pub cur: usize,
    pub focus: Focus,
    /// The cursor in the sub-pick list while it has focus.
    pub list_cur: usize,
}

impl Default for OverlaySwitcher {
    fn default() -> Self {
        Self::new()
    }
}

impl OverlaySwitcher {
    pub const fn new() -> Self {
        Self { tab: Tab::Base, cur: 0, focus: Focus::Rows, list_cur: 0 }
    }

    /// Open from the map: a sub-pick whose layer is off is refreshed by the
    /// S02f / S02d default rules so every row says what would come on.
    pub fn open(app: &mut AppState) -> Self {
        if !matches!(app.overlay.base, Base::Species | Base::Scent) {
            app.overlay.species = WorldMap::default_species(app);
        }
        if !app.overlay.sense && !subject_alive(app, app.overlay.sense_subject) {
            app.overlay.sense_subject = WorldMap::default_sense(app);
        }
        Self::new()
    }

    /// `Space` on a row: a base row becomes the base, a mark toggles.
    fn toggle(row: &RowSpec, app: &mut AppState) {
        if row.disabled {
            return;
        }
        let stack = &mut app.overlay;
        match row.layer {
            Layer::Base(b) => stack.base = b,
            Layer::Sense => {
                if stack.sense {
                    stack.sense = false;
                } else {
                    WorldMap::turn_sense_on(app);
                }
            }
            Layer::Regions => stack.regions = !stack.regions,
            Layer::Health => stack.health = !stack.health,
            Layer::Disease => {
                if stack.disease.is_on() {
                    stack.disease = crate::widgets::map::Disease::Off;
                } else {
                    stack.show_disease(stack.pathogen);
                }
            }
        }
    }

    /// `Enter` in the list: pick the entry and turn its layer on. A species
    /// picked from the Scent row turns Scent on, not Species (both share it).
    const fn pick(pick: Pick, layer: Layer, app: &mut AppState) {
        let stack = &mut app.overlay;
        match pick {
            Pick::Species(sp) => {
                stack.species = sp;
                stack.base = if matches!(layer, Layer::Base(Base::Scent)) { Base::Scent } else { Base::Species };
            }
            Pick::Predator(id) => {
                stack.sense_subject = Some(id);
                stack.sense = true;
            }
            Pick::Pathogen(slot) => stack.show_disease(slot),
        }
    }

    fn rows_key(&mut self, code: KeyCode, app: &mut AppState, rows: &[RowSpec]) -> Action {
        let n = rows.len().max(1);
        match code {
            KeyCode::Up => self.cur = (self.cur + n - 1) % n,
            KeyCode::Down => self.cur = (self.cur + 1) % n,
            KeyCode::Char(' ') => {
                if let Some(row) = rows.get(self.cur) {
                    Self::toggle(row, app);
                }
            }
            KeyCode::Right => {
                let Some(row) = rows.get(self.cur) else { return Action::None };
                if row.disabled || !row.layer.has_sub_pick() {
                    return Action::None;
                }
                let Some(list) = sub_pick(row.layer, app) else { return Action::None };
                self.list_cur = list.position(chosen(row.layer, &app.overlay)).unwrap_or(0);
                self.focus = Focus::List;
            }
            KeyCode::Enter => return Action::Pop,
            _ => return Action::Unhandled,
        }
        Action::None
    }

    fn list_key(&mut self, code: KeyCode, app: &mut AppState, rows: &[RowSpec]) -> Action {
        let layer = rows.get(self.cur).map(|row| row.layer);
        let list = layer.and_then(|l| sub_pick(l, app));
        let n = list.as_ref().map_or(0, |l| l.entries.len()).max(1);
        match code {
            KeyCode::Up => self.list_cur = (self.list_cur + n - 1) % n,
            KeyCode::Down => self.list_cur = (self.list_cur + 1) % n,
            KeyCode::Left => self.focus = Focus::Rows,
            KeyCode::Enter | KeyCode::Char(' ') => {
                if let (Some(entry), Some(layer)) = (list.as_ref().and_then(|l| l.entries.get(self.list_cur)), layer) {
                    Self::pick(entry.pick, layer, app);
                }
                self.focus = Focus::Rows;
            }
            _ => return Action::Unhandled,
        }
        Action::None
    }
}

impl Screen for OverlaySwitcher {
    fn opaque(&self) -> bool {
        false
    }

    fn dims_backdrop(&self) -> bool {
        false
    }

    fn handle_key(&mut self, key: KeyEvent, app: &mut AppState) -> Action {
        if let Some(sim) = app.sim.as_ref() {
            app.overlay = live_stack(sim, app.overlay);
        }
        match key.code {
            KeyCode::Esc | KeyCode::Char('o') => return Action::Pop,
            KeyCode::Backspace => {
                app.overlay.clear();
                return Action::None;
            }
            KeyCode::Tab | KeyCode::BackTab => {
                self.tab = self.tab.other();
                self.cur = 0;
                self.focus = Focus::Rows;
                return Action::None;
            }
            _ => {}
        }
        let rows = rows(self.tab, app);
        self.cur = self.cur.min(rows.len().saturating_sub(1));
        match self.focus {
            Focus::Rows => self.rows_key(key.code, app, &rows),
            Focus::List => self.list_key(key.code, app, &rows),
        }
    }

    fn render(&self, app: &AppState, f: &mut Frame<'_>, area: Rect) {
        let stack = app.sim.as_ref().map_or(app.overlay, |sim| live_stack(sim, app.overlay));
        let rows = rows(self.tab, app);
        let cur = self.cur.min(rows.len().saturating_sub(1));

        // Centred on the map panel (D6), whose inner size the map reported.
        let (iw, ih) = app.viewport_size.get();
        let panel = if iw == 0 { area } else { Rect::new(area.x, area.y, crate::cast!(iw => u16) + 2, crate::cast!(ih => u16) + 2).intersection(area) };
        let modal = Modal::new(MODAL_W.min(panel.width.saturating_sub(2)), MODAL_H.min(panel.height.saturating_sub(2))).title("Overlay").info("o or Esc closes");
        let buf = f.buffer_mut();
        let frame = modal.frame(panel);
        let body = modal.render(buf, panel);
        let columns = Rect { height: COLUMN_ROWS.min(body.height), ..body };
        let left = Rect { width: LEFT_W.min(columns.width), ..columns };

        // Left column: the tab strip, the tab rule, the layer rows.
        tab_strip(self.tab).render(buf, Rect { height: 1, ..left });
        tab_rule(self.tab).render(buf, Rect { y: left.y + 1, height: 1, ..left });
        let boxes: Rows<'_> = rows.iter().enumerate().map(|(i, row)| layer_row(row, i == cur && self.focus == Focus::Rows, self.tab, &stack)).collect();
        VStack::from_boxes(&boxes).render(buf, Rect { y: left.y + 2, height: left.height.saturating_sub(2), ..left });

        // The divider and its joins.
        if body.width > LEFT_W {
            let rx = body.x + LEFT_W;
            Rule.render(buf, Rect { x: rx, width: 1, ..columns });
            buf.set_stringn(rx, frame.y, glyphs::T_DOWN.to_string(), 1, theme::border_focus());
            if columns.bottom() < body.bottom() {
                buf.set_stringn(rx, columns.bottom(), glyphs::T_UP.to_string(), 1, theme::border());
            }
        }

        // Right column: the sub-pick list, or the description and legend.
        if body.width > RIGHT_X + 20 {
            let right = Rect { x: body.x + RIGHT_X, width: body.width - RIGHT_X, ..columns };
            if let Some(row) = rows.get(cur) {
                match sub_pick(row.layer, app) {
                    Some(list) => self.render_list(buf, right, row, &list, &stack),
                    None => render_description(buf, right, row.layer, &stack, app),
                }
            }
        }

        // Showing.
        if body.height > COLUMN_ROWS + 1 {
            let showing = Rect { y: body.y + COLUMN_ROWS + 1, height: body.height - COLUMN_ROWS - 1, ..body };
            let title = app.sim.as_ref().map(|sim| stack.title(sim.roster(), &sim.disease)).filter(|t| !t.is_empty());
            let title_row = title.map_or_else(|| Text::new("  nothing, plain terrain").style(theme::dim_text()), |t| Text::new(format!("  {t}")).fg(theme::TEXT_BRIGHT).bold());
            let (d, h) = (Divider::new("Showing"), Text::new("  Enter closes and keeps the stack   Backspace clears everything").style(theme::dim_text()));
            VStack::new().child(&d).child(&title_row).child(&h).render(buf, showing);
        }

        let status_row = area.y + area.height - 1;
        StatusBar::new(&STATUS_KEYS).render(buf, Rect::new(area.x, status_row, area.width, 1));
    }
}

impl OverlaySwitcher {
    /// The right column while the cursor row has a sub-pick (S14 items 6–11).
    fn render_list(&self, buf: &mut ratatui::buffer::Buffer, right: Rect, row: &RowSpec, list: &SubPick, stack: &OverlayStack) {
        let on = row.is_on(stack);
        let chosen = chosen(row.layer, stack);
        let focused = self.focus == Focus::List;
        let table_rows: Vec<TableRow<'_>> = list
            .entries
            .iter()
            .map(|e| {
                let picked = on && Some(e.pick) == chosen;
                let label = if picked {
                    TableCell::widget(Text::spans(vec![Span::styled(e.label.clone(), Style::default().fg(theme::TEXT_BRIGHT).add_modifier(Modifier::BOLD))]))
                } else {
                    TableCell::text(e.label.clone())
                };
                let glyph = e.glyph.map_or(TableCell::Blank, |(g, c)| TableCell::Glyph(g, c));
                TableRow::new([TableCell::Blank, glyph, label, TableCell::text(e.right.clone())]).absent(e.absent)
            })
            .collect();
        let columns = [Column::new(Fixed(1)), Column::new(Fixed(2)), Column::new(Fixed(22)), Column::new(Fixed(11)).right()];
        Table::new(&columns, &table_rows).selected(focused.then_some(self.list_cur)).render(buf, right);
        // The header row (the Table's own is blank) and the chosen mark.
        Text::new(list.header).style(theme::label()).render(buf, Rect { height: 1, ..right });
        if let Some(i) = chosen.filter(|_| on).and_then(|c| list.position(Some(c))) {
            let y = right.y + 1 + crate::cast!(i => u16);
            if y < right.bottom() && !(focused && i == self.list_cur) {
                buf.set_stringn(right.x, y, glyphs::BULLET.to_string(), 1, theme::label());
            }
        }
        let mut y = right.y + 1 + crate::cast!(list.entries.len() => u16);
        if list.more > 0 && y < right.bottom() {
            Text::new(format!("  … and {} more", list.more)).style(theme::dim_text()).render(buf, Rect { y, height: 1, ..right });
            y += 1;
        }
        let footer = [" → steps into this list, ← back", " Enter picks it and switches the layer on", " Space on the left toggles without picking"];
        for (i, line) in footer.iter().enumerate() {
            let fy = y + 1 + crate::cast!(i => u16);
            if fy < right.bottom() {
                Text::new(*line).style(theme::dim_text()).render(buf, Rect { y: fy, height: 1, ..right });
            }
        }
    }
}

/// The right column for a row without a sub-pick (S14 items 12–13).
fn render_description(buf: &mut ratatui::buffer::Buffer, right: Rect, layer: Layer, stack: &OverlayStack, app: &AppState) {
    let kind = if matches!(layer, Layer::Base(_)) { "base heatmap" } else { "mark" };
    let mut rows: Rows<'_> = vec![
        Box::new(Text::new(format!(" {}  · {kind}", layer.name().to_lowercase())).style(theme::label())),
        Box::new(Text::new(format!(" {}", layer.description()))),
        Box::new(Spacer::rows(1)),
    ];
    if let Some(sim) = app.sim.as_ref() {
        rows.extend(layer_legend(layer, stack, sim));
    }
    VStack::from_boxes(&rows).render(buf, Rect { height: right.height.saturating_sub(3), ..right });
    let note = if matches!(layer, Layer::Base(_)) { " Space makes it the base" } else { " Space toggles it" };
    for (i, line) in [" no sub-pick for this layer", note].iter().enumerate() {
        let y = right.y + right.height.saturating_sub(3) + crate::cast!(i => u16);
        if y < right.bottom() {
            Text::new(*line).style(theme::dim_text()).render(buf, Rect { y, height: 1, ..right });
        }
    }
}

/// One layer row as a Checkbox (S14 item 5).
fn layer_row<'a>(row: &RowSpec, focused: bool, tab: Tab, stack: &OverlayStack) -> Box<dyn Component + 'a> {
    let mut cb = Checkbox::new(row.layer.name(), row.is_on(stack)).radio(tab == Tab::Base).label_w(11).indent(1).focused(focused).disabled(row.disabled);
    if let Some(value) = &row.value {
        cb = cb.value(value.clone()).cue();
    }
    Box::new(cb)
}

/// ` Base heatmap   Marks        Tab`, the active tab in the selection style.
fn tab_strip(tab: Tab) -> Text<'static> {
    let style = |t: Tab| if t == tab { theme::selected() } else { theme::text() };
    Text::spans(vec![
        sp(" ", theme::text()),
        sp(format!(" {} ", Tab::Base.title()), style(Tab::Base)),
        sp(" ", theme::text()),
        sp(format!(" {} ", Tab::Marks.title()), style(Tab::Marks)),
        sp("       ", theme::text()),
        sp("Tab", theme::key()),
    ])
}

/// The `─` rule under the tabs: blank in the selection background under the
/// active tab, the tab's word right-aligned before three rule cells.
fn tab_rule(tab: Tab) -> Text<'static> {
    let w = usize::from(LEFT_W);
    let active = match tab {
        Tab::Base => 1..1 + Tab::Base.title().len() + 2,
        Tab::Marks => 16..16 + Tab::Marks.title().len() + 2,
    };
    let word = format!(" {} ", tab.rule_word());
    // Right-aligned before three rule cells, but never inside the active span
    // (the Marks word is cut at the edge instead).
    let word_at = w.saturating_sub(3 + word.len()).max(active.end);
    let mut spans = Vec::new();
    let mut x = 0;
    while x < w {
        if active.contains(&x) {
            spans.push(sp(" ", Style::default().bg(theme::SELECT_BG)));
            x += 1;
        } else if x == word_at {
            spans.push(sp(word.clone(), theme::label()));
            x += word.len();
        } else {
            spans.push(sp(glyphs::H_LINE.to_string(), theme::border()));
            x += 1;
        }
    }
    Text::spans(spans)
}

pub mod rows;
#[cfg(test)]
mod tests;
