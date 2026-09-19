# Finder Service Installation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Install a per-user Finder Service for the `localize` component by
default, skip duplicate registrations, support `--no-service`, and use the same
multicall binary as an AppKit service host with a native name-entry dialog.

**Architecture:** Move component metadata into a shared registry, add a
dedicated `service` subsystem for bundle creation and Launch Services
registration, and branch the multicall executable into AppKit service-host mode
when it is executed as `KakoHelpersLocalizeService`.

**Tech Stack:** Rust 2024, Clap, anyhow, thiserror, objc2, objc2-foundation,
objc2-app-kit, anstyle/anstream

**Spec:**
`docs/superpowers/specs/2026-09-19-finder-service-install-design.md`

## Global Constraints

- Service bundles live at
  `~/Library/Services/KakoHelpersLocalize.service`.
- Bundle identifier: `com.kako.helpers.localize-service`.
- Host executable: `KakoHelpersLocalizeService`.
- Finder menu title: `Localize Folder`.
- Finder filters to `public.folder`.
- Services are installed by default.
- `--no-service` disables service installation and registration.
- `--override` replaces an existing service bundle and re-registers it.
- Without `--override`, an existing service bundle produces `Skipped`.
- Only components with a service descriptor register Finder services.
- `install` has no service descriptor.
- No additional automated tests are added.

---

### Task 1: Shared Component and Service Registry

**Files:**

- Create: `src/components.rs`
- Modify: `src/main.rs`
- Modify: `src/commands/install.rs`

**Interfaces:**

- Produces:
  - `pub enum Component { Localize, Install }`
  - `pub enum LinkStyle { Default, Prefix }`
  - `pub struct ServiceDescriptor`
  - `Component::DEFAULT`
  - `Component::command_name(self) -> &'static str`
  - `Component::service_descriptor(self) -> Option<&'static ServiceDescriptor>`
  - `Component::link_name(self, style: LinkStyle) -> String`

- [ ] Move `Component` and `LinkStyle` out of `commands/install.rs`.
- [ ] Add the static `LOCALIZE_SERVICE` descriptor with the exact values from
  the design spec.
- [ ] Update `commands/install.rs` to import the shared types.
- [ ] Run `cargo check --locked`.

### Task 2: Service Bundle Installation

**Files:**

- Create: `src/service/mod.rs`
- Create: `src/service/bundle.rs`
- Modify: `Cargo.toml`
- Modify: `src/main.rs`
- Modify: `src/commands/install.rs`
- Modify: `src/output.rs`

**Interfaces:**

- Produces:
  - `pub fn install_component_services(components: &[Component], source: &Path, override_existing: bool, style: OutputStyle) -> anyhow::Result<()>`
  - `pub fn is_service_host() -> bool`
  - `pub fn run_service_host() -> anyhow::Result<()>`
  - `OutputStyle::service_installed(title: &str)`
  - `OutputStyle::service_skipped(title: &str)`

- [ ] Add `objc2-app-kit = "0.3.2"` for macOS targets.
- [ ] Implement typed `ServiceError` variants for inspection, staging,
  Info.plist creation, executable copy, backup, commit, rollback, and
  `lsregister`.
- [ ] Implement `service_home()` using `$HOME/Library/Services`.
- [ ] Generate `Contents/Info.plist` with the exact keys and values from the
  design spec.
- [ ] Copy the current executable into `Contents/MacOS/KakoHelpersLocalizeService`
  with executable permissions.
- [ ] Build staging bundles as
  `~/Library/Services/.KakoHelpersLocalize.service.tmp.<pid>`.
- [ ] Skip an existing canonical bundle without `--override`.
- [ ] Replace an existing bundle through backup and no-replace commit with
  rollback on failure.
- [ ] Register created or replaced bundles with the absolute `lsregister -f`
  path.
- [ ] Add `--no-service` to `InstallArgs`.
- [ ] Call `install_component_services` after CLI and symlink installation when
  `--no-service` is absent.
- [ ] Run `cargo check --locked`.

### Task 3: AppKit Service Host

**Files:**

- Create: `src/service/host.rs`
- Modify: `src/service/mod.rs`
- Modify: `src/main.rs`

**Interfaces:**

- Produces:
  - `pub fn is_service_host() -> bool`
  - `pub fn run_service_host() -> anyhow::Result<()>`
  - Objective-C selector implementation:
    `localizeFolder:userData:error:`

- [ ] Detect `KakoHelpersLocalizeService` from `current_exe().file_name()`
  before CLI parsing.
- [ ] Initialize `NSApplication.sharedApplication`.
- [ ] Use `objc2::define_class!` to define an Objective-C service provider.
- [ ] Set the provider with `NSApplication::setServicesProvider`.
- [ ] Read file URLs from the service pasteboard using
  `readObjectsForClasses:options:`.
- [ ] Filter results to existing directories.
- [ ] Show one `NSAlert` per selected folder with an attached `NSTextField`.
- [ ] Reject empty input with an explanatory alert and reopen the name dialog.
- [ ] Call `tools::localizer::localize` with the selected folder and entered
  name.
- [ ] Accumulate failures and show a final native error alert when needed.
- [ ] Run `cargo check --locked`.

### Task 4: Integration and Verification

**Files:**

- Modify: `src/commands/install.rs`
- Modify: `src/cli.rs`

**Interfaces:**

- Consumes all interfaces from Tasks 1-3.

- [ ] Confirm `kako-helpers install --help` includes `--no-service`.
- [ ] Confirm `kako-helpers install <folder>` installs the service by default.
- [ ] Confirm `kako-helpers install <folder> --no-service` creates no service
  bundle.
- [ ] Confirm a repeated install without `--override` reports the existing
  service as `Skipped`.
- [ ] Confirm `--override` replaces the service bundle and invokes
  `lsregister`.
- [ ] Inspect the generated Info.plist with `plutil -p`.
- [ ] Confirm the installed service host executable runs in service-host mode
  instead of entering the CLI parser.
- [ ] Run `cargo fmt --all -- --check`.
- [ ] Run `cargo test --locked`.
- [ ] Run `cargo clippy --locked --all-targets -- -D warnings`.
- [ ] Manually verify the Finder menu item after enabling it in System
  Settings.
