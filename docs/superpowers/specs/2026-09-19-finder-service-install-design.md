# Finder Service Installation Design

## Goal

Extend `kako-helpers install` so components with Finder actions automatically
install a per-user macOS Service bundle. The initial service belongs to the
`localize` component and lets Finder invoke a native name-entry dialog for one
or more selected folders.

## Scope

- Add a shared component registry with optional Finder service metadata.
- Add `--no-service` to `install`; services are enabled by default.
- Install service bundles under `~/Library/Services`.
- Register service bundles through Launch Services.
- Skip an existing service without `--override`.
- Replace and re-register an existing service with `--override`.
- Use the same multicall binary as the AppKit service host.
- Show a native input dialog for each selected folder.
- Keep `install` as a CLI-only component with no service menu.

## Non-Goals

- No Finder Sync extension.
- No Automator workflow or AppleScript service generation.
- No automatic enabling in System Settings.
- No removal command for services in this iteration.
- No service registration for components without a Finder action.

## Architecture

The current executable serves two roles:

1. Normal CLI when the executable name is `kako-helpers`, `localize`, or
   `install`.
2. AppKit service host when the executable name is
   `KakoHelpersLocalizeService`.

Install creates a self-contained service bundle and copies the current
executable into it. The service host therefore does not depend on the folder
where `kako-helpers` was installed.

## Components

### Component Registry

Move `Component` out of `src/commands/install.rs` into `src/components.rs`.

```rust
pub enum Component {
    Localize,
    Install,
}
```

Each component exposes:

- `command_name() -> &'static str`
- `service_descriptor() -> Option<&'static ServiceDescriptor>`

Current mappings:

| Component | CLI name | Finder service |
| --- | --- | --- |
| `Localize` | `localize` | `Localize Folder` |
| `Install` | `install` | none |

Default install components remain `[Localize]`.

### Service Descriptor

```rust
pub struct ServiceDescriptor {
    pub component: Component,
    pub bundle_name: &'static str,
    pub bundle_identifier: &'static str,
    pub host_executable_name: &'static str,
    pub message: &'static str,
    pub menu_title: &'static str,
}
```

The `localize` descriptor uses:

```text
bundle_name          = KakoHelpersLocalize.service
bundle_identifier    = com.kako.helpers.localize-service
host_executable_name = KakoHelpersLocalizeService
message              = localizeFolder
menu_title           = Localize Folder
```

## Service Bundle Layout

```text
~/Library/Services/KakoHelpersLocalize.service/
  Contents/
    Info.plist
    MacOS/
      KakoHelpersLocalizeService
```

`KakoHelpersLocalizeService` is a copy of the current `kako-helpers`
executable with executable permissions preserved.

## Info.plist

Required entries:

```text
CFBundlePackageType             APPL
CFBundleExecutable              KakoHelpersLocalizeService
CFBundleIdentifier              com.kako.helpers.localize-service
CFBundleName                    Kako Helpers Localize
LSUIElement                     true
NSPrincipalClass                NSApplication
NSServices                      one service dictionary
```

The `NSServices` entry contains:

```text
NSMessage                       localizeFolder
NSMenuItem.default              Localize Folder
NSPortName                      KakoHelpersLocalizeService
NSSendFileTypes                 public.folder
NSRequiredContext               NSApplicationIdentifier = com.apple.finder
```

`NSRequiredContext` is always present so Finder is the only caller.

## Install Flow

1. Validate the destination folder.
2. Resolve selected components.
3. Preflight the CLI binary, component symlinks, and requested services.
4. Install `kako-helpers`.
5. Create component symlinks.
6. If services are enabled, install one service for every selected component
   with a `ServiceDescriptor`.
7. Register each newly written or replaced service bundle through Launch
   Services.

`--no-service` skips steps 6 and 7 entirely.

Components without a service descriptor are installed normally and produce no
service output.

## Duplicate Prevention

Service identity is the stable bundle path plus `CFBundleIdentifier`.

| State | Without `--override` | With `--override` |
| --- | --- | --- |
| Service path absent | install and register | install and register |
| Service path exists | output `Skipped` | replace and re-register |

Replacement uses a temporary bundle under `~/Library/Services`:

1. Build `.KakoHelpersLocalize.service.tmp.<pid>`.
2. Move the existing bundle to a backup path.
3. Move the temporary bundle to the canonical path with no replacement.
4. Remove the backup after the new bundle is committed.
5. Restore the backup if the new commit fails.

This prevents duplicate services with the same bundle identifier.

## Launch Services Registration

After a service bundle is created or replaced, run:

```text
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f <bundle>
```

Registration is not repeated when a service is skipped.

The user still enables the service manually:

```text
System Settings -> Keyboard -> Keyboard Shortcuts -> Services
```

## Service Host

`main.rs` checks the executable file name before CLI parsing. When it is
`KakoHelpersLocalizeService`, it starts the AppKit service host instead of the
CLI command tree.

The host:

1. Creates the shared `NSApplication`.
2. Sets an Objective-C service-provider object on `NSApplication`.
3. Registers `localizeFolder:userData:error:` through `objc2`.
4. Reads file URLs from `NSPasteboard`.
5. Filters the URLs to existing directories.
6. Shows one `NSAlert` with an `NSTextField` for each selected folder.
7. Calls the existing localizer library function with the folder and entered
   name.
8. Accumulates failures and presents a final native error alert if needed.

The service host does not spawn the CLI process and does not depend on the
installation destination.

## Interaction Details

- Dialog title: `Localize Folder`
- Dialog text: `Enter the localized display name.`
- Buttons: `Set` and `Cancel`
- Empty input is rejected, an explanatory alert is shown, and the name dialog
  is presented again.
- Cancel skips that folder.
- Multiple selected folders produce one dialog per folder.
- Successful operations do not open Terminal or write CLI status output.
- Errors are shown in a native alert.

## Output

Service installation uses the existing `OutputStyle`:

```text
Successfully installed Finder service Localize Folder.
Skipped Localize Folder: service already exists.
```

The words `Successfully` and `Localize Folder` are highlighted. Secondary text
is muted.

## Error Model

Add a typed `ServiceError` hierarchy:

- service home inspection and creation
- staging bundle creation
- Info.plist and host executable write failures
- existing bundle conflict
- backup, commit, and rollback failures
- Launch Services registration failures
- invalid or missing service metadata

Service failures remain wrapped in `anyhow::Error` with the install command
context.

## Dependencies

Add the macOS-target dependency:

```toml
objc2-app-kit = "0.3.2"
```

Existing `objc2-foundation` supplies pasteboard-related Foundation types.

## Verification

No additional automated tests are added in this iteration. Verification uses:

- `cargo fmt --all -- --check`
- `cargo clippy --locked --all-targets -- -D warnings`
- existing `cargo test --locked`
- a temporary `HOME` for bundle layout checks
- direct Info.plist inspection
- an isolated Launch Services registration smoke test
- manual Finder menu verification after enabling the service

## Known Limitations

- The user must enable the service in System Settings.
- Finder or Services caches may require a Finder restart or re-login.
- The localize dialog sets one folder at a time even when multiple folders are
  selected.
- Service removal is not part of this iteration.
