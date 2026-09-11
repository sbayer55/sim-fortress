//! S01: the live world map (variants a/b/d — default, wide, winter/night).

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use crate::sim::{Season, SpeciesId, World};
use crate::ui::app::AppState;
use crate::ui::style::{EventKindStyle, SeasonStyle, SpeciesStyle};
use crate::ui::viewport::{self, GUTTER_W, MAP_CHROME_ROWS, MIN_MAP_W, SIDEBAR_W};
use crate::ui::screens::{Action, Screen};
use crate::widgets::map::{self, MapData, MapOptions, Overlay};
use crate::widgets::{bars, panel, status, util};
use crate::{glyphs, theme};

pub struct WorldMap {
    pub world_name: String,
    pub origin: (usize, usize),
    /// Sidebar collapsed (S01b wide view).
    pub wide: bool,
}

impl WorldMap {
    pub fn new(world_name: String) -> Self {
        WorldMap { world_name, origin: (0, 0), wide: false }
    }
}

impl Screen for WorldMap {
    fn opaque(&self) -> bool {
        true
    }

    fn handle_key(&mut self, key: KeyEvent, _app: &mut AppState) -> Action {
        match key.code {
            KeyCode::Tab => {
                self.wide = !self.wide;
                Action::None
            }
            KeyCode::Left => {
                self.origin.0 = self.origin.0.saturating_sub(5);
                Action::None
            }
            KeyCode::Right => {
                self.origin.0 += 5;
                Action::None
            }
            KeyCode::Up => {
                self.origin.1 = self.origin.1.saturating_sub(5);
                Action::None
            }
            KeyCode::Down => {
                self.origin.1 += 5;
                Action::None
            }
            _ => Action::Unhandled,
        }
    }

    fn render(&self, app: &AppState, f: &mut Frame, area: Rect) {
        let Some(sim) = &app.sim else {
            return;
        };
        let world = &sim.world;
        let time = &sim.time;
        let night = time.is_night();
        let winter = time.season() == Season::Winter;

        let tw = area.width;
        let th = area.height;
        let map_rows = th.saturating_sub(MAP_CHROME_ROWS);
        let (map_w, side_w) = if self.wide || tw < SIDEBAR_W + MIN_MAP_W {
            (tw.saturating_sub(GUTTER_W), GUTTER_W)
        } else {
            (tw.saturating_sub(SIDEBAR_W), SIDEBAR_W)
        };

        // Clamp the viewport origin to the world for display (the stored origin
        // is updated by scrolling; render is `&self`).
        let map_inner_w = map_w.saturating_sub(2) as usize;
        let map_inner_h = map_rows.saturating_sub(2) as usize;
        let max = viewport::max_origin(world.width(), world.height(), map_inner_w, map_inner_h);
        let origin = (self.origin.0.min(max.0), self.origin.1.min(max.1));

        let map_area = Rect::new(area.x, area.y, map_w, map_rows);
        let map_inner = panel::draw_with_hint(
            f,
            map_area,
            &self.world_name,
            &map_hint(world, origin, map_inner_w),
            panel::Kind::Outer,
        );

        let opts = MapOptions {
            overlay: Overlay::None,
            night,
            winter,
            cursor: None,
            follow: None,
            origin,
            creatures: true,
            fade_creatures: false,
        };
        let empty: &[map::MapCreature] = &[];
        let data = MapData { world, creatures: empty, selected: None };
        map::render(f.buffer_mut(), map_inner, &data, &opts);

        if side_w == GUTTER_W {
            // Collapsed sidebar gutter.
            let gutter = Rect::new(area.x + map_w, area.y, side_w, map_rows);
            let inner = panel::draw(f, gutter, "", panel::Kind::Outer);
            for (i, ch) in "«SIDEBAR»".chars().enumerate() {
                let y = inner.y + 1 + i as u16;
                if y < inner.bottom() {
                    f.buffer_mut().set_stringn(inner.x, y, ch.to_string(), 1, theme::key());
                }
            }
        } else {
            let side = Rect::new(area.x + map_w, area.y, side_w, map_rows);
            self.sidebar(f, side, app, world, time);
        }

        // Ticker row.
        let ticker_row = area.y + map_rows;
        let ticker = Rect::new(area.x, ticker_row, area.width, 1);
        util::fill(f.buffer_mut(), ticker, Style::default().bg(theme::BG));
        if let Some(last) = sim.events.last() {
            util::line(
                f,
                ticker,
                0,
                Line::from(vec![
                    Span::styled(format!(" {} ", last.kind.glyph()), Style::default().fg(last.kind.color()).bg(theme::BG).add_modifier(Modifier::BOLD)),
                    Span::styled(last.text.clone(), Style::default().fg(theme::TEXT).bg(theme::BG)),
                    Span::styled("   (e: full log)", Style::default().fg(theme::DIM).bg(theme::BG)),
                ]),
            );
        }

        // Status bar.
        let status_row = area.y + area.height - 1;
        let keys: &[(&str, &str)] = &[
            ("Tab", "wide"),
            ("←→↑↓", "scroll"),
            ("Space", "pause"),
            ("+/-", "speed"),
            ("p", "controls"),
            ("?", "help"),
            ("q", "world"),
        ];
        let sky = if night { glyphs::MOON } else { glyphs::SUN };
        let skyname = if night { "night" } else { "day" };
        let right = format!("{}  {} {}", time.clock_label(), sky, skyname);
        status::render(f, Rect::new(area.x, status_row, area.width, 1), keys, &right);
    }
}

