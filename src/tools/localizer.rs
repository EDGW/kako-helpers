use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result};
use thiserror::Error;

use crate::components::{Component, ServiceActionDescriptor, ServiceDescriptor};

const LOCALIZED_SUFFIX: &str = ".localized";
static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(crate) const LOCALIZE_SERVICE: ServiceDescriptor = ServiceDescriptor {
    component: &LOCALIZER,
    bundle_name: "KakoHelpersLocalize.service",
    bundle_identifier: "com.kako.helpers.localize-service",
    host_executable_name: "KakoHelpersLocalizeService",
    actions: &LOCALIZE_ACTIONS,
};

const LOCALIZE_ACTIONS: [ServiceActionDescriptor; 4] = [
    ServiceActionDescriptor {
        message: "localizeFolder",
        menu_title: "Localizer: Localize Folder",
    },
    ServiceActionDescriptor {
        message: "localizeFolderWithSymlink",
        menu_title: "Localizer: Localize Folder with Symlink",
    },
    ServiceActionDescriptor {
        message: "removeLocalizedNames",
        menu_title: "Localizer: Remove Localized Names",
    },
    ServiceActionDescriptor {
        message: "removeLocalizedNamesWithoutRename",
        menu_title: "Localizer: Remove Localized Names without Rename",
    },
];

#[derive(Debug)]
pub struct Localizer;

pub static LOCALIZER: Localizer = Localizer;

impl Component for Localizer {
    fn command_name(&self) -> &'static str {
        "localize"
    }

    fn service_descriptor(&self) -> Option<&'static ServiceDescriptor> {
        Some(&LOCALIZE_SERVICE)
    }
}

