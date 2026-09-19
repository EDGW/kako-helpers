use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

#[cfg(target_os = "macos")]
use objc2::rc::Retained;
#[cfg(target_os = "macos")]
use objc2::runtime::{AnyObject, NSObject};
#[cfg(target_os = "macos")]
use objc2::{MainThreadOnly, define_class, msg_send};
#[cfg(target_os = "macos")]
use objc2_app_kit::{
    NSAlert, NSAlertFirstButtonReturn, NSApplication, NSApplicationActivationPolicy, NSPasteboard,
    NSPasteboardTypeFileURL, NSTextField,
};
#[cfg(target_os = "macos")]
use objc2_foundation::{
    MainThreadMarker, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString, NSURL,
};

#[cfg(target_os = "macos")]
define_class!(
    // SAFETY: NSObject has no subclassing requirements, and the provider has no Drop.
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    struct ServiceProvider;

    // SAFETY: NSObjectProtocol has no safety requirements.
    unsafe impl NSObjectProtocol for ServiceProvider {}

    impl ServiceProvider {
        // SAFETY: This matches the selector expected by NSServices.
        #[unsafe(method(localizeFolder:userData:error:))]
        fn localize_folder(
            &self,
            pasteboard: &NSPasteboard,
            _user_data: Option<&NSString>,
            error: *mut *mut NSString,
        ) {
            handle_localize(
                self.mtm(),
                pasteboard,
                error,
                false,
                "Localizer: Localize Folder",
            );
        }

        // SAFETY: This matches the selector expected by NSServices.
        #[unsafe(method(localizeFolderWithSymlink:userData:error:))]
        fn localize_folder_with_symlink(
            &self,
            pasteboard: &NSPasteboard,
            _user_data: Option<&NSString>,
            error: *mut *mut NSString,
        ) {
            handle_localize(
                self.mtm(),
                pasteboard,
                error,
                true,
                "Localizer: Localize Folder with Symlink",
            );
        }

        // SAFETY: This matches the selector expected by NSServices.
        #[unsafe(method(removeLocalizedNames:userData:error:))]
        fn remove_localized_names(
            &self,
            pasteboard: &NSPasteboard,
            _user_data: Option<&NSString>,
            error: *mut *mut NSString,
        ) {
            handle_remove(
                self.mtm(),
                pasteboard,
                error,
                false,
                "Localizer: Remove Localized Names",
            );
        }

        // SAFETY: This matches the selector expected by NSServices.
        #[unsafe(method(removeLocalizedNamesWithoutRename:userData:error:))]
        fn remove_localized_names_without_rename(
            &self,
            pasteboard: &NSPasteboard,
            _user_data: Option<&NSString>,
            error: *mut *mut NSString,
        ) {
            handle_remove(
                self.mtm(),
                pasteboard,
                error,
                true,
                "Localizer: Remove Localized Names without Rename",
            );
        }
    }
);

#[cfg(target_os = "macos")]
impl ServiceProvider {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let object = Self::alloc(mtm).set_ivars(());
        // SAFETY: NSObject's init signature is correct.
        unsafe { msg_send![super(object), init] }
    }
}

pub fn is_service_host() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.file_name().map(|name| name.to_owned()))
        .is_some_and(|name| name == "KakoHelpersLocalizeService")
}

