use anyhow::Result;

use oxlide::cli::{ResolvedCommand, parse_and_resolve};
use oxlide::present::run_present;

fn main() -> Result<()> {
    match parse_and_resolve() {
        ResolvedCommand::Present { path, theme } => run_present(&path, theme),
    }
}
