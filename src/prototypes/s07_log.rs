//! S07: the event log. S07a lists every event newest first; S07b filters to
//! deaths and extinctions and shows a detail panel with a mini-map.

#[allow(unused_imports)]
use crate::fixtures::{EventKindStyle as _, SeasonStyle as _, SpeciesStyle as _};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget, Wrap};
use ratatui::Frame;

use super::Prototype;
use crate::fixtures::{self, Event, EventKind, Fixtures, Kind};
use crate::widgets::map::{self, MapOptions, Overlay};
use crate::widgets::{panel, status, util};
use crate::{glyphs, theme};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    Full,
    Deaths,
}

pub struct EventLog {
    pub variant: Variant,
}

const DETAIL_W: u16 = 55;
const MINI_W: u16 = 25;
const MINI_H: u16 = 9;

pub fn all() -> Vec<Box<dyn Prototype>> {
    vec![Box::new(EventLog { variant: Variant::Full }), Box::new(EventLog { variant: Variant::Deaths })]
}

impl Prototype for EventLog {
    fn id(&self) -> &'static str {
        match self.variant {
            Variant::Full => "S07a",
            Variant::Deaths => "S07b",
        }
    }
    fn name(&self) -> &'static str {
        "Event Log"
    }
    fn variant(&self) -> &'static str {
        match self.variant {
            Variant::Full => "full log",
            Variant::Deaths => "deaths & extinctions with detail",
        }
    }

    fn render(&self, f: &mut Frame, area: Rect) {
        let fx = fixtures::get();
        let body_h = area.height - 1;
        let deaths_only = self.variant == Variant::Deaths;
        // Newest first.
        let events: Vec<&Event> = fx
            .events
            .iter()
            .rev()
            .filter(|e| !deaths_only || e.kind.is_death() || e.kind == EventKind::Extinction)
            .collect();
        let selected = 0usize;

        let list_w = if deaths_only { area.width - DETAIL_W } else { area.width };
        let list_area = Rect::new(area.x, area.y, list_w, body_h);
        let title = if deaths_only { "Event Log — deaths & extinctions" } else { "Event Log" };
        let hint = format!("{} events, {} today", events.len(), events.iter().filter(|e| e.year == fx.clock.year && e.day == fx.clock.day).count());
        let inner = panel::draw_with_hint(f, list_area, title, &hint, panel::Kind::Outer);
        chips(f, inner, deaths_only);
        list(f, Rect::new(inner.x, inner.y + 2, inner.width, inner.height - 2), fx, &events, selected, deaths_only);

        if deaths_only {
            let detail_area = Rect::new(area.x + list_w, area.y, DETAIL_W, body_h);
            detail(f, detail_area, fx, events[selected]);
        }

        status::render(
            f,
            Rect::new(area.x, area.y + area.height - 1, area.width, 1),
            &[("↑↓", "select"), ("f", "filter"), ("Enter", "jump"), ("Esc", "back")],
            &fx.clock.label().to_string(),
        );
    }
}

fn sp(s: impl Into<String>, fg: Color, bg: Color) -> Span<'static> {
    Span::styled(s.into(), Style::default().fg(fg).bg(bg))
}

fn bold(s: impl Into<String>, fg: Color, bg: Color) -> Span<'static> {
    Span::styled(s.into(), Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD))
}

