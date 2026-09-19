mod cli;
mod commands;
mod components;
mod output;
mod service;
pub mod tools;

fn main() {
    let style = output::OutputStyle {
        emphasis: anstyle::Style::new()
            .fg_color(Some(anstyle::AnsiColor::Green.into()))
            .bold(),
        highlight: anstyle::Style::new()
            .fg_color(Some(anstyle::AnsiColor::BrightWhite.into()))
            .bold(),
        muted: anstyle::Style::new().fg_color(Some(anstyle::AnsiColor::BrightBlack.into())),
        skipped: anstyle::Style::new()
            .fg_color(Some(anstyle::AnsiColor::BrightWhite.into()))
            .bold(),
        error: anstyle::Style::new()
            .fg_color(Some(anstyle::AnsiColor::BrightRed.into()))
            .bold(),
    };

    if service::is_service_host() {
        if let Err(error) = service::run_service_host() {
            style.error(format!("{error:#}"));
            std::process::exit(1);
        }
        return;
    }

    if let Err(error) = cli::run(style) {
        style.error(format!("{error:#}"));
        std::process::exit(1);
    }
}
