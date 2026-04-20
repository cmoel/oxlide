//! Present mode: the main event loop that drives a deck from the keyboard.

use std::fs;
use std::io::{self, Stdout};
use std::panic;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui_image::picker::Picker;

use crate::parser::{SlideDeck, parse_deck};
use crate::render::{RenderContext, render_slide_with};
use crate::render::theme::{registry, theme_from_deck};
use crate::wake::PresentationLock;

type Tui = Terminal<CrosstermBackend<Stdout>>;

const DEFAULT_THEME: &str = "paper-white";

struct App {
    deck: SlideDeck,
    deck_dir: Option<PathBuf>,
    current_slide: usize,
    should_quit: bool,
    current_theme: &'static str,
    picker: Option<Picker>,
}

impl App {
    fn new(deck: SlideDeck, current_theme: &'static str) -> Self {
        Self {
            deck,
            deck_dir: None,
            current_slide: 0,
            should_quit: false,
            current_theme,
            picker: None,
        }
    }

    fn last_index(&self) -> usize {
        self.deck.slides.len().saturating_sub(1)
    }

    fn on_key(&mut self, code: KeyCode, mods: KeyModifiers) {
        match code {
            KeyCode::Char(' ') | KeyCode::Right | KeyCode::PageDown
                if self.current_slide < self.last_index() =>
            {
                self.current_slide += 1;
            }
            KeyCode::Left | KeyCode::PageUp => {
                self.current_slide = self.current_slide.saturating_sub(1);
            }
            KeyCode::Home => {
                self.current_slide = 0;
            }
            KeyCode::End => {
                self.current_slide = self.last_index();
            }
            KeyCode::Char('T') => {
                self.current_theme = registry::cycle(self.current_theme);
            }
            KeyCode::Char('q') | KeyCode::Esc => {
                self.should_quit = true;
            }
            KeyCode::Char('c') if mods.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            _ => {}
        }
    }
}

pub fn run_present(path: &Path, theme_override: Option<String>) -> Result<()> {
    let source = fs::read_to_string(path)
        .with_context(|| format!("reading deck: {}", path.display()))?;
    let deck = parse_deck(&source)
        .with_context(|| format!("parsing deck: {}", path.display()))?;

    if deck.slides.is_empty() {
        return Err(anyhow!("deck has no slides: {}", path.display()));
    }

    let theme_name = resolve_theme_name(&deck, theme_override.as_deref())?;

    let _lock = PresentationLock::new();

    install_panic_hook();

    enable_raw_mode().context("enabling raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).context("entering alternate screen")?;
    let mut terminal =
        Terminal::new(CrosstermBackend::new(stdout)).context("creating terminal")?;

    // Detect the terminal's image protocol once. Per ratatui-image docs this
    // must happen after entering the alternate screen but before reading
    // events. On failure (headless test env, terminal that doesn't respond
    // to capability queries), fall through to halfblocks rendering. Either
    // way, image rendering must never block startup.
    let picker = Picker::from_query_stdio()
        .ok()
        .or_else(|| Some(Picker::halfblocks()));

    let mut app = App::new(deck, theme_name);
    app.deck_dir = path.parent().map(Path::to_path_buf);
    app.picker = picker;

    let result = event_loop(&mut terminal, app);

    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen);

    result
}

fn resolve_theme_name(deck: &SlideDeck, cli_theme: Option<&str>) -> Result<&'static str> {
    if let Some(name) = cli_theme {
        return match registry::names().find(|n| *n == name) {
            Some(n) => Ok(n),
            None => {
                let valid: Vec<&str> = registry::names().collect();
                Err(anyhow!(
                    "unknown theme '{}'. Valid themes: {}",
                    name,
                    valid.join(", ")
                ))
            }
        };
    }
    if let Some(directive_name) = theme_from_deck(deck) {
        if let Some(n) = registry::names().find(|n| *n == directive_name) {
            return Ok(n);
        }
        eprintln!(
            "warning: unknown theme '{}' in deck directive; falling back to {}",
            directive_name, DEFAULT_THEME
        );
    }
    registry::names()
        .find(|n| *n == DEFAULT_THEME)
        .ok_or_else(|| anyhow!("default theme '{}' not registered", DEFAULT_THEME))
}

