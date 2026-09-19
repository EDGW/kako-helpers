use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Args, FromArgMatches, Subcommand};

use crate::output::OutputStyle;
use crate::tools::localizer;

#[derive(Debug, Args)]
pub struct LocalizeArgs {
    #[command(subcommand)]
    command: LocalizeCommand,
}

#[derive(Debug, Subcommand)]
enum LocalizeCommand {
    /// Create or update a localized directory display name.
    Set(SetArgs),
    /// Remove one or all localized directory display names.
    Remove(RemoveArgs),
    /// List stored locales and their display names.
    List(ListArgs),
}

#[derive(Debug, Args)]
pub struct SetArgs {
    /// Directory whose Finder display name will be localized.
    path: PathBuf,

    /// Localized display name.
    name: String,

    /// Language identifier for the generated .strings file.
    #[arg(long, value_name = "xxx")]
    lang: Option<String>,

    /// Suppress success output.
    #[arg(short, long)]
    silent: bool,

    /// Require the path to match exactly; do not try path.localized.
    #[arg(short = 'S', long)]
    strict: bool,
}

#[derive(Debug, Args)]
pub struct RemoveArgs {
    /// Localized directory to modify.
    path: PathBuf,

    /// Remove only this language; omit to remove all localizations.
    #[arg(long, value_name = "xxx")]
    lang: Option<String>,

    /// Suppress success output.
    #[arg(short, long)]
    silent: bool,

    /// Require the path to match exactly; do not try path.localized.
    #[arg(short = 'S', long)]
    strict: bool,
}

#[derive(Debug, Args)]
pub struct ListArgs {
    /// Localized directory to inspect.
    path: PathBuf,

    /// Require the path to match exactly; do not try path.localized.
    #[arg(short = 'S', long)]
    strict: bool,
}

pub fn try_parse_from<I, T>(args: I) -> std::result::Result<LocalizeArgs, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let command = LocalizeArgs::augment_args(clap::Command::new("localize"));
    let matches = command.try_get_matches_from(args)?;
    LocalizeArgs::from_arg_matches(&matches)
}

pub fn run(args: LocalizeArgs, style: OutputStyle) -> Result<()> {
    match args.command {
        LocalizeCommand::Set(args) => run_set(args, style),
        LocalizeCommand::Remove(args) => run_remove(args, style),
        LocalizeCommand::List(args) => run_list(args, style),
    }
}

fn run_set(args: SetArgs, style: OutputStyle) -> Result<()> {
    let path = resolve_path(args.path, args.strict);
    let localized = localizer::localize(&path, args.lang.as_deref(), &args.name)
        .with_context(|| format!("localize set failed for {}", path.display()))?;

    style.set_localized_name(args.silent, &localized, &args.name);
    Ok(())
}

fn run_remove(args: RemoveArgs, style: OutputStyle) -> Result<()> {
    let path = resolve_path(args.path, args.strict);
    let report = localizer::remove_localization(&path, args.lang.as_deref())
        .with_context(|| format!("localize remove failed for {}", path.display()))?;

    match args.lang {
        Some(language) => style.removed_language(args.silent, &language, &report.path),
        None => style.removed_all(args.silent, &report.path),
    }
    Ok(())
}

fn run_list(args: ListArgs, style: OutputStyle) -> Result<()> {
    let path = resolve_path(args.path, args.strict);
    let entries = localizer::list_localizations(&path)
        .with_context(|| format!("localize list failed for {}", path.display()))?;

    if entries.is_empty() {
        style.info(format!("No localized names found for {}.", path.display()));
        return Ok(());
    }

    for entry in entries {
        style.list_entry(&entry.language, &entry.display_name);
    }
    Ok(())
}

fn resolve_path(path: PathBuf, strict: bool) -> PathBuf {
    let path: PathBuf = path.components().collect();
    if strict || has_localized_suffix(&path) {
        return path;
    }

    match fs::symlink_metadata(&path) {
        Ok(_) => path,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let mut appended = path.as_os_str().to_os_string();
            appended.push(".localized");
            let localized = PathBuf::from(appended);
            if fs::symlink_metadata(&localized).is_ok() {
                localized
            } else {
                path
            }
        }
        Err(_) => path,
    }
}

fn has_localized_suffix(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".localized"))
}
