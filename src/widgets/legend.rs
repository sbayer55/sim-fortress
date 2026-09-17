//! The glyph, colour and description grid that explains the map.
//! See `docs/components/legend.md`.

use std::borrow::Cow;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

use super::component::Component;
use super::text::Text;
use crate::sim::{Kind, Roster};
use crate::theme;

#[cfg(test)]
mod tests;

/// One species row entry, resolved from the roster.
#[derive(Clone, Debug)]
struct SpeciesEntry {
    adult: char,
    juvenile: char,
    color: Color,
    name: String,
    role: (&'static str, Color),
    diet: String,
}

impl SpeciesEntry {
    fn all(roster: &Roster) -> Vec<Self> {
        roster
            .ids()
            .map(|id| {
                let glyph = roster.get(id).glyph;
                let [r, g, b] = roster.get(id).color;
                Self {
                    adult: glyph.to_ascii_uppercase(),
                    juvenile: glyph,
                    color: Color::Rgb(r, g, b),
                    name: roster.display_name(id),
                    role: match roster.kind(id) {
                        Kind::Prey => ("prey", theme::GOOD),
                        Kind::Predator => ("predator", theme::BAD),
                    },
                    diet: String::from(roster.get(id).diet.as_str()),
                }
            })
            .collect()
    }
}

const FOOTER: &str = " UPPER adult   lower juvenile";

/// Entry rows in `columns`, optional species rows in threes, and the Footer.
#[derive(Clone, Debug)]
pub struct Legend<'a> {
    entries: Vec<(char, Color, Cow<'a, str>)>,
    notes: &'a [&'a str],
    columns: u16,
    label_w: u16,
    species: Vec<SpeciesEntry>,
    /// Help: glyph bold, label in `theme::text()`.
    help: bool,
    /// Creatures: the S11 `ad jv  species role  diet` table.
    creatures: bool,
    footer: bool,
}

impl<'a> Legend<'a> {
    /// Any glyph/colour/label list, in the Sidebar style: two columns, `label_w` 17.
    pub fn new(entries: &[(char, Color, &'a str)]) -> Self {
        Self {
            entries: entries.iter().map(|&(g, c, l)| (g, c, Cow::Borrowed(l))).collect(),
            notes: &[],
            columns: 2,
            label_w: 17,
            species: Vec::new(),
            help: false,
            creatures: false,
            footer: false,
        }
    }

    /// The map legend from `map::legend()`.
    pub fn map() -> Self {
        let entries: Vec<(char, Color, &'static str)> = super::map::legend();
        Self::new(&entries)
    }

    /// The S11 Creatures table: a header, one row per species, the Footer.
    pub fn creatures(roster: &Roster) -> Self {
        let mut l = Self::new(&[]);
        l.species = SpeciesEntry::all(roster);
        l.creatures = true;
        l.footer = true;
        l
    }

    #[must_use]
    pub fn columns(mut self, columns: u16) -> Self {
        self.columns = columns.max(1);
        self
    }

    #[must_use]
    pub const fn label_w(mut self, label_w: u16) -> Self {
        self.label_w = label_w;
        self
    }

    /// Help style: bold glyph, text label.
    #[must_use]
    pub const fn help(mut self) -> Self {
        self.help = true;
        self
    }

    /// One note per entry after the Label (Help style).
    #[must_use]
    pub const fn notes(mut self, notes: &'a [&'a str]) -> Self {
        self.notes = notes;
        self.help = true;
        self
    }

    /// Species rows in threes (`Vv Vole`) and the Footer.
    #[must_use]
    pub fn species(mut self, roster: &Roster) -> Self {
        self.species = SpeciesEntry::all(roster);
        self.footer = true;
        self
    }

    #[must_use]
    pub const fn footer(mut self) -> Self {
        self.footer = true;
        self
    }

    fn entry_rows(&self) -> Vec<Text<'a>> {
        let label_style = if self.help { theme::text() } else { theme::dim_text() };
        let lw = crate::cast!(self.label_w => usize);
        self.entries
            .chunks(crate::cast!(self.columns => usize))
            .enumerate()
            .map(|(r, chunk)| {
                let mut spans = vec![Span::styled(" ", theme::text())];
                for (c, (g, color, label)) in chunk.iter().enumerate() {
                    let mut glyph = Style::default().fg(*color).bg(theme::PANEL_BG);
                    if self.help {
                        glyph = glyph.add_modifier(Modifier::BOLD);
                    }
                    spans.push(Span::styled(g.to_string(), glyph));
                    spans.push(Span::styled(format!(" {label:<lw$}"), label_style));
                    if let Some(note) = self.notes.get(r * crate::cast!(self.columns => usize) + c) {
                        spans.push(Span::styled(*note, theme::dim_text()));
                    }
                }
                Text::spans(spans)
            })
            .collect()
    }

    fn species_rows(&self) -> Vec<Text<'a>> {
        self.species
            .chunks(3)
            .map(|chunk| {
                let mut spans = vec![Span::styled(" ", theme::text())];
                for s in chunk {
                    let style = Style::default().fg(s.color).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD);
                    spans.push(Span::styled(format!("{}{}", s.adult, s.juvenile), style));
                    spans.push(Span::styled(format!(" {:<10}", s.name), theme::dim_text()));
                }
                Text::spans(spans)
            })
            .collect()
    }

    fn creature_rows(&self) -> Vec<Text<'a>> {
        let mut rows = vec![Text::new(" ad jv  species role      diet").style(theme::label())];
        rows.extend(self.species.iter().map(|s| {
            Text::spans(vec![
                Span::styled(format!(" {}  {}  ", s.adult, s.juvenile), Style::default().fg(s.color).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
                Span::styled(format!("{:<7}", s.name), theme::text()),
                Span::styled(format!("{:<10}", s.role.0), Style::default().fg(s.role.1).bg(theme::PANEL_BG)),
                Span::styled(s.diet.clone(), theme::dim_text()),
            ])
        }));
        rows
    }

    fn rows(&self) -> Vec<Text<'a>> {
        let mut rows = self.entry_rows();
        if self.creatures {
            rows.extend(self.creature_rows());
        } else {
            rows.extend(self.species_rows());
        }
        if self.footer {
            rows.push(Text::new(FOOTER).style(theme::dim_text()));
        }
        rows
    }
}

impl Component for Legend<'_> {
    fn height(&self, _width: u16) -> u16 {
        crate::cast!(self.rows().len() => u16)
    }

    fn min_width(&self) -> u16 {
        1 + self.columns * (2 + self.label_w)
    }

    fn render(&self, buf: &mut Buffer, area: Rect) {
        for (i, row) in self.rows().iter().enumerate() {
            let y = area.y.saturating_add(crate::cast!(i => u16));
            if y >= area.bottom() {
                break;
            }
            row.render(buf, Rect::new(area.x, y, area.width, 1));
        }
    }
}