/// Filter chip row: `all births deaths ...` with the active chips highlighted.
fn chips(f: &mut Frame, inner: Rect, deaths_only: bool) {
    let chips: [(&str, EventKind); 7] = [
        ("all", EventKind::Note),
        ("births", EventKind::Birth),
        ("deaths", EventKind::DeathPredation),
        ("mutations", EventKind::Mutation),
        ("migrations", EventKind::Migration),
        ("extinctions", EventKind::Extinction),
        ("droughts", EventKind::Drought),
    ];
    let mut spans = vec![sp(" filter: ", theme::DIM, theme::PANEL_BG)];
    for (name, kind) in chips {
        let on = if deaths_only { name == "deaths" || name == "extinctions" } else { name == "all" };
        if on {
            spans.push(Span::styled(format!(" {} ", name), theme::selected()));
        } else {
            let g = if name == "all" { '*' } else { kind.glyph() };
            spans.push(sp(format!(" {} ", g), kind.color(), theme::PANEL_BG));
            spans.push(sp(format!("{} ", name), theme::TEXT, theme::PANEL_BG));
        }
        spans.push(sp(" ", theme::TEXT, theme::PANEL_BG));
    }
    if inner.width > 120 {
        spans.push(sp("  [f] cycles, [1-7] toggles", theme::DIM, theme::PANEL_BG));
    }
    util::line(f, inner, 0, Line::from(spans));
    util::line(f, inner, 1, Line::from(sp(
        format!("{:<16}{:<14}{:<4}{}", " when", "kind", "sp", "event"),
        theme::DIM,
        theme::PANEL_BG,
    )));
}

fn list(f: &mut Frame, area: Rect, fx: &Fixtures, events: &[&Event], selected: usize, compact: bool) {
    let rows = area.height as usize;
    let scroll_x = area.right() - 1;
    let text_w = area.width - 2; // leave the scrollbar column and a gap
    for (i, e) in events.iter().take(rows).enumerate() {
        let y = area.y + i as u16;
        let is_sel = i == selected;
        let bg = if is_sel { theme::SELECT_BG } else { theme::PANEL_BG };
        let row = Rect::new(area.x, y, text_w, 1);
        util::fill(f.buffer_mut(), row, Style::default().bg(bg));
        let (sg, sc) = match e.species {
            Some(s) => (s.glyph().to_ascii_uppercase(), s.color()),
            None => ('-', theme::DIM),
        };
        let when_color = if is_sel { theme::TEXT_BRIGHT } else if e.year == fx.clock.year && e.day == fx.clock.day { theme::TEXT } else { theme::DIM };
        let pos_w = 9usize;
        let text_max = text_w as usize - 34 - if compact { 0 } else { pos_w };
        let text = clip(&e.text, text_max);
        let mut spans = vec![
            sp(format!("{}Y{:<2} D{:03} {:02}:00  ", if is_sel { glyphs::PLAY } else { ' ' }, e.year, e.day, e.hour), when_color, bg),
            bold(format!("{} ", e.kind.glyph()), e.kind.color(), bg),
            sp(format!("{:<11}", e.kind.label()), e.kind.color(), bg),
            bold(format!("{}   ", sg), sc, bg),
            sp(text, if is_sel { theme::TEXT_BRIGHT } else { theme::TEXT }, bg),
        ];
        if is_sel {
            spans[4].style = spans[4].style.add_modifier(Modifier::BOLD);
        }
        util::line(f, row, 0, Line::from(spans));
        if !compact {
            let pos = match e.pos {
                Some((x, y)) => format!("({:>3},{:>2})", x, y),
                None => String::new(),
            };
            let buf = f.buffer_mut();
            buf.set_stringn(row.right() - pos_w as u16, y, &pos, pos_w, Style::default().fg(theme::DIM).bg(bg));
        }
    }
    // Scrollbar: track of ░ with a █ thumb sized to the visible fraction.
    let total = events.len().max(1);
    let thumb = ((rows as f32 / total as f32) * rows as f32).ceil().clamp(1.0, rows as f32) as usize;
    let buf = f.buffer_mut();
    for r in 0..rows {
        let g = if r < thumb { glyphs::SHADE_4 } else { glyphs::SHADE_1 };
        let color = if r < thumb { theme::BORDER } else { theme::dim(theme::BORDER, 0.5) };
        buf.set_stringn(scroll_x, area.y + r as u16, g.to_string(), 1, Style::default().fg(color).bg(theme::PANEL_BG));
    }
    if events.len() > rows {
        // On the panel's bottom border, so no row loses its position column.
        let more = format!(" {} more {} ", events.len() - rows, glyphs::DOWN);
        let w = more.chars().count();
        buf.set_stringn(area.right() - 2 - w as u16, area.bottom(), &more, w, theme::dim_text());
    }
}

