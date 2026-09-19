# kako-helpers

[English](./README.md) | 简体中文

`kako-helpers` 是 kako_ 的 macOS 实用工具库和命令行工具。

当前实现的功能仅有 Finder Localized Name 管理。

## 环境要求

- macOS
- Rust 2024 工具链

## 构建

```console
cargo build --release
```

## Localize

Localized Name 用于修改 Finder 中目录的显示名称，不修改目录的真实文件名。

`localize` 命令管理保存在以下结构中的 Finder 显示名称：

```text
Name.localized/
 | .localized/
 |  | en.strings
 |  | zh_CN.strings
```

如果传入路径不以 `.localized` 结尾，程序会先检查原路径，再尝试
`<path>.localized`。使用 `--strict` 或 `-S` 可以禁止回退。

### 设置本地化名称

```console
kako-helpers localize set <PATH> <NAME> [OPTIONS]
```

参数：

```text
--lang <LANG>          生成 .strings 文件时使用的语言标识
-C, --create-symlink   在原名位置创建指向本地化目录的 symlink
-S, --strict           不尝试 <PATH>.localized
-s, --silent           不输出成功信息
```

示例：

```console
kako-helpers localize set ./Release\ Notes 发布说明 --lang zh_CN
```

真实目录会变为：

```text
Release Notes.localized
```

使用 `--create-symlink` 时，原名位置会保留一个 symlink：

```text
Release Notes -> Release Notes.localized
```

### 移除本地化名称

```console
kako-helpers localize remove <PATH> [OPTIONS]
```

参数：

```text
--lang <LANG>     只移除指定语言；不传则移除全部语言
-N, --no-rename   保留 .localized 目录名，不恢复原名
-S, --strict      不尝试 <PATH>.localized
-s, --silent      不输出成功信息
```

不使用 `--no-rename` 时，删除最后一个语言后会恢复：

```text
Name.localized -> Name
```

如果原名路径已经存在，它必须是指向该本地化目录的 symlink；否则操作会拒绝执行。

本地化 metadata 不存在或为空时，会视为空语言集合处理。

### 列出本地化名称

```console
kako-helpers localize list <PATH> [--strict]
```

输出示例：

```text
en = Release Notes
zh_CN = 发布说明
```

## 安装

```console
kako-helpers install <FOLDER> [OPTIONS]
```

参数：

```text
-C, --components <COMPONENT>...  指定安装的组件
-O, --override                   覆盖已有文件和 symlink
--style <default|prefix>         symlink 命名风格
--no-service                     不安装 Finder Service
```

目标文件夹必须已经存在。

默认只安装 `localize` 组件：

```text
<FOLDER>/kako-helpers
<FOLDER>/localize -> kako-helpers
```

使用 `--components install` 可以额外或单独安装 `install` 组件。

使用 `--style prefix` 时，symlink 名称为：

```text
kako-localize -> kako-helpers
kako-install  -> kako-helpers
```

## Finder Service

默认会安装 Finder Service，路径为：

```text
~/Library/Services/KakoHelpersLocalize.service
```

可用的 Finder 文件夹菜单：

```text
Localizer: Localize Folder
Localizer: Localize Folder with Symlink
Localizer: Remove Localized Names
Localizer: Remove Localized Names without Rename
```

用户需要在以下位置启用服务：

```text
系统设置 -> 键盘 -> 键盘快捷键 -> 服务
```

安装或覆盖 Service 后，交互式终端会询问是否重启 Finder。输入 `y` 即可重启。

