use std::io::{self, Stdout};
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Alignment;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

type Tui = Terminal<CrosstermBackend<Stdout>>;

struct App {
    should_quit: bool,
}

impl App {
    fn new() -> Self {
        Self { should_quit: false }
    }

    fn on_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            _ => {}
        }
    }
}

fn main() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;

    let result = run(&mut terminal);

    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;

    result
}

fn run(terminal: &mut Tui) -> Result<()> {
    let mut app = App::new();

    while !app.should_quit {
        terminal.draw(|frame| {
            let area = frame.area();
            let title = Line::from(vec![
                Span::styled("oxlide", Style::new().add_modifier(Modifier::BOLD)),
                Span::raw("  —  press "),
                Span::styled("q", Style::new().add_modifier(Modifier::BOLD)),
                Span::raw(" to quit"),
            ]);
            let block = Block::default().borders(Borders::ALL).title("oxlide");
            let paragraph = Paragraph::new(title)
                .alignment(Alignment::Center)
                .block(block);
            frame.render_widget(paragraph, area);
        })?;

        if event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            app.on_key(key.code);
        }
    }

    Ok(())
}
