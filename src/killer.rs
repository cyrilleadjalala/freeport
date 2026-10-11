//! Process table, safety rules and portable process stopping (SIGTERM then
//! SIGKILL on Unix; `taskkill` then `TerminateProcess` on Windows).

use std::collections::{HashMap, HashSet};
use std::thread::sleep;
use std::time::{Duration, Instant};
use sysinfo::{
    Pid, ProcessRefreshKind, ProcessStatus, ProcessesToUpdate, Signal, System, UpdateKind,
};

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The process exited after a polite stop request (or was already gone).
    Terminated,
    /// It had to be killed.
    Killed,
}

#[derive(Debug, PartialEq, Eq)]
pub enum KillError {
    /// The system refused to signal the process (not enough privileges).
    Denied,
    /// The process is still alive after the forced kill.
    Survived,
}

#[derive(Debug, Clone)]
struct Node {
    parent: Option<u32>,
    name: String,
    cmd: String,
}

/// A snapshot of the running processes: who is whose parent, names, command lines.
#[derive(Debug, Default)]
pub struct Table {
    nodes: HashMap<u32, Node>,
}

/// Names (lower case, without `.exe`) that end the climb towards supervisors:
/// shells, terminals, multiplexers, session managers and init systems. Stopping
/// any of these would take the user's session down with the service.
const BOUNDARY: &[&str] = &[
    "sh",
    "bash",
    "zsh",
    "fish",
    "dash",
    "ksh",
    "csh",
    "tcsh",
    "nu",
    "elvish",
    "pwsh",
    "powershell",
    "cmd",
    "tmux",
    "screen",
    "sshd",
    "login",
    "su",
    "sudo",
    "systemd",
    "launchd",
    "init",
    "explorer",
    "conhost",
    "windowsterminal",
    "wt",
    "terminal",
    "iterm2",
    "gnome-terminal",
    "gnome-terminal-server",
    "konsole",
    "xterm",
    "alacritty",
    "kitty",
    "wezterm",
    "code",
    "dockerd",
    "containerd",
];

fn is_boundary(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    let n = n.strip_suffix(".exe").unwrap_or(&n);
    BOUNDARY.contains(&n)
}

impl Table {
    pub fn load() -> Table {
        let mut sys = System::new();
        sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_cmd(UpdateKind::Always),
        );
        let nodes = sys
            .processes()
            .iter()
            .map(|(pid, p)| {
                let cmd: Vec<String> = p
                    .cmd()
                    .iter()
                    .map(|s| s.to_string_lossy().into_owned())
                    .collect();
                (
                    pid.as_u32(),
                    Node {
                        parent: p.parent().map(|x| x.as_u32()),
                        name: p.name().to_string_lossy().into_owned(),
                        cmd: cmd.join(" "),
                    },
                )
            })
            .collect();
        Table { nodes }
    }

    #[cfg(test)]
    fn from_nodes(list: &[(u32, Option<u32>, &str)]) -> Table {
        Table {
            nodes: list
                .iter()
                .map(|(pid, parent, name)| {
                    (
                        *pid,
                        Node {
                            parent: *parent,
                            name: (*name).to_string(),
                            cmd: String::new(),
                        },
                    )
                })
                .collect(),
        }
    }

    pub fn parent(&self, pid: u32) -> Option<u32> {
        self.nodes.get(&pid).and_then(|n| n.parent)
    }

    pub fn name(&self, pid: u32) -> &str {
        self.nodes.get(&pid).map_or("", |n| n.name.as_str())
    }

    /// Full command line, when readable.
    pub fn cmd(&self, pid: u32) -> Option<&str> {
        self.nodes
            .get(&pid)
            .map(|n| n.cmd.as_str())
            .filter(|c| !c.is_empty())
    }

    /// Parents of `pid`, nearest first (cycle-safe).
    pub fn ancestors(&self, pid: u32) -> Vec<u32> {
        let mut out = Vec::new();
        let mut seen = HashSet::from([pid]);
        let mut cur = pid;
        while let Some(p) = self.parent(cur) {
            if !seen.insert(p) {
                break;
            }
            out.push(p);
            cur = p;
        }
        out
    }

    /// Everything below `pid`, children before their parents (leaves first).
    pub fn descendants(&self, pid: u32) -> Vec<u32> {
        let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
        for (&p, n) in &self.nodes {
            if let Some(parent) = n.parent {
                children.entry(parent).or_default().push(p);
            }
        }
        for v in children.values_mut() {
            v.sort_unstable();
        }
        fn walk(
            pid: u32,
            children: &HashMap<u32, Vec<u32>>,
            seen: &mut HashSet<u32>,
            out: &mut Vec<u32>,
        ) {
            for &c in children.get(&pid).into_iter().flatten() {
                if seen.insert(c) {
                    walk(c, children, seen, out);
                    out.push(c);
                }
            }
        }
        let mut out = Vec::new();
        walk(pid, &children, &mut HashSet::from([pid]), &mut out);
        out
    }

    /// Ancestors that look like service supervisors (nodemon, npm, a wrapper
    /// script…), nearest first. The climb stops at the first protected process
    /// or boundary (shell, terminal, init…): those are never returned.
    pub fn supervisors(&self, pid: u32, protected: &Protected) -> Vec<u32> {
        let mut out = Vec::new();
        for p in self.ancestors(pid) {
            if protected.contains(p) || is_boundary(self.name(p)) {
                break;
            }
            out.push(p);
        }
        out
    }
}

