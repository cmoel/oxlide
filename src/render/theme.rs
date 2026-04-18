use ratatui::style::{Color, Modifier, Style};

pub struct Theme {
    pub heading: [Style; 6],
    pub prose: Style,
    pub link: Style,
    pub code: Style,
    pub list_marker: Style,
    pub image_placeholder: Style,
}

impl Default for Theme {
    fn default() -> Self {
        let bold = Modifier::BOLD;
        Self {
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
        }
    }
}
