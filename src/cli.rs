use crate::ports::{self, PortSpec};
use clap::Parser;
use clap_complete::Shell;
use std::net::IpAddr;

/// Free a port by stopping the process that holds it.
#[derive(Parser)]
#[command(name = "freeport", version)]
pub struct Cli {
    /// Port(s) or range(s) to free, e.g. 3000 8080 3000-3010 (none: list listening ports)
    #[arg(value_name = "PORT", value_parser = ports::parse_spec)]
    pub ports: Vec<PortSpec>,

    /// List listening ports (default when no port is given)
    #[arg(short, long, conflicts_with_all = ["dry_run", "force", "interactive", "tree", "allow_docker"])]
    pub list: bool,

    /// Show the processes involved without stopping anything
    #[arg(short = 'n', long)]
    pub dry_run: bool,

    /// Kill immediately (SIGKILL), without a polite stop first
    #[arg(short, long)]
    pub force: bool,

    /// Target UDP instead of TCP
    #[arg(short, long)]
    pub udp: bool,

    /// Seconds granted to the polite stop before forcing
    #[arg(short, long, default_value_t = 3)]
    pub timeout: u64,

    /// Ask for confirmation before each stop
    #[arg(short, long)]
    pub interactive: bool,

    /// Only target processes listening on this IP address
    #[arg(long, value_name = "IP")]
    pub addr: Option<IpAddr>,

    /// Also stop supervisors (which would restart the service) and child processes
    #[arg(long)]
    pub tree: bool,

    /// Allow stopping a process managed by Docker (not recommended)
    #[arg(long)]
    pub allow_docker: bool,

    /// JSON output, for scripts
    #[arg(long)]
    pub json: bool,

    /// Print nothing on standard output (errors still go to stderr)
    #[arg(short, long)]
    pub quiet: bool,

    /// Write the completion script for the given shell to standard output
    #[arg(long, value_name = "SHELL", exclusive = true)]
    pub completions: Option<Shell>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{self, Lang};
    use clap::CommandFactory;

    #[test]
    fn command_definition_is_valid_in_both_languages() {
        Cli::command().debug_assert();
        i18n::localize(Cli::command(), Lang::Fr).debug_assert();
    }

    #[test]
    fn french_help_translates_every_argument() {
        let en = Cli::command().render_help().to_string();
        let fr = i18n::localize(Cli::command(), Lang::Fr)
            .render_help()
            .to_string();
        assert!(en.contains("Free a port"));
        assert!(fr.contains("Libère un port"));
        for arg in Cli::command().get_arguments() {
            if let Some(h) = arg.get_help() {
                let id = arg.get_id().as_str();
                if id == "help" || id == "version" {
                    continue;
                }
                let fr_cmd = i18n::localize(Cli::command(), Lang::Fr);
                let fr_arg = fr_cmd.get_arguments().find(|a| a.get_id() == id).unwrap();
                assert_ne!(
                    fr_arg.get_help().map(|x| x.to_string()),
                    Some(h.to_string()),
                    "--{id} is not translated"
                );
            }
        }
    }
}