fn event_loop(terminal: &mut Tui, mut app: App) -> Result<()> {
    while !app.should_quit {
        terminal.draw(|frame| {
            let area = frame.area();
            let buf = frame.buffer_mut();
            let slide = &app.deck.slides[app.current_slide];
            let total = app.deck.slides.len();
            let theme = registry::get(app.current_theme)
                .expect("registry invariant: current_theme must be registered");
            let ctx = RenderContext {
                theme: &theme,
                deck_dir: app.deck_dir.as_deref(),
                picker: app.picker.as_ref(),
            };
            render_slide_with(slide, app.current_slide, total, area, buf, &ctx);
        })?;

        if event::poll(Duration::from_millis(100))? {
            handle_event(event::read()?, &mut app);
        }
    }

    Ok(())
}

fn handle_event(event: Event, app: &mut App) {
    match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => {
            app.on_key(key.code, key.modifiers);
        }
        Event::Resize(_, _) => {
            // Resize is handled implicitly by the next loop iteration: terminal.draw
            // re-reads frame.area() each frame, so no cached dimensions to invalidate.
            // We match this arm explicitly (instead of letting it fall through _) so
            // that the event is acknowledged as a first-class render trigger.
        }
        _ => {}
    }
}

fn install_panic_hook() {
    let prior = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        prior(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{Cell, Directive, Slide, SourceSpan, parse_deck};

    fn span() -> SourceSpan {
        SourceSpan { start: 0, end: 0 }
    }

    fn make_deck(n: usize) -> SlideDeck {
        let slides = (0..n)
            .map(|_| Slide {
                cells: vec![Cell {
                    blocks: vec![],
                    directives: vec![],
                    span: span(),
                }],
                notes: vec![],
                directives: vec![],
                span: span(),
            })
            .collect();
        SlideDeck {
            slides,
            directives: vec![],
            source: String::new(),
        }
    }

    fn app(n: usize) -> App {
        App::new(make_deck(n), "paper-white")
    }

    #[test]
    fn space_advances_to_next_slide() {
        let mut a = app(3);
        a.on_key(KeyCode::Char(' '), KeyModifiers::NONE);
        assert_eq!(a.current_slide, 1);
    }

    #[test]
    fn right_advances_to_next_slide() {
        let mut a = app(3);
        a.on_key(KeyCode::Right, KeyModifiers::NONE);
        assert_eq!(a.current_slide, 1);
    }

    #[test]
    fn pagedown_advances_to_next_slide() {
        let mut a = app(3);
        a.on_key(KeyCode::PageDown, KeyModifiers::NONE);
        assert_eq!(a.current_slide, 1);
    }

    #[test]
    fn advancing_past_last_stays_at_last() {
        let mut a = app(3);
        for _ in 0..10 {
            a.on_key(KeyCode::Right, KeyModifiers::NONE);
        }
        assert_eq!(a.current_slide, 2);
    }

    #[test]
    fn left_retreats_one_slide() {
        let mut a = app(3);
        a.current_slide = 2;
        a.on_key(KeyCode::Left, KeyModifiers::NONE);
        assert_eq!(a.current_slide, 1);
    }

    #[test]
    fn pageup_retreats_one_slide() {
        let mut a = app(3);
        a.current_slide = 2;
        a.on_key(KeyCode::PageUp, KeyModifiers::NONE);
        assert_eq!(a.current_slide, 1);
    }

    #[test]
    fn retreating_before_first_stays_at_zero() {
        let mut a = app(3);
        for _ in 0..10 {
            a.on_key(KeyCode::Left, KeyModifiers::NONE);
        }
        assert_eq!(a.current_slide, 0);
    }

    #[test]
    fn home_jumps_to_first() {
        let mut a = app(3);
        a.current_slide = 2;
        a.on_key(KeyCode::Home, KeyModifiers::NONE);
        assert_eq!(a.current_slide, 0);
    }

    #[test]
    fn end_jumps_to_last() {
        let mut a = app(3);
        a.on_key(KeyCode::End, KeyModifiers::NONE);
        assert_eq!(a.current_slide, 2);
    }

    #[test]
    fn q_sets_quit() {
        let mut a = app(3);
        a.on_key(KeyCode::Char('q'), KeyModifiers::NONE);
        assert!(a.should_quit);
    }

    #[test]
    fn esc_sets_quit() {
        let mut a = app(3);
        a.on_key(KeyCode::Esc, KeyModifiers::NONE);
        assert!(a.should_quit);
    }

    #[test]
    fn ctrl_c_sets_quit() {
        let mut a = app(3);
        a.on_key(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert!(a.should_quit);
    }

    #[test]
    fn plain_c_does_not_quit() {
        let mut a = app(3);
        a.on_key(KeyCode::Char('c'), KeyModifiers::NONE);
        assert!(!a.should_quit);
    }

    #[test]
    fn single_slide_deck_end_stays_at_zero() {
        let mut a = app(1);
        a.on_key(KeyCode::End, KeyModifiers::NONE);
        assert_eq!(a.current_slide, 0);
        a.on_key(KeyCode::Right, KeyModifiers::NONE);
        assert_eq!(a.current_slide, 0);
    }

    #[test]
    fn shift_t_cycles_theme() {
        let mut a = app(1);
        let start = a.current_theme;
        a.on_key(KeyCode::Char('T'), KeyModifiers::SHIFT);
        assert_eq!(a.current_theme, registry::cycle(start));
    }

    #[test]
    fn shift_t_without_shift_modifier_still_cycles() {
        // Some terminals deliver uppercase Char without reporting the SHIFT modifier.
        let mut a = app(1);
        let start = a.current_theme;
        a.on_key(KeyCode::Char('T'), KeyModifiers::NONE);
        assert_eq!(a.current_theme, registry::cycle(start));
    }

    #[test]
    fn lowercase_t_does_not_cycle_theme() {
        let mut a = app(1);
        let start = a.current_theme;
        a.on_key(KeyCode::Char('t'), KeyModifiers::NONE);
        assert_eq!(a.current_theme, start);
    }

    #[test]
    fn resize_event_accepted_not_filtered() {
        let mut a = app(3);
        handle_event(Event::Resize(80, 24), &mut a);
        assert!(!a.should_quit);
        assert_eq!(a.current_slide, 0);
    }

    #[test]
    fn resize_event_repeated_never_crashes_or_quits() {
        let mut a = app(3);
        for (w, h) in [(80, 24), (40, 10), (200, 60), (1, 1)] {
            handle_event(Event::Resize(w, h), &mut a);
        }
        assert!(!a.should_quit);
    }

    #[test]
    fn key_event_still_dispatched_via_handle_event() {
        let mut a = app(3);
        let ev = Event::Key(crossterm::event::KeyEvent::new(
            KeyCode::Right,
            KeyModifiers::NONE,
        ));
        handle_event(ev, &mut a);
        assert_eq!(a.current_slide, 1);
    }

    fn deck_from(src: &str) -> SlideDeck {
        parse_deck(src).unwrap()
    }

    #[test]
    fn resolve_theme_cli_overrides_directive() {
        let deck = deck_from("<!-- oxlide-theme: paper-white -->\n\n# Slide");
        // With a single-entry registry we can only assert the happy path here.
        let name = resolve_theme_name(&deck, Some("paper-white")).unwrap();
        assert_eq!(name, "paper-white");
    }

    #[test]
    fn resolve_theme_cli_unknown_errors_with_list() {
        let deck = deck_from("# Slide");
        let err = resolve_theme_name(&deck, Some("bogus")).unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("bogus"), "error should mention bad name: {}", msg);
        assert!(
            msg.contains("paper-white"),
            "error should list valid names: {}",
            msg
        );
    }

    #[test]
    fn resolve_theme_directive_used_when_no_cli() {
        let deck = deck_from("<!-- oxlide-theme: paper-white -->\n\n# Slide");
        let name = resolve_theme_name(&deck, None).unwrap();
        assert_eq!(name, "paper-white");
    }

    #[test]
    fn resolve_theme_falls_back_when_directive_unknown() {
        // Unknown directive name → falls back to default, does NOT error.
        let mut deck = deck_from("# Slide");
        deck.directives.push(Directive::Raw {
            name: "theme".into(),
            args: "bogus".into(),
            span: span(),
        });
        let name = resolve_theme_name(&deck, None).unwrap();
        assert_eq!(name, DEFAULT_THEME);
    }

    #[test]
    fn resolve_theme_default_when_no_cli_no_directive() {
        let deck = deck_from("# Slide");
        let name = resolve_theme_name(&deck, None).unwrap();
        assert_eq!(name, DEFAULT_THEME);
    }
}
