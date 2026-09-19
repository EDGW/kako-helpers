use std::env;
use std::ffi::OsString;
use std::path::Path;

use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};

use crate::commands;
use crate::output::OutputStyle;

#[derive(Debug, Parser)]
#[command(name = "kako-helpers", version, about = "macOS helper commands")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Manage localized directory display names.
    Localize(commands::localizer::LocalizeArgs),
    /// Install the helper and component symlinks into a folder.
    Install(commands::install::InstallArgs),
}

pub fn run(style: OutputStyle) -> Result<()> {
    let args: Vec<OsString> = env::args_os().collect();
    if let Some(command) = implied_subcommand(&args) {
        return match command {
            "localize" => {
                let args =
                    commands::localizer::try_parse_from(args).unwrap_or_else(|error| error.exit());
                commands::localizer::run(args, style)
            }
            "install" => {
                let args =
                    commands::install::try_parse_from(args).unwrap_or_else(|error| error.exit());
                commands::install::run(args, style)
            }
            _ => unreachable!("implied command must be in the top-level command tree"),
        };
    }

    dispatch(Cli::parse_from(args), style)
}

pub fn dispatch(cli: Cli, style: OutputStyle) -> Result<()> {
    match cli.command {
        Commands::Localize(args) => commands::localizer::run(args, style),
        Commands::Install(args) => commands::install::run(args, style),
    }
}

fn implied_subcommand(args: &[OsString]) -> Option<&str> {
    let program_name = args
        .first()
        .and_then(|arg| Path::new(arg).file_name())?
        .to_str()?;
    (program_name != "kako-helpers" && is_top_level_command(program_name)).then_some(program_name)
}

fn is_top_level_command(name: &str) -> bool {
    Cli::command()
        .get_subcommands()
        .any(|command| command.get_name() == name)
}