fn is_system_pid(pid: u32) -> bool {
    if cfg!(windows) { pid <= 4 } else { pid <= 1 }
}

/// Processes that must never be stopped: system processes, `freeport` itself
/// and every one of its ancestors (the shell, `npm run`, the terminal…).
#[derive(Debug, Default)]
pub struct Protected {
    pids: HashSet<u32>,
}

impl Protected {
    pub fn current(table: &Table) -> Protected {
        let me = std::process::id();
        let mut pids: HashSet<u32> = table.ancestors(me).into_iter().collect();
        pids.insert(me);
        Protected { pids }
    }

    pub fn contains(&self, pid: u32) -> bool {
        is_system_pid(pid) || self.pids.contains(&pid)
    }
}

fn is_gone(sys: &mut System, pid: Pid) -> bool {
    sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
    match sys.process(pid) {
        None => true,
        // A zombie has released its sockets; it only waits to be reaped.
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

/// Windows: ask politely first. Windowed applications close properly; console
/// applications refuse (they "can only be terminated forcefully") and we fall
/// through to `TerminateProcess` right away.
#[cfg(windows)]
fn taskkill_graceful(pid: u32) -> bool {
    use std::process::{Command, Stdio};
    Command::new("taskkill")
        .args(["/PID", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Stops the process. `force` skips the polite request.
pub fn terminate(pid: u32, force: bool, grace: Duration) -> Result<Outcome, KillError> {
    let raw = pid;
    let pid = Pid::from_u32(raw);
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
    if sys.process(pid).is_none() {
        return Ok(Outcome::Terminated); // already gone
    }

    if !force {
        #[cfg(windows)]
        if taskkill_graceful(raw) && wait_gone(&mut sys, pid, grace) {
            return Ok(Outcome::Terminated);
        }
        // `None`: signals unsupported (Windows) -> straight to kill().
        match sys.process(pid).and_then(|p| p.kill_with(Signal::Term)) {
            Some(false) => return Err(KillError::Denied),
            Some(true) if wait_gone(&mut sys, pid, grace) => return Ok(Outcome::Terminated),
            // Signal sent but ignored, or unsupported: force it.
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

    /// 1 init -> 10 bash -> 20 npm -> 30 nodemon -> 40 node (holder) -> 50, 51 -> 60
    fn sample() -> Table {
        Table::from_nodes(&[
            (1, None, "init"),
            (10, Some(1), "bash"),
            (20, Some(10), "npm"),
            (30, Some(20), "nodemon"),
            (40, Some(30), "node"),
            (50, Some(40), "worker"),
            (51, Some(40), "worker"),
            (60, Some(50), "grandchild"),
            (99, Some(10), "freeport"),
        ])
    }

    fn protected_for(me: u32, table: &Table) -> Protected {
        let mut pids: HashSet<u32> = table.ancestors(me).into_iter().collect();
        pids.insert(me);
        Protected { pids }
    }

    #[test]
    fn ancestors_nearest_first() {
        assert_eq!(sample().ancestors(40), vec![30, 20, 10, 1]);
        assert!(sample().ancestors(1).is_empty());
    }

    #[test]
    fn descendants_leaves_first() {
        assert_eq!(sample().descendants(40), vec![60, 50, 51]);
        assert!(sample().descendants(60).is_empty());
    }

    #[test]
    fn supervisors_stop_at_the_shell() {
        let t = sample();
        let p = protected_for(99, &t); // freeport runs under bash
        assert_eq!(t.supervisors(40, &p), vec![30, 20]); // nodemon, npm; never bash
    }

    #[test]
    fn supervisors_never_include_freeports_own_ancestors() {
        // freeport launched by `npm run` (20): npm and everything above it is protected.
        let t = Table::from_nodes(&[
            (1, None, "init"),
            (20, Some(1), "npm"),
            (30, Some(20), "nodemon"),
            (40, Some(30), "node"),
            (99, Some(20), "freeport"),
        ]);
        let p = protected_for(99, &t);
        assert_eq!(t.supervisors(40, &p), vec![30]);
    }

    #[test]
    fn supervisors_stop_at_system_pid_and_unknown_parent() {
        let t = Table::from_nodes(&[(1, None, "init"), (7, Some(1), "daemon")]);
        assert!(t.supervisors(7, &Protected::default()).is_empty());
        assert!(t.supervisors(999, &Protected::default()).is_empty());
    }

    #[test]
    fn cycles_do_not_loop_forever() {
        let t = Table::from_nodes(&[(5, Some(6), "a"), (6, Some(5), "b")]);
        assert_eq!(t.ancestors(5), vec![6]);
        assert_eq!(t.descendants(5), vec![6]);
    }

    #[test]
    fn boundary_names_are_case_and_extension_insensitive() {
        assert!(is_boundary("PowerShell.exe"));
        assert!(is_boundary("zsh"));
        assert!(is_boundary("WindowsTerminal.exe"));
        assert!(!is_boundary("nodemon"));
    }

    #[test]
    fn protects_system_self_and_ancestors() {
        let t = sample();
        let p = protected_for(99, &t);
        assert!(p.contains(1)); // system pid
        assert!(p.contains(99)); // me
        assert!(p.contains(10)); // parent
        assert!(!p.contains(40));
    }
}