/// Top-level failure returned by the localized-name API.
///
/// Each variant groups failures by operation so callers can match on a stable
/// stage while retaining the underlying source error.
#[derive(Debug, Error)]
pub enum LocalizeError {
    #[error("localized directory names are only supported on macOS")]
    UnsupportedPlatform,
    #[error(transparent)]
    Language(#[from] LanguageError),
    #[error(transparent)]
    DisplayName(#[from] DisplayNameError),
    #[error(transparent)]
    Target(#[from] TargetError),
    #[error(transparent)]
    Metadata(#[from] MetadataError),
    #[error(transparent)]
    Commit(#[from] CommitError),
    #[error(transparent)]
    Remove(#[from] RemoveError),
    #[error(transparent)]
    List(#[from] ListError),
}

/// Failures while resolving or validating a language identifier.
#[derive(Debug, Error)]
pub enum LanguageError {
    #[error("macOS did not return any preferred languages")]
    NoPreferredLanguage,
    #[error("target language {language:?} is invalid: {reason}")]
    InvalidTarget {
        language: String,
        reason: InvalidLanguageReason,
    },
    #[error("current language {language:?} is invalid: {reason}")]
    InvalidCurrent {
        language: String,
        reason: InvalidLanguageReason,
    },
}

/// Specific reasons a language identifier cannot become a `.strings` file name.
#[derive(Debug, Error)]
pub enum InvalidLanguageReason {
    #[error("language cannot be empty")]
    Empty,
    #[error("language contains an empty component")]
    EmptyComponent,
    #[error("unsupported character {0:?}")]
    UnsupportedCharacter(char),
    #[error("language is too long: {length} bytes; maximum is {max}")]
    TooLong { length: usize, max: usize },
    #[error("language has too many components: {count}; maximum is {max}")]
    TooManyComponents { count: usize, max: usize },
}

/// Invalid localized display names rejected before touching the file system.
#[derive(Debug, Error)]
pub enum DisplayNameError {
    #[error("localized display name cannot be empty")]
    Empty,
    #[error("localized display name cannot contain NUL")]
    ContainsNul,
}

/// Failures while validating the source and destination directories.
#[derive(Debug, Error)]
pub enum TargetError {
    #[error("failed to inspect target {}", path.display())]
    Inspect {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to inspect localized destination {}", path.display())]
    InspectDestination {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("localized names can only be applied to directories: {}", path.display())]
    NotDirectory { path: PathBuf },
    #[error("refusing to localize symbolic link: {}", path.display())]
    SymbolicLink { path: PathBuf },
    #[error("localized destination already exists: {}", path.display())]
    DestinationExists { path: PathBuf },
    #[error("target path has no file name: {}", path.display())]
    MissingFileName { path: PathBuf },
    #[error("target directory name is not valid UTF-8: {}", path.display())]
    NonUtf8FileName { path: PathBuf },
    #[error("original-name path conflicts with localization symlink: {}", path.display())]
    OriginalSymlinkConflict { path: PathBuf },
    #[error("failed to create original-name symlink {}", path.display())]
    CreateOriginalSymlink {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to remove original-name symlink {}", path.display())]
    RemoveOriginalSymlink {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

/// Failures while creating and atomically replacing `.localized` metadata.
#[derive(Debug, Error)]
pub enum MetadataError {
    #[error("failed to inspect localization metadata path {}", path.display())]
    Inspect {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("localization metadata path is not a directory: {}", path.display())]
    NotDirectory { path: PathBuf },
    #[error("refusing to use symbolic link as localization metadata: {}", path.display())]
    SymbolicLink { path: PathBuf },
    #[error("failed to create localization metadata directory {}", path.display())]
    CreateDirectory {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to create temporary strings file {}", path.display())]
    CreateTemporaryFile {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to write temporary strings file {}", path.display())]
    WriteTemporaryFile {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to synchronize temporary strings file {}", path.display())]
    SyncTemporaryFile {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to atomically replace strings file {}", path.display())]
    ReplaceFile {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

/// Failures while committing the directory rename.
#[derive(Debug, Error)]
pub enum CommitError {
    #[error("localized destination appeared during commit: {}", path.display())]
    DestinationExists { path: PathBuf },
    #[error("failed to rename {} to {}", from.display(), to.display())]
    RenameDirectory {
        from: PathBuf,
        to: PathBuf,
        #[source]
        source: io::Error,
    },
}

/// Failures while removing localized names.
#[derive(Debug, Error)]
pub enum RemoveError {
    #[error("failed to read localization metadata directory {}", path.display())]
    ReadDirectory {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("directory is not localized: {}", path.display())]
    NotLocalized { path: PathBuf },
    #[error("language {language:?} is not present in {}", path.display())]
    LanguageNotFound { language: String, path: PathBuf },
    #[error("unmanaged entry in localization metadata: {}", path.display())]
    UnmanagedEntry { path: PathBuf },
    #[error("failed to remove localization file {}", path.display())]
    RemoveFile {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to remove localization metadata directory {}", path.display())]
    RemoveDirectory {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("original-name path conflicts with localization removal: {}", path.display())]
    OriginalPathConflict { path: PathBuf },
    #[error("failed to remove original-name symlink {}", path.display())]
    RemoveOriginalSymlink {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error(transparent)]
    Commit(#[from] CommitError),
}

/// Failures while listing localized names.
#[derive(Debug, Error)]
pub enum ListError {
    #[error("failed to read localization metadata directory {}", path.display())]
    ReadDirectory {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to read localization file {}", path.display())]
    ReadFile {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("invalid localization file {}: {reason}", path.display())]
    InvalidStrings {
        path: PathBuf,
        reason: StringsFormatError,
    },
}

/// Specific syntax errors in a generated `.strings` file.
#[derive(Debug, Error)]
pub enum StringsFormatError {
    #[error("expected an opening quote at byte {offset}")]
    ExpectedOpeningQuote { offset: usize },
    #[error("unterminated quoted string starting at byte {offset}")]
    UnterminatedString { offset: usize },
    #[error("invalid escape sequence {escape:?} at byte {offset}")]
    InvalidEscape { escape: char, offset: usize },
    #[error("expected '=' at byte {offset}")]
    ExpectedEquals { offset: usize },
    #[error("expected ';' at byte {offset}")]
    ExpectedSemicolon { offset: usize },
    #[error("unexpected trailing content at byte {offset}")]
    TrailingContent { offset: usize },
}

/// A language/display-name pair stored in `.localized`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalizationEntry {
    pub language: String,
    pub display_name: String,
}

/// Result of removing one or more localized names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveReport {
    pub path: PathBuf,
    pub removed_languages: Vec<String>,
}

/// Options for creating a localized directory name.
#[derive(Debug, Clone, Copy, Default)]
pub struct LocalizeOptions {
    pub create_symlink: bool,
}

/// Options for removing localized directory names.
#[derive(Debug, Clone, Copy, Default)]
pub struct RemoveOptions {
    pub no_rename: bool,
}

/// Returns the first preferred macOS language normalized for `.strings` files.
///
/// The value comes from `NSLocale.preferredLanguages`, not the process `LANG`
/// environment variable. The returned identifier is safe to use as a file stem.
#[cfg(target_os = "macos")]
pub fn current_lang() -> Result<Box<str>> {
    use objc2_foundation::NSLocale;

    let preferred = NSLocale::preferredLanguages();
    let language = preferred
        .firstObject()
        .ok_or(LocalizeError::Language(LanguageError::NoPreferredLanguage))?;
    let language =
        normalize_language(&language.to_string(), true).map_err(LocalizeError::Language)?;

    Ok(language.into_boxed_str())
}

/// Returns an unsupported-platform error on non-macOS targets.
#[cfg(not(target_os = "macos"))]
pub fn current_lang() -> Result<Box<str>> {
    Err(LocalizeError::UnsupportedPlatform.into())
}

/// Creates or updates the localized display name of a directory.
///
/// If `dir` is not already named `Name.localized`, it is renamed to that form
/// after metadata has been prepared. The returned path is the final on-disk
/// directory path.
///
/// # Side effects
///
/// Metadata is created or updated before the final rename. If that rename
/// fails, the original directory is preserved, but the hidden `.localized`
/// metadata change can remain in place.
///
/// # Errors
///
/// Returns an [`anyhow::Error`] whose chain contains a [`LocalizeError`] grouped
/// by language, display-name, target, metadata, or commit failure.
pub fn localize(dir: &Path, target_lang: Option<&str>, new_name: &str) -> Result<PathBuf> {
    localize_with_options(dir, target_lang, new_name, LocalizeOptions::default())
}

/// Creates or updates a localized directory name with explicit options.
pub fn localize_with_options(
    dir: &Path,
    target_lang: Option<&str>,
    new_name: &str,
    options: LocalizeOptions,
) -> Result<PathBuf> {
    localize_impl(dir, target_lang, new_name, options)
        .with_context(|| format!("failed to localize directory {}", dir.display()))
}

fn localize_impl(
    dir: &Path,
    target_lang: Option<&str>,
    new_name: &str,
    options: LocalizeOptions,
) -> Result<PathBuf> {
    let normalized_dir = normalized_directory(dir)?;
    let dir = normalized_dir.as_path();
    validate_display_name(new_name).map_err(LocalizeError::DisplayName)?;
    let language = match target_lang {
        Some(language) => normalize_language(language, false).map_err(LocalizeError::Language)?,
        None => {
            let language = current_lang()?;
            normalize_language(&language, true).map_err(LocalizeError::Language)?
        }
    };
    let dir_name = dir
        .file_name()
        .ok_or_else(|| {
            LocalizeError::Target(TargetError::MissingFileName {
                path: dir.to_owned(),
            })
        })?
        .to_str()
        .ok_or_else(|| {
            LocalizeError::Target(TargetError::NonUtf8FileName {
                path: dir.to_owned(),
            })
        })?;
    let source_name = dir_name.strip_suffix(LOCALIZED_SUFFIX).unwrap_or(dir_name);
    let destination = if dir_name.ends_with(LOCALIZED_SUFFIX) {
        dir.to_owned()
    } else {
        dir.with_file_name(format!("{dir_name}{LOCALIZED_SUFFIX}"))
    };
    let original_path = dir.with_file_name(source_name);
    if destination != dir {
        match fs::symlink_metadata(&destination) {
            Ok(_) => {
                return Err(LocalizeError::Target(TargetError::DestinationExists {
                    path: destination,
                })
                .into());
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(LocalizeError::Target(TargetError::InspectDestination {
                    path: destination,
                    source: error,
                })
                .into());
            }
        }
    }

    let metadata_dir = dir.join(".localized");
    ensure_metadata_directory(&metadata_dir)?;
    write_strings_file(&metadata_dir, &language, source_name, new_name)?;

    if destination != dir {
        rename_directory_noreplace(dir, &destination)?;
    }

    if options.create_symlink
        && let Err(error) = ensure_original_symlink(&original_path, &destination)
    {
        if destination != dir {
            let _ = rename_directory_noreplace(&destination, dir);
        }
        return Err(error.into());
    }

    Ok(destination)
}

fn ensure_original_symlink(
    original: &Path,
    destination: &Path,
) -> std::result::Result<(), LocalizeError> {
    match fs::symlink_metadata(original) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            if paths_resolve_equal(original, destination) {
                return Ok(());
            }
            return Err(LocalizeError::Target(
                TargetError::OriginalSymlinkConflict {
                    path: original.to_owned(),
                },
            ));
        }
        Ok(_) => {
            return Err(LocalizeError::Target(
                TargetError::OriginalSymlinkConflict {
                    path: original.to_owned(),
                },
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(LocalizeError::Target(TargetError::Inspect {
                path: original.to_owned(),
                source,
            }));
        }
    }

    let target = destination.file_name().ok_or_else(|| {
        LocalizeError::Target(TargetError::MissingFileName {
            path: destination.to_owned(),
        })
    })?;
    create_relative_symlink(target, original).map_err(|source| {
        LocalizeError::Target(TargetError::CreateOriginalSymlink {
            path: original.to_owned(),
            source,
        })
    })
}

fn paths_resolve_equal(left: &Path, right: &Path) -> bool {
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

fn validate_original_path_for_remove(
    original: &Path,
    localized: &Path,
) -> std::result::Result<Option<PathBuf>, LocalizeError> {
    match fs::symlink_metadata(original) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            if paths_resolve_equal(original, localized) {
                Ok(Some(original.to_owned()))
            } else {
                Err(LocalizeError::Remove(RemoveError::OriginalPathConflict {
                    path: original.to_owned(),
                }))
            }
        }
        Ok(_) => Err(LocalizeError::Remove(RemoveError::OriginalPathConflict {
            path: original.to_owned(),
        })),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(LocalizeError::Target(TargetError::Inspect {
            path: original.to_owned(),
            source,
        })),
    }
}

fn resolve_localized_symlink(path: &Path) -> std::result::Result<PathBuf, LocalizeError> {
    let normalized: PathBuf = path.components().collect();
    match fs::symlink_metadata(&normalized) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            fs::canonicalize(&normalized).map_err(|source| {
                LocalizeError::Target(TargetError::Inspect {
                    path: normalized,
                    source,
                })
            })
        }
        Ok(_) => Ok(normalized),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(normalized),
        Err(source) => Err(LocalizeError::Target(TargetError::Inspect {
            path: normalized,
            source,
        })),
    }
}

#[cfg(unix)]
fn create_relative_symlink(target: &std::ffi::OsStr, path: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(Path::new(target), path)
}

#[cfg(not(unix))]
fn create_relative_symlink(_target: &std::ffi::OsStr, _path: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "symlinks are unsupported",
    ))
}

/// Removes one or all localized names from a `.localized` directory.
///
/// When the last localization is removed, the directory is restored to its
/// unlocalized name using a no-replace rename.
pub fn remove_localization(dir: &Path, target_lang: Option<&str>) -> Result<RemoveReport> {
    remove_localization_with_options(dir, target_lang, RemoveOptions::default())
}

/// Removes one or all localized names with explicit options.
pub fn remove_localization_with_options(
    dir: &Path,
    target_lang: Option<&str>,
    options: RemoveOptions,
) -> Result<RemoveReport> {
    remove_localization_impl(dir, target_lang, options)
        .with_context(|| format!("failed to remove localized names from {}", dir.display()))
}

fn remove_localization_impl(
    dir: &Path,
    target_lang: Option<&str>,
    options: RemoveOptions,
) -> Result<RemoveReport> {
    let resolved = resolve_localized_symlink(dir)?;
    let normalized = normalized_directory(&resolved)?;
    let dir_name = normalized
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            LocalizeError::Target(TargetError::NonUtf8FileName {
                path: normalized.clone(),
            })
        })?;
    let source_name = dir_name
        .strip_suffix(LOCALIZED_SUFFIX)
        .filter(|name| !name.is_empty())
        .ok_or_else(|| {
            LocalizeError::Remove(RemoveError::NotLocalized {
                path: normalized.clone(),
            })
        })?;
    let metadata_dir = normalized.join(".localized");
    let metadata_exists = metadata_directory_exists(&metadata_dir)?;

    let mut managed = Vec::new();
    if metadata_exists {
        for entry in fs::read_dir(&metadata_dir).map_err(|source| {
            LocalizeError::Remove(RemoveError::ReadDirectory {
                path: metadata_dir.clone(),
                source,
            })
        })? {
            let entry = entry.map_err(|source| {
                LocalizeError::Remove(RemoveError::ReadDirectory {
                    path: metadata_dir.clone(),
                    source,
                })
            })?;
            let path = entry.path();
            let file_type = entry.file_type().map_err(|source| {
                LocalizeError::Remove(RemoveError::ReadDirectory {
                    path: metadata_dir.clone(),
                    source,
                })
            })?;
            if !file_type.is_file()
                || path.extension().and_then(|value| value.to_str()) != Some("strings")
            {
                return Err(LocalizeError::Remove(RemoveError::UnmanagedEntry { path }).into());
            }
            let language = path
                .file_stem()
                .and_then(|value| value.to_str())
                .ok_or_else(|| {
                    LocalizeError::Remove(RemoveError::UnmanagedEntry { path: path.clone() })
                })?
                .to_owned();
            managed.push((language, path));
        }
    }

    let requested = target_lang
        .map(|language| normalize_language(language, false))
        .transpose()
        .map_err(LocalizeError::Language)?;
    let mut removed_languages = Vec::new();
    let mut files_to_remove = Vec::new();
    for (language, path) in &managed {
        if requested
            .as_ref()
            .is_none_or(|requested| requested == language)
        {
            removed_languages.push(language.clone());
            files_to_remove.push(path.clone());
        }
    }
    if requested.is_some() && removed_languages.is_empty() {
        return Err(LocalizeError::Remove(RemoveError::LanguageNotFound {
            language: requested.unwrap_or_default(),
            path: normalized,
        })
        .into());
    }

    let original = normalized.with_file_name(source_name);
    let original_symlink = validate_original_path_for_remove(&original, &normalized)?;
    let remove_all = files_to_remove.len() == managed.len();
    let restore_to = if remove_all && !options.no_rename {
        Some(original.clone())
    } else {
        None
    };

    for path in &files_to_remove {
        fs::remove_file(path).map_err(|source| {
            LocalizeError::Remove(RemoveError::RemoveFile {
                path: path.clone(),
                source,
            })
        })?;
    }

    let final_path = if let Some(restored) = restore_to {
        if original_symlink.is_some() {
            fs::remove_file(&original).map_err(|source| {
                LocalizeError::Remove(RemoveError::RemoveOriginalSymlink {
                    path: original.clone(),
                    source,
                })
            })?;
        }
        if metadata_exists {
            fs::remove_dir(&metadata_dir).map_err(|source| {
                LocalizeError::Remove(RemoveError::RemoveDirectory {
                    path: metadata_dir.clone(),
                    source,
                })
            })?;
        }
        rename_directory_noreplace(&normalized, &restored)
            .map_err(|error| LocalizeError::Remove(RemoveError::Commit(error)))?;
        restored
    } else if remove_all {
        if metadata_exists {
            fs::remove_dir(&metadata_dir).map_err(|source| {
                LocalizeError::Remove(RemoveError::RemoveDirectory {
                    path: metadata_dir.clone(),
                    source,
                })
            })?;
        }
        normalized
    } else {
        normalized
    };

    removed_languages.sort();
    Ok(RemoveReport {
        path: final_path,
        removed_languages,
    })
}

/// Lists all managed localizations for a `.localized` directory.
///
/// Non-localized directories return an empty list.
pub fn list_localizations(dir: &Path) -> Result<Vec<LocalizationEntry>> {
    list_localizations_impl(dir)
        .with_context(|| format!("failed to list localized names for {}", dir.display()))
}

fn list_localizations_impl(dir: &Path) -> Result<Vec<LocalizationEntry>> {
    let normalized = normalized_directory(dir)?;
    let dir_name = normalized
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            LocalizeError::Target(TargetError::NonUtf8FileName {
                path: normalized.clone(),
            })
        })?;
    if !dir_name.ends_with(LOCALIZED_SUFFIX) {
        return Ok(Vec::new());
    }

    let metadata_dir = normalized.join(".localized");
    if !metadata_directory_exists(&metadata_dir)? {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    for entry in fs::read_dir(&metadata_dir).map_err(|source| {
        LocalizeError::List(ListError::ReadDirectory {
            path: metadata_dir.clone(),
            source,
        })
    })? {
        let entry = entry.map_err(|source| {
            LocalizeError::List(ListError::ReadDirectory {
                path: metadata_dir.clone(),
                source,
            })
        })?;
        let path = entry.path();
        if !entry
            .file_type()
            .map_err(|source| {
                LocalizeError::List(ListError::ReadDirectory {
                    path: metadata_dir.clone(),
                    source,
                })
            })?
            .is_file()
            || path.extension().and_then(|value| value.to_str()) != Some("strings")
        {
            continue;
        }
        let language = path
            .file_stem()
            .and_then(|value| value.to_str())
            .ok_or_else(|| {
                LocalizeError::List(ListError::InvalidStrings {
                    path: path.clone(),
                    reason: StringsFormatError::ExpectedOpeningQuote { offset: 0 },
                })
            })?
            .to_owned();
        let contents = fs::read_to_string(&path).map_err(|source| {
            LocalizeError::List(ListError::ReadFile {
                path: path.clone(),
                source,
            })
        })?;
        let display_name = parse_strings_value(&contents).map_err(|reason| {
            LocalizeError::List(ListError::InvalidStrings {
                path: path.clone(),
                reason,
            })
        })?;
        entries.push(LocalizationEntry {
            language,
            display_name,
        });
    }

    entries.sort_by(|left, right| left.language.cmp(&right.language));
    Ok(entries)
}

fn metadata_directory_exists(path: &Path) -> std::result::Result<bool, LocalizeError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(MetadataError::SymbolicLink {
            path: path.to_owned(),
        }
        .into()),
        Ok(metadata) if metadata.is_dir() => Ok(true),
        Ok(_) => Ok(false),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(MetadataError::Inspect {
            path: path.to_owned(),
            source,
        }
        .into()),
    }
}

fn normalized_directory(path: &Path) -> std::result::Result<PathBuf, LocalizeError> {
    if !cfg!(target_os = "macos") {
        return Err(LocalizeError::UnsupportedPlatform);
    }

    let normalized: PathBuf = path.components().collect();
    let metadata = fs::symlink_metadata(&normalized).map_err(|source| {
        LocalizeError::Target(TargetError::Inspect {
            path: normalized.clone(),
            source,
        })
    })?;
    if metadata.file_type().is_symlink() {
        return Err(LocalizeError::Target(TargetError::SymbolicLink {
            path: normalized,
        }));
    }
    if !metadata.file_type().is_dir() {
        return Err(LocalizeError::Target(TargetError::NotDirectory {
            path: normalized,
        }));
    }

    Ok(normalized)
}

#[cfg(target_os = "macos")]
fn rename_directory_noreplace(from: &Path, to: &Path) -> std::result::Result<(), CommitError> {
    use rustix::fs::{CWD, RenameFlags, renameat_with};

    match renameat_with(CWD, from, CWD, to, RenameFlags::NOREPLACE) {
        Ok(()) => Ok(()),
        Err(error) if error == rustix::io::Errno::EXIST => Err(CommitError::DestinationExists {
            path: to.to_owned(),
        }),
        Err(source) => Err(CommitError::RenameDirectory {
            from: from.to_owned(),
            to: to.to_owned(),
            source: io::Error::from(source),
        }),
    }
}

#[cfg(not(target_os = "macos"))]
fn rename_directory_noreplace(from: &Path, to: &Path) -> std::result::Result<(), CommitError> {
    fs::rename(from, to).map_err(|source| CommitError::RenameDirectory {
        from: from.to_owned(),
        to: to.to_owned(),
        source,
    })
}

fn validate_display_name(name: &str) -> std::result::Result<(), DisplayNameError> {
    if name.is_empty() {
        return Err(DisplayNameError::Empty);
    }
    if name.contains('\0') {
        return Err(DisplayNameError::ContainsNul);
    }
    Ok(())
}

fn ensure_metadata_directory(path: &Path) -> std::result::Result<(), LocalizeError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(MetadataError::SymbolicLink {
            path: path.to_owned(),
        }
        .into()),
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(_) => Err(MetadataError::NotDirectory {
            path: path.to_owned(),
        }
        .into()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            fs::create_dir_all(path).map_err(|source| {
                MetadataError::CreateDirectory {
                    path: path.to_owned(),
                    source,
                }
                .into()
            })
        }
        Err(source) => Err(MetadataError::Inspect {
            path: path.to_owned(),
            source,
        }
        .into()),
    }
}

fn write_strings_file(
    metadata_dir: &Path,
    language: &str,
    source_name: &str,
    new_name: &str,
) -> std::result::Result<(), LocalizeError> {
    let target = metadata_dir.join(format!("{language}.strings"));
    let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let temporary = metadata_dir.join(format!(
        ".{language}.strings.tmp.{}.{}",
        std::process::id(),
        counter
    ));
    let contents = format!(
        "\"{}\" = \"{}\";\n",
        escape_strings_component(source_name),
        escape_strings_component(new_name)
    );

    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|source| MetadataError::CreateTemporaryFile {
                path: temporary.clone(),
                source,
            })?;
        file.write_all(contents.as_bytes()).map_err(|source| {
            MetadataError::WriteTemporaryFile {
                path: temporary.clone(),
                source,
            }
        })?;
        file.sync_all()
            .map_err(|source| MetadataError::SyncTemporaryFile {
                path: temporary.clone(),
                source,
            })?;
        fs::rename(&temporary, &target).map_err(|source| MetadataError::ReplaceFile {
            path: target.clone(),
            source,
        })?;
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }

    result.map_err(LocalizeError::Metadata)
}

fn normalize_language(language: &str, current: bool) -> std::result::Result<String, LanguageError> {
    const MAX_LANGUAGE_BYTES: usize = 64;

    let invalid = |reason| {
        if current {
            LanguageError::InvalidCurrent {
                language: language.to_owned(),
                reason,
            }
        } else {
            LanguageError::InvalidTarget {
                language: language.to_owned(),
                reason,
            }
        }
    };

    if language.is_empty() {
        return Err(invalid(InvalidLanguageReason::Empty));
    }
    if language.len() > MAX_LANGUAGE_BYTES {
        return Err(invalid(InvalidLanguageReason::TooLong {
            length: language.len(),
            max: MAX_LANGUAGE_BYTES,
        }));
    }
    if let Some(separator) = language
        .chars()
        .find(|character| matches!(character, '/' | '\\'))
    {
        return Err(invalid(InvalidLanguageReason::UnsupportedCharacter(
            separator,
        )));
    }

    let components: Vec<_> = language.split(['-', '_']).collect();
    if components.iter().any(|component| component.is_empty()) {
        return Err(invalid(InvalidLanguageReason::EmptyComponent));
    }
    if components.len() > 3 {
        return Err(invalid(InvalidLanguageReason::TooManyComponents {
            count: components.len(),
            max: 3,
        }));
    }
    for component in &components {
        for character in component.chars() {
            if !character.is_ascii_alphanumeric() {
                return Err(invalid(InvalidLanguageReason::UnsupportedCharacter(
                    character,
                )));
            }
        }
    }

    let language = components[0].to_ascii_lowercase();
    match components.as_slice() {
        [_] => Ok(language),
        [_, second] if is_script(second) => {
            Ok(format!("{language}_{}", canonicalize_script(second)))
        }
        [_, region] => Ok(format!("{language}_{}", region.to_ascii_uppercase())),
        [_, script, region] if is_script(script) => Ok(format!(
            "{language}_{}_{}",
            canonicalize_script(script),
            region.to_ascii_uppercase()
        )),
        _ => Err(invalid(InvalidLanguageReason::UnsupportedCharacter('_'))),
    }
}

fn is_script(component: &str) -> bool {
    component.len() == 4
        && component
            .chars()
            .all(|character| character.is_ascii_alphabetic())
}

fn canonicalize_script(script: &str) -> String {
    let mut characters = script.chars();
    let Some(first) = characters.next() else {
        return String::new();
    };

    first
        .to_ascii_uppercase()
        .to_string()
        .chars()
        .chain(characters.flat_map(char::to_lowercase))
        .collect()
}

fn escape_strings_component(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character => escaped.push(character),
        }
    }
    escaped
}

fn parse_strings_value(contents: &str) -> std::result::Result<String, StringsFormatError> {
    let mut parser = StringsParser {
        input: contents,
        offset: 0,
    };
    let _key = parser.parse_quoted()?;
    parser.skip_whitespace();
    parser.expect('=')?;
    parser.skip_whitespace();
    let value = parser.parse_quoted()?;
    parser.skip_whitespace();
    parser.expect(';')?;
    parser.skip_whitespace();
    if parser.offset != contents.len() {
        return Err(StringsFormatError::TrailingContent {
            offset: parser.offset,
        });
    }
    Ok(value)
}

struct StringsParser<'a> {
    input: &'a str,
    offset: usize,
}

impl StringsParser<'_> {
    fn parse_quoted(&mut self) -> std::result::Result<String, StringsFormatError> {
        let start = self.offset;
        self.skip_whitespace();
        self.expect('"')
            .map_err(|_| StringsFormatError::ExpectedOpeningQuote {
                offset: self.offset,
            })?;

        let mut value = String::new();
        loop {
            let Some(character) = self.peek() else {
                return Err(StringsFormatError::UnterminatedString { offset: start });
            };
            self.bump();
            match character {
                '"' => return Ok(value),
                '\\' => {
                    let escape_offset = self.offset.saturating_sub(1);
                    let Some(escaped) = self.peek() else {
                        return Err(StringsFormatError::UnterminatedString { offset: start });
                    };
                    self.bump();
                    match escaped {
                        '\\' => value.push('\\'),
                        '"' => value.push('"'),
                        'n' => value.push('\n'),
                        'r' => value.push('\r'),
                        't' => value.push('\t'),
                        escape => {
                            return Err(StringsFormatError::InvalidEscape {
                                escape,
                                offset: escape_offset,
                            });
                        }
                    }
                }
                character => value.push(character),
            }
        }
    }

