use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::Result;
use clap::{Args, FromArgMatches};
use thiserror::Error;

use crate::components::{Component, ComponentKind, LinkStyle};
use crate::output::OutputStyle;
use crate::service;

const INSTALLED_BINARY_NAME: &str = "kako-helpers";
static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Args)]
pub struct InstallArgs {
    /// Existing directory that will receive the helper.
    folder: PathBuf,

    /// Component symlinks to create; omit to install all components.
    #[arg(
        short = 'C',
        long,
        value_name = "COMPONENT",
        value_delimiter = ',',
        num_args = 1..
    )]
    components: Vec<ComponentKind>,

    /// Replace an existing installed binary or component symlink.
    #[arg(short = 'O', long = "override")]
    override_existing: bool,

    /// Naming style for component symlinks.
    #[arg(long, value_enum, default_value_t = LinkStyle::Default)]
    style: LinkStyle,

    /// Do not install or register Finder services.
    #[arg(long)]
    no_service: bool,
}

#[derive(Debug, Error)]
enum InstallError {
    #[error("installation folder does not exist: {}", path.display())]
    FolderMissing { path: PathBuf },
    #[error("installation folder is not a directory: {}", path.display())]
    FolderNotDirectory { path: PathBuf },
    #[error("refusing to install into symbolic link: {}", path.display())]
    FolderSymlink { path: PathBuf },
    #[error("failed to inspect installation path {}", path.display())]
    Inspect {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("installed binary already exists: {}", path.display())]
    BinaryExists { path: PathBuf },
    #[error("cannot replace directory with installed binary: {}", path.display())]
    BinaryIsDirectory { path: PathBuf },
    #[error("cannot override directory with symlink: {}", path.display())]
    SymlinkIsDirectory { path: PathBuf },
    #[error("source and destination are the same file: {}", path.display())]
    SourceDestinationSame { path: PathBuf },
    #[error("failed to resolve the running executable")]
    CurrentExecutable {
        #[source]
        source: io::Error,
    },
    #[error("failed to copy executable {}", path.display())]
    CopyExecutable {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to synchronize executable {}", path.display())]
    SyncExecutable {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to commit installed executable {}", path.display())]
    CommitExecutable {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to remove existing path {}", path.display())]
    RemoveExisting {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to create symlink {}", path.display())]
    CreateSymlink {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

pub fn try_parse_from<I, T>(args: I) -> std::result::Result<InstallArgs, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let command = InstallArgs::augment_args(clap::Command::new("install"));
    let matches = command.try_get_matches_from(args)?;
    InstallArgs::from_arg_matches(&matches)
}

pub fn run(args: InstallArgs, style: OutputStyle) -> Result<()> {
    let folder = validate_folder(&args.folder)?;
    let source =
        std::env::current_exe().map_err(|source| InstallError::CurrentExecutable { source })?;
    let destination = folder.join(INSTALLED_BINARY_NAME);

    let components = selected_components(&args.components);
    let service_plans = if args.no_service {
        Vec::new()
    } else {
        service::preflight_component_services(&components, args.override_existing, style)?
    };
    preflight_binary(&source, &destination, args.override_existing)?;
    preflight_symlinks(&folder, &components, args.style, args.override_existing)?;

    install_binary(&source, &destination, args.override_existing)?;
    style.installed(&destination);

    for component in &components {
        let name = component.link_name(args.style);
        let path = folder.join(&name);
        if fs::symlink_metadata(&path).is_ok() {
            if args.override_existing {
                fs::remove_file(&path).map_err(|source| InstallError::RemoveExisting {
                    path: path.clone(),
                    source,
                })?;
            } else {
                style.skipped(&name);
                continue;
            }
        }

        create_symlink(INSTALLED_BINARY_NAME, &path)?;
        style.created_symlink(&name, INSTALLED_BINARY_NAME);
    }

    if !args.no_service {
        service::install_component_services(&service_plans, &source, style)?;
    }

    Ok(())
}

fn validate_folder(folder: &Path) -> Result<PathBuf> {
    let folder: PathBuf = folder.components().collect();
    match fs::symlink_metadata(&folder) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(InstallError::FolderSymlink { path: folder }.into())
        }
        Ok(metadata) if metadata.is_dir() => Ok(folder),
        Ok(_) => Err(InstallError::FolderNotDirectory { path: folder }.into()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            Err(InstallError::FolderMissing { path: folder }.into())
        }
        Err(source) => Err(InstallError::Inspect {
            path: folder,
            source,
        }
        .into()),
    }
}

fn selected_components(requested: &[ComponentKind]) -> Vec<&'static dyn Component> {
    if requested.is_empty() {
        return ComponentKind::DEFAULT
            .iter()
            .map(|component| component.instance())
            .collect();
    }

    let mut selected = Vec::new();
    for component in requested {
        if !selected.contains(component) {
            selected.push(*component);
        }
    }
    selected.into_iter().map(ComponentKind::instance).collect()
}

fn preflight_binary(source: &Path, destination: &Path, override_existing: bool) -> Result<()> {
    match fs::symlink_metadata(destination) {
        Ok(metadata) => {
            if source == destination {
                return Err(InstallError::SourceDestinationSame {
                    path: destination.to_owned(),
                }
                .into());
            }
            if !override_existing {
                return Err(InstallError::BinaryExists {
                    path: destination.to_owned(),
                }
                .into());
            }
            if metadata.is_dir() {
                return Err(InstallError::BinaryIsDirectory {
                    path: destination.to_owned(),
                }
                .into());
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(InstallError::Inspect {
                path: destination.to_owned(),
                source,
            }
            .into());
        }
    }
    Ok(())
}

fn preflight_symlinks(
    folder: &Path,
    components: &[&'static dyn Component],
    style: LinkStyle,
    override_existing: bool,
) -> Result<()> {
    if !override_existing {
        return Ok(());
    }

    for component in components {
        let path = folder.join(component.link_name(style));
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
                return Err(InstallError::SymlinkIsDirectory { path }.into());
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(InstallError::Inspect { path, source }.into());
            }
        }
    }
    Ok(())
}

fn install_binary(source: &Path, destination: &Path, override_existing: bool) -> Result<()> {
    let source_path = source.to_owned();
    let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let temporary = destination.with_file_name(format!(
        ".{INSTALLED_BINARY_NAME}.tmp.{}.{}",
        std::process::id(),
        counter
    ));
    let result = (|| {
        fs::copy(source, &temporary).map_err(|source| InstallError::CopyExecutable {
            path: temporary.clone(),
            source,
        })?;
        let permissions = fs::metadata(&source_path)
            .map_err(|source| InstallError::CopyExecutable {
                path: source_path.clone(),
                source,
            })?
            .permissions();
        fs::set_permissions(&temporary, permissions).map_err(|source| {
            InstallError::CopyExecutable {
                path: temporary.clone(),
                source,
            }
        })?;
        File::open(&temporary)
            .and_then(|file| file.sync_all())
            .map_err(|source| InstallError::SyncExecutable {
                path: temporary.clone(),
                source,
            })?;

        if override_existing {
            fs::rename(&temporary, destination)
        } else {
            fs::hard_link(&temporary, destination).and_then(|()| fs::remove_file(&temporary))
        }
        .map_err(|source| InstallError::CommitExecutable {
            path: destination.to_owned(),
            source,
        })?;
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(unix)]
fn create_symlink(target: &str, path: &Path) -> Result<()> {
    std::os::unix::fs::symlink(target, path).map_err(|source| InstallError::CreateSymlink {
        path: path.to_owned(),
        source,
    })?;
    Ok(())
}

#[cfg(not(unix))]
fn create_symlink(target: &str, path: &Path) -> Result<()> {
    let _ = target;
    Err(InstallError::CreateSymlink {
        path: path.to_owned(),
        source: io::Error::new(io::ErrorKind::Unsupported, "symlinks are unsupported"),
    }
    .into())
}
