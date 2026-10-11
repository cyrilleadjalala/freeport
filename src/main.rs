mod finder;

use clap::Parser;
use finder::{Holder, Proto};
use std::process::ExitCode;
use std::thread::sleep;
use std::time::{Duration, Instant};

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

    /// Envoie SIGKILL directement, sans tenter SIGTERM d'abord
    #[arg(short, long)]
    force: bool,

    /// Cible UDP au lieu de TCP
    #[arg(short, long)]
    udp: bool,

    /// Délai (secondes) accordé après SIGTERM avant SIGKILL
    #[arg(short, long, default_value_t = 3)]
    timeout: u64,
}

fn alive(pid: u32) -> bool {
    // SAFETY: le signal 0 ne fait que tester l'existence du processus.
    let r = unsafe { libc::kill(pid as i32, 0) };
    r == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

fn signal(pid: u32, sig: i32) -> Result<(), std::io::Error> {
    // SAFETY: appel direct à kill(2) avec un pid et un signal valides.
    if unsafe { libc::kill(pid as i32, sig) } == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

fn wait_exit(pid: u32, timeout: Duration) -> bool {
    let start = Instant::now();
    while alive(pid) {
        if start.elapsed() >= timeout {
            return false;
        }
        sleep(Duration::from_millis(50));
    }
    true
}

/// Arrête un processus ; renvoie `true` s'il a bien disparu.
fn stop(h: &Holder, force: bool, timeout: Duration) -> bool {
    if !force {
        if let Err(e) = signal(h.pid, libc::SIGTERM) {
            eprintln!("  ✗ SIGTERM sur {} : {e}", h.pid);
            return false;
        }
        if wait_exit(h.pid, timeout) {
            return true;
        }
        eprintln!("  ! {} ignore SIGTERM, envoi de SIGKILL", h.pid);
    }
    match signal(h.pid, libc::SIGKILL) {
        Ok(()) => wait_exit(h.pid, Duration::from_secs(2)),
        Err(e) => {
            eprintln!("  ✗ SIGKILL sur {} : {e}", h.pid);
            false
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let proto = if cli.udp { Proto::Udp } else { Proto::Tcp };
    let me = std::process::id();
    let mut ok = true;

    for port in cli.ports {
        let holders = match finder::find(port, proto) {
            Ok(h) => h,
            Err(e) => {
                eprintln!("port {port}/{proto} : recherche impossible ({e})");
                ok = false;
                continue;
            }
        };
        let holders: Vec<_> = holders.into_iter().filter(|h| h.pid != me).collect();
        if holders.is_empty() {
            println!("port {port}/{proto} : déjà libre");
            continue;
        }
        for h in holders {
            let cmd = if h.cmdline.is_empty() { &h.name } else { &h.cmdline };
            if cli.dry_run {
                println!("port {port}/{proto} : occupé par {} (pid {}) — {cmd}", h.name, h.pid);
            } else if stop(&h, cli.force, Duration::from_secs(cli.timeout)) {
                println!("port {port}/{proto} : {} (pid {}) arrêté", h.name, h.pid);
            } else {
                eprintln!("port {port}/{proto} : échec pour {} (pid {})", h.name, h.pid);
                ok = false;
            }
        }
    }
    if ok { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}
