# kako-helpers

`kako-helpers` is a macOS utility CLI and helper library for Kako_ tooling.
It currently provides one feature: Finder Localized Name management.

Localized Names change how Finder displays a directory without changing the
underlying filesystem name.

## Requirements

- macOS
- Rust 2024 toolchain

## Build

```console
cargo build --release
```

## Localize

The `localize` command manages Finder display names stored in:

```text
Name.localized/
  .localized/
    en.strings
    zh_CN.strings
```

If the supplied path does not end with `.localized`, `kako-helpers` first
checks the exact path, then tries `<path>.localized`. Use `--strict` or `-S` to
disable this fallback.

### Set a localized name

```console
kako-helpers localize set <PATH> <NAME> [OPTIONS]
```

Options:

```text
--lang <LANG>          Language identifier for the generated .strings file
-C, --create-symlink   Create a symlink at the original name
-S, --strict           Do not try <PATH>.localized
-s, --silent           Suppress success output
```

Example:

```console
kako-helpers localize set ./Release\ Notes 发布说明 --lang zh_CN
```

The real directory becomes:

```text
Release Notes.localized
```

With `--create-symlink`, the original path is kept as a symlink:

```text
Release Notes -> Release Notes.localized
```

### Remove localized names

```console
kako-helpers localize remove <PATH> [OPTIONS]
```

Options:

```text
--lang <LANG>     Remove only this language; omit to remove all languages
-N, --no-rename   Keep the .localized directory name
-S, --strict      Do not try <PATH>.localized
-s, --silent      Suppress success output
```

Without `--no-rename`, removing the last language restores:

```text
Name.localized -> Name
```

If the original name already exists, it must be a symlink pointing to the
localized directory. Otherwise the operation fails closed.

Missing or empty localization metadata is treated as an empty language set.

### List localized names

```console
kako-helpers localize list <PATH> [--strict]
```

Example output:

```text
en = Release Notes
zh_CN = 发布说明
```

## Install

```console
kako-helpers install <FOLDER> [OPTIONS]
```

Options:

```text
-C, --components <COMPONENT>...  Components to install
-O, --override                   Replace existing files and symlinks
--style <default|prefix>         Symlink naming style
--no-service                     Do not install Finder Services
```

The destination folder must already exist.

By default, only the `localize` component is installed:

```text
<FOLDER>/kako-helpers
<FOLDER>/localize -> kako-helpers
```

Use `--components install` to install the `install` symlink instead.

With `--style prefix`, symlinks are named:

```text
kako-localize -> kako-helpers
kako-install  -> kako-helpers
```

## Finder Services

Service installation is enabled by default. The service bundle is installed at:

```text
~/Library/Services/KakoHelpersLocalize.service
```

Available Finder folder actions:

```text
Localizer: Localize Folder
Localizer: Localize Folder with Symlink
Localizer: Remove Localized Names
Localizer: Remove Localized Names without Rename
```

The user must enable the services in:

```text
System Settings -> Keyboard -> Keyboard Shortcuts -> Services
```

After installing or replacing a service, an interactive terminal asks whether
to restart Finder. Enter `y` to restart it.

