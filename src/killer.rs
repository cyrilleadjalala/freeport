//! Arrêt d'un processus de façon portable (SIGTERM puis SIGKILL sous Unix,
//! `TerminateProcess` sous Windows).

use std::thread::sleep;
use std::time::{Duration, Instant};
use sysinfo::{Pid, ProcessStatus, ProcessesToUpdate, Signal, System};

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Le processus s'est arrêté après une demande d'arrêt propre.
    Terminated,
    /// Il a fallu le tuer de force.
    Killed,
}

#[derive(Debug, PartialEq, Eq)]
pub enum KillError {
    /// Le système a refusé d'envoyer le signal (droits insuffisants).
    Denied,
    /// Le processus est toujours vivant après SIGKILL.
    Survived,
}

/// Processus qu'on ne doit jamais viser (init/système, nous-mêmes, notre parent).
pub fn is_protected(pid: u32, me: u32, parent: Option<u32>) -> bool {
    let system_pid = if cfg!(windows) { pid <= 4 } else { pid <= 1 };
    system_pid || pid == me || Some(pid) == parent
}

pub fn current_and_parent() -> (u32, Option<u32>) {
    let me = std::process::id();
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::Some(&[Pid::from_u32(me)]), true);
    let parent = sys
        .process(Pid::from_u32(me))
        .and_then(|p| p.parent())
        .map(|p| p.as_u32());
    (me, parent)
}

fn is_gone(sys: &mut System, pid: Pid) -> bool {
    sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
    match sys.process(pid) {
        None => true,
        // Un zombie a libéré ses sockets ; il attend seulement d'être récolté.
        Some(p) => p.status() == ProcessStatus::Zombie,
    }
}

fn wait_gone(sys: &mut System, pid: Pid, timeout: Duration) -> bool {
    let start = Instant::now();
    loop {
        if is_gone(sys, pid) {
            return true;
        }
        if start.elapsed() >= timeout {
            return false;
        }
        sleep(Duration::from_millis(50));
    }
}

/// Ligne de commande complète du processus, si lisible.
pub fn command_line(pid: u32) -> Option<String> {
    let pid = Pid::from_u32(pid);
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
    let p = sys.process(pid)?;
    let cmd: Vec<String> = p
        .cmd()
        .iter()
        .map(|s| s.to_string_lossy().into_owned())
        .collect();
    (!cmd.is_empty()).then(|| cmd.join(" "))
}

/// Arrête le processus. `force` saute l'arrêt propre.
pub fn terminate(pid: u32, force: bool, grace: Duration) -> Result<Outcome, KillError> {
    let pid = Pid::from_u32(pid);
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
    let Some(proc_) = sys.process(pid) else {
        return Ok(Outcome::Terminated); // déjà parti entre-temps
    };

    if !force {
        // `None` : signal non supporté (Windows) -> on passe directement à kill().
        match proc_.kill_with(Signal::Term) {
            Some(false) => return Err(KillError::Denied),
            Some(true) if wait_gone(&mut sys, pid, grace) => return Ok(Outcome::Terminated),
            // Signal envoyé mais ignoré, ou non supporté (Windows) : on force.
            Some(true) | None => {}
        }
    }

    sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
    let Some(proc_) = sys.process(pid) else {
        return Ok(Outcome::Terminated);
    };
    if !proc_.kill() {
        return Err(KillError::Denied);
    }
    if wait_gone(&mut sys, pid, Duration::from_secs(3)) {
        Ok(Outcome::Killed)
    } else {
        Err(KillError::Survived)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protects_system_self_and_parent() {
        assert!(is_protected(1, 500, None));
        assert!(is_protected(500, 500, None));
        assert!(is_protected(400, 500, Some(400)));
        assert!(!is_protected(600, 500, Some(400)));
    }
}