fn detail(f: &mut Frame, area: Rect, fx: &Fixtures, e: &Event) {
    let inner = panel::draw(f, area, "Event detail", panel::Kind::Focus);
    let bg = theme::PANEL_BG;
    let mut row = 0;
    util::line(f, inner, row, Line::from(vec![
        bold(format!(" {} ", e.kind.glyph()), e.kind.color(), bg),
        bold(e.kind.label().to_string(), e.kind.color(), bg),
        sp(format!("   Year {}, Day {}, {:02}:00", e.year, e.day, e.hour), theme::DIM, bg),
    ]));
    row += 1;
    let text_lines = wrapped_height(&e.text, inner.width - 2);
    Paragraph::new(Line::from(Span::styled(e.text.clone(), Style::default().fg(theme::TEXT_BRIGHT).bg(bg).add_modifier(Modifier::BOLD))))
        .wrap(Wrap { trim: true })
        .render(Rect::new(inner.x + 1, inner.y + row, inner.width - 2, text_lines), f.buffer_mut());
    row += text_lines + 1;

    panel::section(f, inner, row, "Detail");
    row += 1;
    let detail_lines = wrapped_height(&e.detail, inner.width - 2).min(5);
    Paragraph::new(Line::from(Span::styled(e.detail.clone(), Style::default().fg(theme::TEXT).bg(bg))))
        .wrap(Wrap { trim: true })
        .render(Rect::new(inner.x + 1, inner.y + row, inner.width - 2, detail_lines), f.buffer_mut());
    row += detail_lines + 1;

    panel::section(f, inner, row, "Who");
    row += 1;
    match e.species {
        Some(s) => {
            let kind = match s.kind() {
                Kind::Prey => "prey",
                Kind::Predator => "predator",
            };
            let sp_ = &fx.species[fixtures::SpeciesId::ALL.iter().position(|&x| x == s).unwrap()];
            util::line(f, inner, row, Line::from(vec![
                bold(format!(" {} ", s.glyph().to_ascii_uppercase()), s.color(), bg),
                bold(s.name().to_string(), s.color(), bg),
                sp(format!("  {}, eats {}", kind, s.diet()), theme::TEXT, bg),
            ]));
            row += 1;
            util::line(f, inner, row, Line::from(sp(
                format!("   {} alive today, {} born / {} died", sp_.count, sp_.births_today, sp_.deaths_today),
                theme::DIM,
                bg,
            )));
            row += 1;
            // Try to find the named creature for a vitals line.
            if let Some(c) = fx.creatures.iter().find(|c| c.species == s && e.text.contains(&c.tag())) {
                let state = if c.alive { "alive".to_string() } else { format!("dead: {}", c.cause_of_death.clone().unwrap_or_default()) };
                util::line(f, inner, row, Line::from(vec![
                    sp(format!("   {} {}  ", c.name, c.tag()), theme::TEXT, bg),
                    sp(format!("gen {}  age {}d  {}", c.generation, c.age_days, state), theme::DIM, bg),
                ]));
                row += 1;
            }
        }
        None => {
            util::line(f, inner, row, Line::from(sp(" whole valley", theme::DIM, bg)));
            row += 1;
        }
    }
    row += 1;

    panel::section(f, inner, row, "Where");
    row += 1;
    match e.pos {
        Some((x, y)) => {
            let cell = fx.world.cell(x, y);
            util::line(f, inner, row, Line::from(vec![
                sp(format!(" ({}, {})  ", x, y), theme::TEXT, bg),
                bold(fx.world.region_name(x, y).to_string(), theme::ACCENT, bg),
                sp(format!("  {}  veg {:.2}", cell.terrain.name(), cell.vegetation), theme::DIM, bg),
            ]));
            row += 1;
            // Mini-map centred on the event position.
            let world = &fx.world;
            let ox = (x as i64 - MINI_W as i64 / 2).clamp(0, world.width() as i64 - MINI_W as i64) as usize;
            let oy = (y as i64 - MINI_H as i64 / 2).clamp(0, world.height() as i64 - MINI_H as i64) as usize;
            let mini = Rect::new(inner.x + 1, inner.y + row, MINI_W + 2, MINI_H + 2);
            let mini_inner = panel::draw(f, mini, "", panel::Kind::Inner);
            let opts = MapOptions {
                overlay: Overlay::None,
                night: false,
                winter: false,
                cursor: Some((x, y)),
                follow: None,
                origin: (ox, oy),
                creatures: true,
                fade_creatures: false,
            };
            let creatures = fx.map_creatures();
            let data = map::MapData { world: &fx.world, creatures: &creatures, selected: None };
            map::render(f.buffer_mut(), mini_inner, &data, &opts);
            // Nearby creatures listed beside the map.
            let lx = mini.right() + 1;
            let lw = inner.right() - lx;
            let near: Vec<&fixtures::Creature> = fx
                .creatures
                .iter()
                .filter(|c| c.alive && (c.x as i64 - x as i64).abs() <= 12 && (c.y as i64 - y as i64).abs() <= 4)
                .take(MINI_H as usize - 1)
                .collect();
            let buf = f.buffer_mut();
            buf.set_stringn(lx, mini.y + 1, format!("nearby ({}):", near.len()), lw as usize, theme::label());
            for (i, c) in near.iter().enumerate() {
                let y = mini.y + 2 + i as u16;
                buf.set_stringn(lx, y, c.glyph().to_string(), 1, Style::default().fg(c.species.color()).bg(bg).add_modifier(Modifier::BOLD));
                buf.set_stringn(lx + 2, y, format!("{} {}", c.name, c.tag()), lw as usize - 2, theme::text());
            }
            if near.is_empty() {
                buf.set_stringn(lx, mini.y + 2, "no one within 12 cells", lw as usize, theme::dim_text());
            }
            buf.set_stringn(lx, mini.bottom() - 1, format!("{} = event cell", glyphs::CURSOR), lw as usize, theme::dim_text());
            row += MINI_H + 2;
        }
        None => {
            util::line(f, inner, row, Line::from(sp(" no position (valley-wide event)", theme::DIM, bg)));
            row += 1;
        }
    }
    row += 1;
    util::line(f, inner, row, Line::from(vec![
        Span::styled(" [Enter]", theme::key()),
        sp(" jump to map   ", theme::TEXT, bg),
        Span::styled("[i]", theme::key()),
        sp(" inspect creature", theme::TEXT, bg),
    ]));
    row += 1;
    util::line(f, inner, row, Line::from(vec![
        Span::styled(" [l]", theme::key()),
        sp(" lineage       ", theme::TEXT, bg),
        Span::styled("[s]", theme::key()),
        sp(" species screen", theme::TEXT, bg),
    ]));
    row += 2;

    // Related events for the same species.
    panel::section(f, inner, row, "Same species, recent");
    row += 1;
    let related: Vec<&Event> = fx
        .events
        .iter()
        .rev()
        .filter(|o| o.species == e.species && !std::ptr::eq(*o, e))
        .take((inner.height - row) as usize)
        .collect();
    for o in related {
        util::line(f, inner, row, Line::from(vec![
            sp(format!(" D{:03} ", o.day), theme::DIM, bg),
            bold(format!("{} ", o.kind.glyph()), o.kind.color(), bg),
            sp(clip(&o.text, inner.width as usize - 9), theme::TEXT, bg),
        ]));
        row += 1;
    }
}

/// Clip to `max` cells, marking the cut with `~`.
fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        let mut t: String = text.chars().take(max.saturating_sub(1)).collect();
        t.push('~');
        t
    }
}

/// Number of rows a greedy word wrap of `text` needs at `width`.
fn wrapped_height(text: &str, width: u16) -> u16 {
    let width = width.max(1) as usize;
    let mut lines = 1u16;
    let mut cur = 0usize;
    for w in text.split_whitespace() {
        let wl = w.chars().count();
        if cur > 0 && cur + 1 + wl > width {
            lines += 1;
            cur = wl;
        } else {
            cur += if cur > 0 { 1 + wl } else { wl };
        }
    }
    lines
}
