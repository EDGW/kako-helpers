use std::fmt::Display;
use std::io::Write;
use std::path::Path;

use anstyle::Style;

#[derive(Debug, Clone, Copy)]
pub struct OutputStyle {
    pub emphasis: Style,
    pub highlight: Style,
    pub muted: Style,
    pub skipped: Style,
    pub error: Style,
}

impl OutputStyle {
    pub fn set_localized_name(&self, silent: bool, path: &Path, name: &str) {
        if silent {
            return;
        }

        let mut stdout = anstream::stdout();
        let _ = write!(
            stdout,
            "{}Successfully{} ",
            self.emphasis,
            self.emphasis.render_reset()
        );
        let _ = write!(
            stdout,
            "{}set localized name for{} ",
            self.muted,
            self.muted.render_reset()
        );
        let _ = write!(
            stdout,
            "{}{}{}",
            self.highlight,
            path.display(),
            self.highlight.render_reset()
        );
        let _ = write!(stdout, "{} to {}", self.muted, self.muted.render_reset());
        let _ = write!(
            stdout,
            "{}{}{}",
            self.highlight,
            name,
            self.highlight.render_reset()
        );
        let _ = writeln!(stdout, "{}.{}", self.muted, self.muted.render_reset());
    }

    pub fn removed_language(&self, silent: bool, language: &str, path: &Path) {
        if silent {
            return;
        }

        let mut stdout = anstream::stdout();
        let _ = write!(
            stdout,
            "{}Successfully{} ",
            self.emphasis,
            self.emphasis.render_reset()
        );
        let _ = write!(
            stdout,
            "{}removed{} ",
            self.muted,
            self.muted.render_reset()
        );
        let _ = write!(
            stdout,
            "{}{}{}",
            self.highlight,
            language,
            self.highlight.render_reset()
        );
        let _ = write!(stdout, "{} from {}", self.muted, self.muted.render_reset());
        let _ = write!(
            stdout,
            "{}{}{}",
            self.highlight,
            path.display(),
            self.highlight.render_reset()
        );
        let _ = writeln!(stdout, "{}.{}", self.muted, self.muted.render_reset());
    }

    pub fn removed_all(&self, silent: bool, path: &Path) {
        if silent {
            return;
        }

        let mut stdout = anstream::stdout();
        let _ = write!(
            stdout,
            "{}Successfully{} ",
            self.emphasis,
            self.emphasis.render_reset()
        );
        let _ = write!(
            stdout,
            "{}removed all localized names from{} ",
            self.muted,
            self.muted.render_reset()
        );
        let _ = write!(
            stdout,
            "{}{}{}",
            self.highlight,
            path.display(),
            self.highlight.render_reset()
        );
        let _ = writeln!(stdout, "{}.{}", self.muted, self.muted.render_reset());
    }

    pub fn installed(&self, path: &Path) {
        let mut stdout = anstream::stdout();
        let _ = write!(
            stdout,
            "{}Successfully{} ",
            self.emphasis,
            self.emphasis.render_reset()
        );
        let _ = write!(
            stdout,
            "{}installed kako-helpers to{} ",
            self.muted,
            self.muted.render_reset()
        );
        let _ = write!(
            stdout,
            "{}{}{}",
            self.highlight,
            path.display(),
            self.highlight.render_reset()
        );
        let _ = writeln!(stdout, "{}.{}", self.muted, self.muted.render_reset());
    }

    pub fn created_symlink(&self, name: &str, target: &str) {
        let mut stdout = anstream::stdout();
        let _ = write!(
            stdout,
            "{}Successfully{} ",
            self.emphasis,
            self.emphasis.render_reset()
        );
        let _ = write!(
            stdout,
            "{}created symlink{} ",
            self.muted,
            self.muted.render_reset()
        );
        let _ = write!(
            stdout,
            "{}{}{}",
            self.highlight,
            name,
            self.highlight.render_reset()
        );
        let _ = write!(stdout, "{} -> {}", self.muted, self.muted.render_reset());
        let _ = write!(
            stdout,
            "{}{}{}",
            self.highlight,
            target,
            self.highlight.render_reset()
        );
        let _ = writeln!(stdout, "{}.{}", self.muted, self.muted.render_reset());
    }

    pub fn service_installed(&self, title: &str) {
        let mut stdout = anstream::stdout();
        let _ = write!(
            stdout,
            "{}Successfully{} ",
            self.emphasis,
            self.emphasis.render_reset()
        );
        let _ = write!(
            stdout,
            "{}installed Finder service{} ",
            self.muted,
            self.muted.render_reset()
        );
        let _ = write!(
            stdout,
            "{}{}{}",
            self.highlight,
            title,
            self.highlight.render_reset()
        );
        let _ = writeln!(stdout, "{}.{}", self.muted, self.muted.render_reset());
    }

    pub fn service_skipped(&self, title: &str) {
        let mut stdout = anstream::stdout();
        let _ = write!(
            stdout,
            "{}Skipped{} ",
            self.skipped,
            self.skipped.render_reset()
        );
        let _ = write!(
            stdout,
            "{}{}{}",
            self.highlight,
            title,
            self.highlight.render_reset()
        );
        let _ = writeln!(
            stdout,
            "{}: service already exists.{}",
            self.muted,
            self.muted.render_reset()
        );
    }

    pub fn list_entry(&self, language: &str, display_name: &str) {
        let mut stdout = anstream::stdout();
        let _ = write!(
            stdout,
            "{}{}{} ",
            self.highlight,
            language,
            self.highlight.render_reset()
        );
        let _ = write!(stdout, "{}=", self.muted);
        let _ = write!(stdout, "{} ", self.muted.render_reset());
        let _ = writeln!(
            stdout,
            "{}{}{}",
            self.highlight,
            display_name,
            self.highlight.render_reset()
        );
    }

    pub fn info(&self, message: impl Display) {
        let mut stdout = anstream::stdout();
        let _ = writeln!(
            stdout,
            "{}{message}{}",
            self.muted,
            self.muted.render_reset()
        );
    }

    pub fn skipped(&self, name: &str) {
        let mut stdout = anstream::stdout();
        let _ = writeln!(
            stdout,
            "{}Skipped{} {}symlink{} {}{name}{} {}already exists.{}",
            self.skipped,
            self.skipped.render_reset(),
            self.muted,
            self.muted.render_reset(),
            self.highlight,
            self.highlight.render_reset(),
            self.muted,
            self.muted.render_reset()
        );
    }

    pub fn error(&self, message: impl Display) {
        let mut stderr = anstream::stderr();
        let _ = writeln!(
            stderr,
            "{}Error:{} {}{message}{}",
            self.error,
            self.error.render_reset(),
            self.highlight,
            self.highlight.render_reset()
        );
    }
}
