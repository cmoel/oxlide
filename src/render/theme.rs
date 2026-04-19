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
    pub chrome_dim: Style,
}

impl Theme {
    /// Paper-white is the compiled default. A high-contrast, terminal-native
    /// look: fg/bg fall through to the user's terminal, weight carries heading
    /// emphasis (bold H1/H2; dim H3+), a single accent for links, and a muted
    /// gray for chrome + code borders + inline code.
    pub fn paper_white() -> Self {
        let bold = Modifier::BOLD;
        let dim = Modifier::DIM;
        let muted = Style::new().fg(Color::DarkGray);
        Self {
            name: "paper-white",
            heading: [
                Style::new().add_modifier(bold),
                Style::new().add_modifier(bold),
                Style::new().add_modifier(dim),
                Style::new().add_modifier(dim),
                Style::new().add_modifier(dim),
                Style::new().add_modifier(dim),
            ],
            prose: Style::new(),
            link: Style::new()
                .fg(Color::Blue)
                .add_modifier(Modifier::UNDERLINED),
            code: muted,
            list_marker: muted,
            image_placeholder: muted.add_modifier(Modifier::ITALIC),
            chrome_rows: 2,
            chrome: ChromeSpec::BottomRule,
            chrome_dim: muted,
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
    fn paper_white_has_bottom_rule_chrome() {
        let t = Theme::paper_white();
        assert!(matches!(t.chrome, ChromeSpec::BottomRule));
        assert_eq!(t.chrome_rows, 2);
    }

    #[test]
    fn paper_white_palette_is_reset_bg_and_muted_chrome() {
        let t = Theme::paper_white();
        // Headings carry no color — weight (bold) or dim handles emphasis.
        assert!(t.heading[0].fg.is_none());
        assert!(t.heading[2].fg.is_none());
        assert!(t.heading[0].bg.is_none());
        // Link accent is blue, underlined.
        assert_eq!(t.link.fg, Some(Color::Blue));
        assert!(t.link.add_modifier.contains(Modifier::UNDERLINED));
        // Chrome + code + inline code share the muted gray accent.
        assert_eq!(t.chrome_dim.fg, Some(Color::DarkGray));
        assert_eq!(t.code.fg, Some(Color::DarkGray));
        // No theme style fills a background — the paper metaphor sits on the
        // terminal's default bg.
        assert!(t.prose.bg.is_none());
        assert!(t.code.bg.is_none());
        assert!(t.link.bg.is_none());
        assert!(t.chrome_dim.bg.is_none());
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