impl WorldMap {
    fn sidebar(&self, f: &mut Frame, area: Rect, app: &AppState, world: &World, time: &crate::sim::Time) {
        let inner = panel::draw(f, area, "Status", panel::Kind::Outer);
        let mut row = 0u16;
        let night = time.is_night();

        // ---- clock
        panel::section(f, inner, row, "Clock");
        row += 1;
        let season = time.season();
        util::line(f, inner, row, Line::from(vec![
            Span::styled(format!(" Year {:<3} Day {:<3}", time.year(), time.day_of_season()), theme::text()),
            Span::styled(format!("   {} {}", season.glyph(), season.name()), Style::default().fg(season.color()).bg(theme::PANEL_BG)),
        ]));
        row += 1;
        let sky = if night { (glyphs::MOON, "night") } else { (glyphs::SUN, "day") };
        let (state_glyph, state_color) = if app.paused { (glyphs::PAUSE_STR, theme::WARN) } else { (glyphs::FAST_STR, theme::GOOD) };
        util::line(f, inner, row, Line::from(vec![
            Span::styled(format!(" {:02}:00  {} {}", time.hour(), sky.0, sky.1), theme::text()),
            Span::styled(format!("      {} x{}  {}", state_glyph, app.speed(), if app.paused { "paused" } else { "running" }), Style::default().fg(state_color).bg(theme::PANEL_BG)),
        ]));
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(format!(" tick {}", group(time.tick)), theme::dim_text())));
        row += 2;

        // ---- population
        panel::section(f, inner, row, "Population");
        row += 1;
        for id in SpeciesId::ALL {
            util::line(f, inner, row, Line::from(vec![
                Span::styled(format!(" {} ", id.glyph().to_ascii_uppercase()), Style::default().fg(id.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
                Span::styled(format!("{:<6}", id.name()), theme::text()),
                Span::styled(format!("{:>5} ", "—"), theme::text()),
            ]));
            bars::sparkline(f.buffer_mut(), inner.x + 20, inner.y + row, 18, &[], id.color());
            row += 1;
        }
        util::line(f, inner, row, Line::from(Span::styled(" prey —  pred —  ratio —:1", theme::dim_text())));
        row += 2;

        // ---- resources
        panel::section(f, inner, row, "Resources");
        row += 1;
        bars::labeled(f.buffer_mut(), inner, row, " vegetation", mean_veg_land(world), theme::VEGETATION, 12, 20);
        row += 1;
        bars::labeled(f.buffer_mut(), inner, row, " water", 1.0, theme::SHALLOW_FG, 12, 20);
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(
            format!(" carcasses {:<3} dens {:<3} regrowth {}", world.carcasses.len(), world.dens.len(), world.seeds.len()),
            theme::text(),
        )));
        row += 2;

        // ---- notable
        panel::section(f, inner, row, "Notable");
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(" no creatures yet", theme::dim_text())));
        row += 2;

        // ---- legend
        panel::section(f, inner, row, "Legend");
        row += 1;
        let legend = map::legend();
        for pair in legend.chunks(2) {
            let mut spans = vec![Span::styled(" ", theme::text())];
            for (g, color, label) in pair {
                spans.push(Span::styled(g.to_string(), Style::default().fg(*color).bg(theme::PANEL_BG)));
                spans.push(Span::styled(format!(" {:<17}", label), theme::dim_text()));
            }
            util::line(f, inner, row, Line::from(spans));
            row += 1;
        }
        for chunk in SpeciesId::ALL.chunks(3) {
            let mut spans = vec![Span::styled(" ", theme::text())];
            for id in chunk {
                spans.push(Span::styled(format!("{}{}", id.glyph().to_ascii_uppercase(), id.glyph()), Style::default().fg(id.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)));
                spans.push(Span::styled(format!(" {:<10}", id.name()), theme::dim_text()));
            }
            util::line(f, inner, row, Line::from(spans));
            row += 1;
        }
        util::line(f, inner, row, Line::from(Span::styled(" UPPER adult   lower juvenile", theme::dim_text())));
    }
}

fn map_hint(world: &World, origin: (usize, usize), inner_w: usize) -> String {
    if inner_w < world.width() {
        format!("x {}-{} of {}   ← → scroll", origin.0, origin.0 + inner_w - 1, world.width())
    } else {
        format!("{}x{} cells", world.width(), world.height())
    }
}

fn mean_veg_land(world: &World) -> f32 {
    let mut sum = 0.0f32;
    let mut n = 0usize;
    for cell in &world.cells {
        if !cell.terrain.is_water() {
            sum += cell.vegetation;
            n += 1;
        }
    }
    if n == 0 {
        0.0
    } else {
        sum / n as f32
    }
}

/// Thousands separator for tick counters.
pub fn group(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}
