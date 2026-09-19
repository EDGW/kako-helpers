use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result};
use thiserror::Error;

use crate::components::{Component, ServiceDescriptor};
use crate::output::OutputStyle;

const SERVICES_DIRECTORY: &str = "Library/Services";
const LSREGISTER: &str = "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister";
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Error)]
enum ServiceError {
    #[error("HOME is not set")]
    HomeMissing,
    #[error("failed to inspect service path {}", path.display())]
    Inspect {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to create service directory {}", path.display())]
    CreateDirectory {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("invalid service metadata for component {component}")]
    InvalidMetadata { component: &'static str },
    #[error("service backup already exists: {}", path.display())]
    BackupExists { path: PathBuf },
    #[error("failed to create service bundle {}", path.display())]
    CreateBundle {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to write service metadata {}", path.display())]
    WriteMetadata {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to copy service host to {}", path.display())]
    CopyHost {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to synchronize service host {}", path.display())]
    SyncHost {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to replace service bundle {}", path.display())]
    ReplaceBundle {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error(
        "failed to commit service bundle {}: {commit}; rollback failed: {rollback}",
        path.display()
    )]
    CommitRollback {
        path: PathBuf,
        commit: io::Error,
        rollback: io::Error,
    },
    #[error("failed to remove previous service bundle {}", path.display())]
    RemoveBackup {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to register service {} with Launch Services", bundle.display())]
    Register {
        bundle: PathBuf,
        status: Option<i32>,
        #[source]
        source: io::Error,
    },
    #[error("Launch Services rejected service {} with exit status {status}", bundle.display())]
    RegisterStatus { bundle: PathBuf, status: i32 },
    #[error(
        "failed to register service {}: {registration}; rollback failed: {rollback}",
        path.display()
    )]
    RegistrationRollback {
        path: PathBuf,
        registration: String,
        rollback: io::Error,
    },
}

#[derive(Debug)]
pub struct ServicePlan {
    descriptor: &'static ServiceDescriptor,
    services: PathBuf,
}

pub fn preflight_component_services(
    components: &[Component],
    override_existing: bool,
    style: OutputStyle,
) -> Result<Vec<ServicePlan>> {
    let descriptors: Vec<_> = components
        .iter()
        .filter_map(|component| component.service_descriptor())
        .collect();
    if descriptors.is_empty() {
        return Ok(Vec::new());
    }

    for descriptor in &descriptors {
        validate_descriptor(descriptor)?;
    }

    let services = services_directory()?;
    if path_exists(&services)? {
        let metadata = fs::metadata(&services).map_err(|source| ServiceError::Inspect {
            path: services.clone(),
            source,
        })?;
        if !metadata.is_dir() {
            return Err(ServiceError::CreateDirectory {
                path: services,
                source: io::Error::new(io::ErrorKind::AlreadyExists, "path is not a directory"),
            }
            .into());
        }
    }

    let mut plans = Vec::new();
    for descriptor in descriptors {
        let destination = services.join(descriptor.bundle_name);
        if path_exists(&destination)? && !override_existing {
            style.service_skipped(descriptor.menu_title);
            continue;
        }
        plans.push(ServicePlan {
            descriptor,
            services: services.clone(),
        });
    }
    Ok(plans)
}

pub fn install_component_services(
    plans: &[ServicePlan],
    source: &Path,
    style: OutputStyle,
) -> Result<()> {
    if plans.is_empty() {
        return Ok(());
    }

    let services = services_directory()?;
    fs::create_dir_all(&services).map_err(|source| ServiceError::CreateDirectory {
        path: services.clone(),
        source,
    })?;

    for plan in plans {
        install_service(plan, source, style)?;
    }

    Ok(())
}

fn services_directory() -> Result<PathBuf> {
    let home = std::env::var_os("HOME").ok_or(ServiceError::HomeMissing)?;
    Ok(PathBuf::from(home).join(SERVICES_DIRECTORY))
}

fn install_service(plan: &ServicePlan, source: &Path, style: OutputStyle) -> Result<()> {
    let descriptor = plan.descriptor;
    let services = &plan.services;
    let destination = services.join(descriptor.bundle_name);
    let destination_exists = path_exists(&destination)?;

    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let staging = services.join(format!(
        ".{}.tmp.{}.{}",
        descriptor.bundle_name,
        std::process::id(),
        counter
    ));
    let backup = services.join(format!(
        ".{}.backup.{}.{}",
        descriptor.bundle_name,
        std::process::id(),
        counter
    ));

    if path_exists(&backup)? {
        return Err(ServiceError::BackupExists { path: backup }.into());
    }

    let result: Result<()> = (|| {
        create_bundle(&staging, descriptor, source)?;
        commit_and_register(&staging, &destination, &backup, destination_exists)?;
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    result
        .with_context(|| format!("failed to install Finder service {}", descriptor.menu_title))?;

    style.service_installed(descriptor.menu_title);
    Ok(())
}

fn validate_descriptor(descriptor: &ServiceDescriptor) -> Result<()> {
    let matches_component = descriptor
        .component
        .service_descriptor()
        .is_some_and(|registered| std::ptr::eq(registered, descriptor));
    if !matches_component
        || descriptor.bundle_name.is_empty()
        || descriptor.bundle_identifier.is_empty()
        || descriptor.host_executable_name.is_empty()
        || descriptor.message.is_empty()
        || descriptor.menu_title.is_empty()
    {
        return Err(ServiceError::InvalidMetadata {
            component: descriptor.component.command_name(),
        }
        .into());
    }
    Ok(())
}

fn create_bundle(bundle: &Path, descriptor: &ServiceDescriptor, source: &Path) -> Result<()> {
    let contents = bundle.join("Contents");
    let macos = contents.join("MacOS");
    fs::create_dir_all(&macos).map_err(|source| ServiceError::CreateBundle {
        path: bundle.to_owned(),
        source,
    })?;

    let info_path = contents.join("Info.plist");
    let mut metadata = File::create(&info_path).map_err(|source| ServiceError::WriteMetadata {
        path: info_path.clone(),
        source,
    })?;
    metadata
        .write_all(info_plist(descriptor).as_bytes())
        .map_err(|source| ServiceError::WriteMetadata {
            path: info_path.clone(),
            source,
        })?;
    metadata
        .sync_all()
        .map_err(|source| ServiceError::WriteMetadata {
            path: info_path.clone(),
            source,
        })?;

    let host = macos.join(descriptor.host_executable_name);
    fs::copy(source, &host).map_err(|source| ServiceError::CopyHost {
        path: host.clone(),
        source,
    })?;
    let source_path = source.to_owned();
    let permissions = fs::metadata(&source_path)
        .map_err(|source| ServiceError::CopyHost {
            path: source_path.clone(),
            source,
        })?
        .permissions();
    fs::set_permissions(&host, permissions).map_err(|source| ServiceError::CopyHost {
        path: host.clone(),
        source,
    })?;
    File::open(&host)
        .and_then(|file| file.sync_all())
        .map_err(|source| ServiceError::SyncHost { path: host, source })?;

    Ok(())
}

fn commit_and_register(
    staging: &Path,
    destination: &Path,
    backup: &Path,
    destination_exists: bool,
) -> Result<()> {
    if destination_exists {
        fs::rename(destination, backup).map_err(|source| ServiceError::ReplaceBundle {
            path: destination.to_owned(),
            source,
        })?;
    }

    if let Err(source) = rename_no_replace(staging, destination) {
        return rollback_commit(
            destination,
            backup,
            destination_exists,
            ServiceError::ReplaceBundle {
                path: destination.to_owned(),
                source,
            },
        );
    }

    if let Err(registration) = register_service(destination) {
        return rollback_registration(destination, backup, destination_exists, registration);
    }

    if destination_exists {
        fs::remove_dir_all(backup).map_err(|source| ServiceError::RemoveBackup {
            path: backup.to_owned(),
            source,
        })?;
    }

    Ok(())
}

fn rollback_commit(
    destination: &Path,
    backup: &Path,
    destination_exists: bool,
    commit_error: ServiceError,
) -> Result<()> {
    if !destination_exists {
        return Err(commit_error.into());
    }

    match fs::rename(backup, destination) {
        Ok(()) => Err(commit_error.into()),
        Err(rollback) => {
            let commit = match commit_error {
                ServiceError::ReplaceBundle { source, .. } => source,
                _ => io::Error::other("service commit failed"),
            };
            Err(ServiceError::CommitRollback {
                path: destination.to_owned(),
                commit,
                rollback,
            }
            .into())
        }
    }
}

fn rollback_registration(
    destination: &Path,
    backup: &Path,
    destination_exists: bool,
    registration: ServiceError,
) -> Result<()> {
    let remove_new = fs::remove_dir_all(destination);
    let restore_backup = if destination_exists {
        fs::rename(backup, destination)
    } else {
        Ok(())
    };

    match (remove_new, restore_backup) {
        (Ok(()), Ok(())) => Err(registration.into()),
        (remove_new, restore_backup) => {
            let rollback = remove_new
                .err()
                .or_else(|| restore_backup.err())
                .unwrap_or_else(|| io::Error::other("service registration rollback failed"));
            Err(ServiceError::RegistrationRollback {
                path: destination.to_owned(),
                registration: registration.to_string(),
                rollback,
            }
            .into())
        }
    }
}

fn path_exists(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(ServiceError::Inspect {
            path: path.to_owned(),
            source,
        }
        .into()),
    }
}

#[cfg(target_os = "macos")]
fn rename_no_replace(from: &Path, to: &Path) -> io::Result<()> {
    use rustix::fs::{CWD, RenameFlags, renameat_with};

    renameat_with(CWD, from, CWD, to, RenameFlags::NOREPLACE).map_err(io::Error::from)
}

#[cfg(not(target_os = "macos"))]
fn rename_no_replace(from: &Path, to: &Path) -> io::Result<()> {
    fs::rename(from, to)
}

fn register_service(bundle: &Path) -> std::result::Result<(), ServiceError> {
    let status = Command::new(LSREGISTER)
        .arg("-f")
        .arg(bundle)
        .status()
        .map_err(|source| ServiceError::Register {
            bundle: bundle.to_owned(),
            status: None,
            source,
        })?;
    if !status.success() {
        return Err(ServiceError::RegisterStatus {
            bundle: bundle.to_owned(),
            status: status.code().unwrap_or(-1),
        });
    }
    Ok(())
}

fn info_plist(descriptor: &ServiceDescriptor) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleExecutable</key>
  <string>{host}</string>
  <key>CFBundleIdentifier</key>
  <string>{identifier}</string>
  <key>CFBundleName</key>
  <string>Kako Helpers Localize</string>
  <key>LSUIElement</key>
  <true/>
  <key>NSPrincipalClass</key>
  <string>NSApplication</string>
  <key>NSServices</key>
  <array>
    <dict>
      <key>NSMessage</key>
      <string>{message}</string>
      <key>NSMenuItem</key>
      <dict>
        <key>default</key>
        <string>{menu}</string>
      </dict>
      <key>NSPortName</key>
      <string>{host}</string>
      <key>NSSendFileTypes</key>
      <array>
        <string>public.folder</string>
      </array>
      <key>NSRequiredContext</key>
      <dict>
        <key>NSApplicationIdentifier</key>
        <string>com.apple.finder</string>
      </dict>
    </dict>
  </array>
</dict>
</plist>
"#,
        host = descriptor.host_executable_name,
        identifier = descriptor.bundle_identifier,
        message = descriptor.message,
        menu = descriptor.menu_title,
    )
}
