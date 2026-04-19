pub mod composition;
pub mod engine;
pub mod theme;

pub use composition::{compute_inner_area, is_hero_slide};
pub use engine::{inline_to_line, render_cell, render_slide};
pub use theme::{ChromeSpec, Theme, theme_from_deck};
