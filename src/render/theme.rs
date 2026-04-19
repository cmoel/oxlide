pub mod registry;

use ratatui::style::{Color, Modifier, Style};

use crate::parser::{Directive, SlideDeck};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChromeSpec {
    None,
    BottomRule,
}

pub struct Theme {
    pub name: &'static str,
    pub heading: [Style; 6],
    pub prose: Style,
    pub link: Style,
    pub code: Style,
    pub list_marker: Style,
    pub image_placeholder: Style,
    pub chrome_rows: u16,
    pub chrome: ChromeSpec,
}

impl Theme {
    pub fn paper_white() -> Self {
        let bold = Modifier::BOLD;
        Self {
            name: "paper-white",
            heading: [
                Style::new().fg(Color::White).add_modifier(bold),
                Style::new().fg(Color::LightCyan).add_modifier(bold),
                Style::new().fg(Color::Cyan).add_modifier(bold),
                Style::new().fg(Color::LightBlue).add_modifier(bold),
                Style::new().fg(Color::Blue),
                Style::new().fg(Color::DarkGray).add_modifier(bold),
            ],
            prose: Style::new().fg(Color::Gray),
            link: Style::new()
                .fg(Color::LightBlue)
                .add_modifier(Modifier::UNDERLINED),
            code: Style::new().fg(Color::LightYellow),
            list_marker: Style::new().fg(Color::DarkGray),
            image_placeholder: Style::new()
                .fg(Color::Magenta)
                .add_modifier(Modifier::ITALIC),
            chrome_rows: 0,
            chrome: ChromeSpec::None,
        }
    }
}

pub fn theme_from_deck(deck: &SlideDeck) -> Option<&str> {
    for directive in &deck.directives {
        let Directive::Raw { name, args, .. } = directive;
        if name == "theme" {
            return Some(args.as_str());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_deck;

    #[test]
    fn paper_white_has_expected_name() {
        let t = Theme::paper_white();
        assert_eq!(t.name, "paper-white");
    }

    #[test]
    fn paper_white_has_none_chrome() {
        let t = Theme::paper_white();
        assert!(matches!(t.chrome, ChromeSpec::None));
    }

    #[test]
    fn theme_from_deck_returns_first_theme_directive_args() {
        let src = "<!-- oxlide-theme: amber -->\n\n# Slide";
        let deck = parse_deck(src).unwrap();
        assert_eq!(theme_from_deck(&deck), Some("amber"));
    }

    #[test]
    fn theme_from_deck_returns_none_when_no_theme_directive() {
        let src = "# Slide";
        let deck = parse_deck(src).unwrap();
        assert_eq!(theme_from_deck(&deck), None);
    }

    #[test]
    fn theme_from_deck_ignores_non_theme_directives() {
        let src = "<!-- oxlide-fx: fade -->\n\n# Slide";
        let deck = parse_deck(src).unwrap();
        assert_eq!(theme_from_deck(&deck), None);
    }

    #[test]
    fn theme_from_deck_takes_first_when_multiple() {
        let src = "<!-- oxlide-theme: amber -->\n<!-- oxlide-theme: green -->\n\n# Slide";
        let deck = parse_deck(src).unwrap();
        assert_eq!(theme_from_deck(&deck), Some("amber"));
    }

    #[test]
    fn theme_from_deck_ignores_slide_level_theme_directive() {
        // Theme directive on slide 2 (after a slide break) must NOT be picked up by deck-level.
        let src = "# A\n\n---\n\n<!-- oxlide-theme: amber -->\n\n# B";
        let deck = parse_deck(src).unwrap();
        assert_eq!(theme_from_deck(&deck), None);
    }
}