    fn expect(&mut self, expected: char) -> std::result::Result<(), StringsFormatError> {
        if self.peek() != Some(expected) {
            return Err(match expected {
                '=' => StringsFormatError::ExpectedEquals {
                    offset: self.offset,
                },
                ';' => StringsFormatError::ExpectedSemicolon {
                    offset: self.offset,
                },
                '"' => StringsFormatError::ExpectedOpeningQuote {
                    offset: self.offset,
                },
                _ => unreachable!("unexpected parser token"),
            });
        }
        self.bump();
        Ok(())
    }

    fn skip_whitespace(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.bump();
        }
    }

    fn peek(&self) -> Option<char> {
        self.input[self.offset..].chars().next()
    }

    fn bump(&mut self) {
        if let Some(character) = self.peek() {
            self.offset += character.len_utf8();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use tempfile::tempdir;

    use super::{CommitError, LocalizeError, TargetError, localize, rename_directory_noreplace};

    fn localize_error(error: &anyhow::Error) -> &LocalizeError {
        error
            .downcast_ref::<LocalizeError>()
            .expect("localize should preserve its typed error")
    }

    #[test]
    fn localize_renames_directory_and_writes_localized_name() {
        let root = tempdir().unwrap();
        let source = root.path().join("Release Notes");
        fs::create_dir(&source).unwrap();

        let localized = localize(&source, Some("zh_CN"), "发布说明").unwrap();

        assert_eq!(localized, root.path().join("Release Notes.localized"));
        assert!(localized.is_dir());
        assert!(!source.exists());
        assert_eq!(
            fs::read_to_string(localized.join(".localized/zh_CN.strings")).unwrap(),
            "\"Release Notes\" = \"发布说明\";\n"
        );
    }

    #[test]
    fn localize_rejects_regular_files() {
        let root = tempdir().unwrap();
        let source = root.path().join("Report.txt");
        fs::write(&source, "report").unwrap();

        let error = localize(&source, Some("zh_CN"), "报告").unwrap_err();
        let chain = format!("{error:#}");

        assert!(chain.contains("failed to localize"));
        assert!(chain.contains("can only be applied to directories"));
        assert!(matches!(
            localize_error(&error),
            LocalizeError::Target(TargetError::NotDirectory { path }) if path == &source
        ));
    }

    #[cfg(unix)]
    #[test]
    fn localize_rejects_symbolic_link_with_trailing_separator() {
        use std::os::unix::fs::symlink;

        let root = tempdir().unwrap();
        let target = root.path().join("Target");
        let link = root.path().join("Link");
        fs::create_dir(&target).unwrap();
        symlink(&target, &link).unwrap();
        let link_with_separator = PathBuf::from(format!("{}/", link.display()));

        let error = localize(&link_with_separator, Some("zh_CN"), "链接").unwrap_err();

        assert!(matches!(
            localize_error(&error),
            LocalizeError::Target(TargetError::SymbolicLink { path }) if path == &link
        ));
        assert!(target.is_dir());
        assert!(link.symlink_metadata().unwrap().file_type().is_symlink());
    }

    #[test]
    fn localize_escapes_strings_file_keys_and_values() {
        let root = tempdir().unwrap();
        let source = root.path().join("Release \"Notes\"");
        fs::create_dir(&source).unwrap();

        let localized = localize(&source, Some("zh_CN"), "发布\\说明").unwrap();

        assert_eq!(
            fs::read_to_string(localized.join(".localized/zh_CN.strings")).unwrap(),
            "\"Release \\\"Notes\\\"\" = \"发布\\\\说明\";\n"
        );
    }

    #[test]
    fn localize_keeps_script_locales_separate() {
        let root = tempdir().unwrap();
        let source = root.path().join("Release Notes");
        fs::create_dir(&source).unwrap();

        let localized = localize(&source, Some("zh-Hans-CN"), "简体说明").unwrap();
        let localized = localize(&localized, Some("zh-Hant-CN"), "繁體說明").unwrap();

        assert_eq!(
            fs::read_to_string(localized.join(".localized/zh_Hans_CN.strings")).unwrap(),
            "\"Release Notes\" = \"简体说明\";\n"
        );
        assert_eq!(
            fs::read_to_string(localized.join(".localized/zh_Hant_CN.strings")).unwrap(),
            "\"Release Notes\" = \"繁體說明\";\n"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn finder_reads_current_language_display_name() {
        use objc2_foundation::{NSFileManager, NSString};

        let root = tempdir().unwrap();
        let source = root.path().join("Release Notes");
        fs::create_dir(&source).unwrap();

        let localized = localize(&source, None, "发布说明").unwrap();
        let localized_path = NSString::from_str(localized.to_str().unwrap());
        let display_name = NSFileManager::defaultManager()
            .displayNameAtPath(&localized_path)
            .to_string();

        assert_eq!(display_name, "发布说明");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn rename_directory_noreplace_preserves_existing_destination() {
        let root = tempdir().unwrap();
        let source = root.path().join("Source");
        let destination = root.path().join("Destination");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("keep"), "keep").unwrap();

        let error = rename_directory_noreplace(&source, &destination).unwrap_err();

        assert!(matches!(
            error,
            CommitError::DestinationExists { path } if path == destination
        ));
        assert!(source.is_dir());
        assert_eq!(
            fs::read_to_string(destination.join("keep")).unwrap(),
            "keep"
        );
    }
}
