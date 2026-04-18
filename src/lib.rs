pub mod cli;
pub mod layout;
pub mod parser;
pub mod present;
pub mod render;
pub mod wake;

pub use parser::{ParseError, SlideDeck, parse_deck};
