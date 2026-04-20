pub mod composition;
pub mod engine;
pub mod text;
pub mod theme;

pub use composition::{compute_inner_area, is_hero_slide};
pub use engine::{RenderContext, inline_to_line, render_cell, render_slide, render_slide_with};
pub use theme::{ChromeSpec, Theme, theme_from_deck};
