use std::path::PathBuf;

use clap::{CommandFactory, Parser, Subcommand, error::ErrorKind};

#[derive(Debug, Parser)]
#[command(name = "oxlide", version, about = "TUI markdown presenter")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Path to a markdown deck to present. Equivalent to `oxlide present <path>`.
    pub path: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Present a markdown deck.
    Present {
        /// Path to the markdown deck.
        path: PathBuf,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedCommand {
    Present(PathBuf),
}

impl Cli {
    pub fn resolve(self) -> Result<ResolvedCommand, clap::Error> {
        match (self.command, self.path) {
            (Some(Command::Present { .. }), Some(_)) => {
                let mut cmd = Cli::command();
                Err(cmd.error(
                    ErrorKind::ArgumentConflict,
                    "cannot specify both a positional path and an explicit subcommand",
                ))
            }
            (Some(Command::Present { path }), None) => Ok(ResolvedCommand::Present(path)),
            (None, Some(path)) => Ok(ResolvedCommand::Present(path)),
            (None, None) => {
                let mut cmd = Cli::command();
                Err(cmd.error(
                    ErrorKind::MissingSubcommand,
                    "a deck path or subcommand is required",
                ))
            }
        }
    }
}

pub fn parse_and_resolve() -> ResolvedCommand {
    let cli = Cli::parse();
    match cli.resolve() {
        Ok(cmd) => cmd,
        Err(err) => err.exit(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(args)
    }

    #[test]
    fn no_subcommand_with_path_resolves_to_present() {
        let cli = parse(&["oxlide", "talk.md"]).unwrap();
        assert_eq!(
            cli.resolve().unwrap(),
            ResolvedCommand::Present(PathBuf::from("talk.md")),
        );
    }

    #[test]
    fn present_subcommand_with_path_resolves_to_present() {
        let cli = parse(&["oxlide", "present", "talk.md"]).unwrap();
        assert_eq!(
            cli.resolve().unwrap(),
            ResolvedCommand::Present(PathBuf::from("talk.md")),
        );
    }

    #[test]
    fn no_args_produces_missing_subcommand_error() {
        let cli = parse(&["oxlide"]).unwrap();
        let err = cli.resolve().unwrap_err();
        assert_eq!(err.kind(), ErrorKind::MissingSubcommand);
    }

    #[test]
    fn positional_path_and_subcommand_is_rejected() {
        match parse(&["oxlide", "talk.md", "present", "other.md"]) {
            Ok(cli) => {
                let err = cli.resolve().unwrap_err();
                assert_eq!(err.kind(), ErrorKind::ArgumentConflict);
            }
            Err(err) => {
                assert_ne!(err.kind(), ErrorKind::DisplayHelp);
                assert_ne!(err.kind(), ErrorKind::DisplayVersion);
            }
        }
    }

    #[test]
    fn help_flag_emits_display_help_error() {
        let err = parse(&["oxlide", "--help"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::DisplayHelp);
    }

    #[test]
    fn version_flag_emits_display_version_error() {
        let err = parse(&["oxlide", "--version"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::DisplayVersion);
    }
}
