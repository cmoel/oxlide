use super::Theme;

type ThemeEntry = (&'static str, fn() -> Theme);

const THEMES: &[ThemeEntry] = &[("paper-white", Theme::paper_white)];

pub fn get(name: &str) -> Option<Theme> {
    THEMES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, factory)| factory())
}

pub fn cycle(current: &str) -> &'static str {
    let idx = THEMES
        .iter()
        .position(|(n, _)| *n == current)
        .unwrap_or(0);
    THEMES[(idx + 1) % THEMES.len()].0
}

pub fn names() -> impl Iterator<Item = &'static str> {
    THEMES.iter().map(|(n, _)| *n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_known_theme_returns_some() {
        assert!(get("paper-white").is_some());
    }

    #[test]
    fn get_unknown_theme_returns_none() {
        assert!(get("bogus").is_none());
    }

    #[test]
    fn cycle_single_entry_registry_returns_self() {
        assert_eq!(cycle("paper-white"), "paper-white");
    }

    #[test]
    fn cycle_unknown_returns_first_theme() {
        // Unknown input cycles back to the first theme — safe reset, no panic.
        assert_eq!(cycle("bogus"), "paper-white");
    }

    #[test]
    fn names_contains_paper_white() {
        let list: Vec<&str> = names().collect();
        assert!(list.contains(&"paper-white"));
    }

    #[test]
    fn names_nonempty() {
        assert!(names().next().is_some());
    }
}
