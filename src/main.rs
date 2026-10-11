mod finder;
mod killer;

use clap::Parser;
use finder::{Holder, Proto};
use killer::{KillError, Outcome};
use std::io::{self, Write};
use std::process::ExitCode;
use std::time::Duration;

/// Libère un ou plusieurs ports en arrêtant les processus qui les occupent.
#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// Port(s) à libérer
    #[arg(required = true, value_parser = clap::value_parser!(u16).range(1..))]
    ports: Vec<u16>,

    /// Affiche les processus concernés sans rien arrêter
    #[arg(short = 'n', long)]
    dry_run: bool,

    /// Arrêt immédiat et forcé (SIGKILL), sans arrêt propre préalable
    #[arg(short, long)]
    force: bool,

    /// Cible UDP au lieu de TCP
    #[arg(short, long)]
    udp: bool,

    /// Délai (secondes) accordé à l'arrêt propre avant de forcer
    #[arg(short, long, default_value_t = 3)]
    timeout: u64,
}

fn describe(h: &Holder) -> String {
    format!(
        "{} (pid {})",
        if h.name.is_empty() { "?" } else { &h.name },
        h.pid
    )
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let proto = if cli.udp { Proto::Udp } else { Proto::Tcp };
    let grace = Duration::from_secs(cli.timeout);
    let (me, parent) = killer::current_and_parent();
    let mut ok = true;
    let mut out = io::stdout().lock();

    let mut ports = cli.ports.clone();
    ports.sort_unstable();
    ports.dedup();
    for port in ports {
        let holders = match finder::find(port, proto) {
            Ok(h) => h,
            Err(e) => {
                eprintln!("port {port}/{proto} : recherche impossible ({e})");
                ok = false;
                continue;
            }
        };
        if holders.is_empty() {
            let _ = writeln!(out, "port {port}/{proto} : déjà libre");
            continue;
        }

        for h in &holders {
            if killer::is_protected(h.pid, me, parent) {
                eprintln!("port {port}/{proto} : {} est protégé, ignoré", describe(h));
                ok = false;
                continue;
            }
            if cli.dry_run {
                let cmd = killer::command_line(h.pid).unwrap_or_else(|| h.path.clone());
                let _ = writeln!(
                    out,
                    "port {port}/{proto} : occupé par {} — {cmd}",
                    describe(h)
                );
                continue;
            }
            match killer::terminate(h.pid, cli.force, grace) {
                Ok(Outcome::Terminated) => {
                    let _ = writeln!(out, "port {port}/{proto} : {} arrêté", describe(h));
                }
                Ok(Outcome::Killed) => {
                    let _ = writeln!(out, "port {port}/{proto} : {} tué de force", describe(h));
                }
                Err(KillError::Denied) => {
                    eprintln!(
                        "port {port}/{proto} : permission refusée pour {} \
                         (relancez avec sudo / en administrateur)",
                        describe(h)
                    );
                    ok = false;
                }
                Err(KillError::Survived) => {
                    eprintln!("port {port}/{proto} : {} refuse de s'arrêter", describe(h));
                    ok = false;
                }
            }
        }

        // Vérification : un superviseur ou un autre processus a pu reprendre le port.
        if !cli.dry_run
            && let Ok(left) = finder::find(port, proto)
        {
            let left: Vec<_> = left.iter().filter(|h| h.pid != me).collect();
            if !left.is_empty() {
                let names: Vec<_> = left.iter().map(|h| describe(h)).collect();
                eprintln!(
                    "port {port}/{proto} : toujours occupé par {}",
                    names.join(", ")
                );
                ok = false;
            }
        }
    }
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
