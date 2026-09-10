//! Event loop and screen cycling for the prototype viewer.

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Widget};
use ratatui::{DefaultTerminal, Frame};

use crate::prototypes::{self, Prototype};
use crate::theme;
use crate::widgets::header;

/// Fixed prototype frame size.
pub const WIDTH: u16 = 155;
pub const HEIGHT: u16 = 45;

pub struct App {
    pub screens: Vec<Box<dyn Prototype>>,
    pub current: usize,
}

impl App {
    pub fn new() -> Self {
        Self { screens: prototypes::all(), current: 0 }
    }

    fn next(&mut self) {
        self.current = (self.current + 1) % self.screens.len();
    }

    fn prev(&mut self) {
        self.current = (self.current + self.screens.len() - 1) % self.screens.len();
    }

    /// Jump to the first screen whose id starts with the given screen number.
    fn jump_to_screen_number(&mut self, n: usize) {
        let prefix = format!("S{:02}", n);
        if let Some(i) = self.screens.iter().position(|s| s.id().starts_with(&prefix)) {
            self.current = i;
        }
    }

    pub fn draw(&self, f: &mut Frame) {
        let size = f.area();
        // Paint the whole terminal with the UI background first.
        f.render_widget(
            Paragraph::new("").style(Style::default().bg(theme::BG)),
            size,
        );
        if size.width < WIDTH || size.height < HEIGHT {
            let msg = format!(
                "Resize terminal to {}x{} (now {}x{})",
                WIDTH, HEIGHT, size.width, size.height
            );
            let y = size.height / 2;
            let x = size.width.saturating_sub(msg.len() as u16) / 2;
            let area = Rect::new(x, y, msg.len().min(size.width as usize) as u16, 1);
            Paragraph::new(Line::from(msg))
                .style(Style::default().fg(Color::Yellow).bg(theme::BG))
                .render(area, f.buffer_mut());
            return;
        }
        let frame = Rect::new(0, 0, WIDTH, HEIGHT);
        let screen = &self.screens[self.current];
        let header_area = Rect::new(frame.x, frame.y, frame.width, 1);
        let body = Rect::new(frame.x, frame.y + 1, frame.width, frame.height - 1);
        header::render(f, header_area, screen.as_ref(), self.current + 1, self.screens.len());
        screen.render(f, body);
    }
}

pub fn run(terminal: &mut DefaultTerminal) -> std::io::Result<()> {
    let mut app = App::new();
    // Optional: start at a screen id given on the command line, e.g. `S03b`.
    if let Some(arg) = std::env::args().nth(1) {
        if let Some(i) = app.screens.iter().position(|s| s.id().eq_ignore_ascii_case(&arg)) {
            app.current = i;
        }
    }
    loop {
        terminal.draw(|f| app.draw(f))?;
        match event::read()? {
            Event::Key(k) if k.kind == KeyEventKind::Press => match k.code {
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) => break,
                KeyCode::Char(']') | KeyCode::PageDown | KeyCode::Right | KeyCode::Char('l') => {
                    app.next()
                }
                KeyCode::Char('[') | KeyCode::PageUp | KeyCode::Left | KeyCode::Char('h') => {
                    app.prev()
                }
                KeyCode::Home => app.current = 0,
                KeyCode::End => app.current = app.screens.len() - 1,
                KeyCode::Char(c) if c.is_ascii_digit() => {
                    app.jump_to_screen_number(c.to_digit(10).unwrap() as usize)
                }
                _ => {}
            },
            Event::Resize(_, _) => {}
            _ => {}
        }
    }
    Ok(())
}