#[cfg(target_os = "macos")]
pub fn run_service_host() -> Result<()> {
    let mtm = MainThreadMarker::new().context("Finder service host must run on the main thread")?;
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

    let provider = ServiceProvider::new(mtm);
    let provider: &AnyObject = provider.as_ref();
    // SAFETY: The provider implements the NSServices callback selector.
    unsafe { app.setServicesProvider(Some(provider)) };

    app.run();
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn run_service_host() -> Result<()> {
    anyhow::bail!("Finder services are only supported on macOS")
}

#[cfg(target_os = "macos")]
fn file_urls(pasteboard: &NSPasteboard) -> Vec<PathBuf> {
    let Some(items) = pasteboard.pasteboardItems() else {
        return Vec::new();
    };

    items
        .iter()
        .filter_map(|item| {
            let value = item.stringForType(unsafe { NSPasteboardTypeFileURL })?;
            let url = NSURL::URLWithString(&value)?;
            let path = url.path()?;
            let path = PathBuf::from(path.to_string());
            path.is_dir().then_some(path)
        })
        .collect()
}

#[cfg(target_os = "macos")]
fn handle_localize(
    mtm: MainThreadMarker,
    pasteboard: &NSPasteboard,
    error: *mut *mut NSString,
    create_symlink: bool,
    title: &str,
) {
    activate_ui(mtm);
    let folders = file_urls(pasteboard);
    if folders.is_empty() {
        let message = format!("No folders were provided to {title}.");
        set_service_error(error, &message);
        show_error(mtm, title, &message);
        return;
    }

    let mut failures = Vec::new();
    for folder in folders {
        let Some(name) = prompt_for_name(mtm, &folder) else {
            continue;
        };
        if let Err(error) = crate::tools::localizer::localize_with_options(
            &folder,
            None,
            &name,
            crate::tools::localizer::LocalizeOptions { create_symlink },
        ) {
            failures.push(format!("{}: {error:#}", folder.display()));
        }
    }

    if !failures.is_empty() {
        let message = failures.join("\n");
        set_service_error(error, &message);
        show_error(mtm, title, &message);
    }
}

#[cfg(target_os = "macos")]
fn handle_remove(
    mtm: MainThreadMarker,
    pasteboard: &NSPasteboard,
    error: *mut *mut NSString,
    no_rename: bool,
    title: &str,
) {
    activate_ui(mtm);
    let folders = file_urls(pasteboard);

    if !confirm_remove(mtm, title) {
        return;
    }
    if folders.is_empty() {
        let message = format!("No folders were provided to {title}.");
        set_service_error(error, &message);
        show_error(mtm, title, &message);
        return;
    }

    let mut failures = Vec::new();
    for folder in folders {
        if let Err(error) = crate::tools::localizer::remove_localization_with_options(
            &folder,
            None,
            crate::tools::localizer::RemoveOptions { no_rename },
        ) {
            failures.push(format!("{}: {error:#}", folder.display()));
        }
    }

    if !failures.is_empty() {
        let message = failures.join("\n");
        set_service_error(error, &message);
        show_error(mtm, title, &message);
    }
}

#[cfg(target_os = "macos")]
fn prompt_for_name(mtm: MainThreadMarker, folder: &Path) -> Option<String> {
    loop {
        let alert = NSAlert::new(mtm);
        alert.setMessageText(&NSString::from_str("Localize Folder"));
        alert.setInformativeText(&NSString::from_str(&format!(
            "Enter the localized display name for {}.",
            folder.display()
        )));

        let field = NSTextField::initWithFrame(
            NSTextField::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(320.0, 24.0)),
        );
        alert.setAccessoryView(Some(&field));
        alert.addButtonWithTitle(&NSString::from_str("Set"));
        alert.addButtonWithTitle(&NSString::from_str("Cancel"));

        if alert.runModal() != NSAlertFirstButtonReturn {
            return None;
        }

        let name = field.stringValue().to_string();
        if !name.is_empty() {
            return Some(name);
        }

        show_error(mtm, "Localize Folder", "The display name cannot be empty.");
    }
}

#[cfg(target_os = "macos")]
fn confirm_remove(mtm: MainThreadMarker, title: &str) -> bool {
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&NSString::from_str(title));
    alert.setInformativeText(&NSString::from_str(
        "Remove all localized names from the selected Finder folders?",
    ));
    alert.addButtonWithTitle(&NSString::from_str("Remove"));
    alert.addButtonWithTitle(&NSString::from_str("Cancel"));
    alert.runModal() == NSAlertFirstButtonReturn
}

#[cfg(target_os = "macos")]
fn show_error(mtm: MainThreadMarker, title: &str, message: &str) {
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&NSString::from_str(title));
    alert.setInformativeText(&NSString::from_str(message));
    alert.addButtonWithTitle(&NSString::from_str("OK"));
    alert.runModal();
}

#[cfg(target_os = "macos")]
fn activate_ui(mtm: MainThreadMarker) {
    let app = NSApplication::sharedApplication(mtm);
    #[allow(deprecated)]
    app.activateIgnoringOtherApps(true);
}

#[cfg(target_os = "macos")]
fn set_service_error(error: *mut *mut NSString, message: &str) {
    if error.is_null() {
        return;
    }

    let message = Retained::autorelease_ptr(NSString::from_str(message));
    // SAFETY: The caller supplied a valid NSServices error out-parameter.
    unsafe { *error = message };
}
