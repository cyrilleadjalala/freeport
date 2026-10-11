mod app;
mod cli;
mod docker;
mod finder;
mod i18n;
mod killer;
mod ports;
mod prompt;
mod report;

use clap::{CommandFactory, FromArgMatches};
use std::process::ExitCode;

fn main() -> ExitCode {
    let lang = i18n::Lang::detect();
    let matches = i18n::localize(cli::Cli::command(), lang).get_matches();
    let cli = cli::Cli::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());
    app::run(cli, lang)
}
