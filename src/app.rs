//! The program itself: the list view and the "free these ports" run.

use crate::cli::Cli;
use crate::docker;
use crate::finder::{self, Holder, Proto};
use crate::i18n::Lang;
use crate::killer::{self, KillError, Outcome, Protected, Table};
use crate::ports::{self, PortSpec};
use crate::prompt;
use crate::report::{Item, Mode, Reporter, Role, Status, Summary};
use clap::CommandFactory;
use listeners::Listener;
use serde::Serialize;
use std::collections::{BTreeSet, HashSet};
use std::io::{self, IsTerminal, Write};
use std::process::ExitCode;
use std::thread::sleep;
use std::time::{Duration, Instant};

pub fn run(cli: Cli, lang: Lang) -> ExitCode {
    if let Some(shell) = cli.completions {
        clap_complete::generate(shell, &mut Cli::command(), "freeport", &mut io::stdout());
        return ExitCode::SUCCESS;
    }
    if cli.interactive && !(io::stdin().is_terminal() && io::stdout().is_terminal()) {
        eprintln!(
            "{}",
            lang.t(
                "error: --interactive needs a terminal (standard input and output)",
                "erreur : --interactive nécessite un terminal (entrée et sortie standard)"
            )
        );
        return ExitCode::from(2);
    }
    let proto = if cli.udp { Proto::Udp } else { Proto::Tcp };
    let all = match finder::snapshot() {
        Ok(a) => a,
        Err(e) => {
            eprintln!(
                "{}",
                lang.pick(
                    format!("error: cannot list sockets ({e})"),
                    format!("erreur : impossible de lister les sockets ({e})")
                )
            );
            return ExitCode::FAILURE;
        }
    };
    let table = Table::load();
    if cli.list || cli.ports.is_empty() {
        list(&cli, &all, &table, proto, lang)
    } else {
        free(&cli, &all, &table, proto, lang)
    }
}

// ---------------------------------------------------------------- list view

#[derive(Serialize)]
struct Row {
    proto: Proto,
    port: u16,
    pid: u32,
    name: String,
    command: Option<String>,
    addresses: Vec<String>,
}

fn list(cli: &Cli, all: &[Listener], table: &Table, proto: Proto, lang: Lang) -> ExitCode {
    let wanted: Option<BTreeSet<u16>> = (!cli.ports.is_empty()).then(|| {
        ports::expand(&cli.ports)
            .into_iter()
            .map(|(p, _)| p)
            .collect()
    });
    let rows: Vec<Row> = finder::listening(all, proto, cli.addr)
        .into_iter()
        .filter(|e| wanted.as_ref().is_none_or(|w| w.contains(&e.port)))
        .map(|e| {
            let mut addresses: Vec<String> = e.addrs.iter().map(|a| a.ip().to_string()).collect();
            addresses.sort();
            addresses.dedup();
            Row {
                proto: e.proto,
                port: e.port,
                pid: e.pid,
                command: table
                    .cmd(e.pid)
                    .map(str::to_string)
                    .or_else(|| Some(e.path.clone()).filter(|p| !p.is_empty())),
                name: e.name,
                addresses,
            }
        })
        .collect();

    let mut o = io::stdout().lock();
    if cli.json {
        #[derive(Serialize)]
        struct Doc<'a> {
            listeners: &'a [Row],
        }
        if let Ok(s) = serde_json::to_string(&Doc { listeners: &rows }) {
            let _ = writeln!(o, "{s}");
        }
        return ExitCode::SUCCESS;
    }
    if rows.is_empty() {
        let _ = writeln!(
            o,
            "{}",
            lang.t("No listening ports found", "Aucun port en écoute")
        );
        return ExitCode::SUCCESS;
    }
    let head = [
        "PROTO",
        "PORT",
        "PID",
        lang.t("PROCESS", "PROCESSUS"),
        lang.t("ADDRESS", "ADRESSE"),
        lang.t("COMMAND", "COMMANDE"),
    ];
    let cells: Vec<[String; 6]> = rows
        .iter()
        .map(|r| {
            [
                r.proto.to_string(),
                r.port.to_string(),
                r.pid.to_string(),
                r.name.clone(),
                r.addresses.join(","),
                r.command.clone().unwrap_or_default(),
            ]
        })
        .collect();
    let mut width = head.map(str::len);
    for c in &cells {
        for (w, s) in width.iter_mut().zip(c) {
            *w = (*w).max(s.chars().count());
        }
    }
    let line = |c: [&str; 6]| {
        let mut s = String::new();
        for (i, v) in c.iter().enumerate() {
            if i == 5 {
                s.push_str(v);
            } else {
                s.push_str(&format!("{v:<w$}  ", w = width[i]));
            }
        }
        s.trim_end().to_string()
    };
    let _ = writeln!(o, "{}", line(head));
    for c in &cells {
        let _ = writeln!(o, "{}", line([&c[0], &c[1], &c[2], &c[3], &c[4], &c[5]]));
    }
    ExitCode::SUCCESS
}

// ----------------------------------------------------------------- free run

struct Target {
    pid: u32,
    name: String,
    role: Role,
}

/// What gets stopped, in order: supervisors first (so nothing restarts the
/// service behind our back), then the holder, then its children (leaves first).
fn plan(table: &Table, protected: &Protected, h: &Holder, tree: bool) -> Vec<Target> {
    let node = |pid: u32, role: Role| Target {
        pid,
        name: table.name(pid).to_string(),
        role,
    };
    let mut v = Vec::new();
    if tree {
        v.extend(
            table
                .supervisors(h.pid, protected)
                .into_iter()
                .map(|p| node(p, Role::Supervisor)),
        );
    }
    v.push(Target {
        pid: h.pid,
        name: h.name.clone(),
        role: Role::Holder,
    });
    if tree {
        v.extend(
            table
                .descendants(h.pid)
                .into_iter()
                .filter(|p| !protected.contains(*p))
                .map(|p| node(p, Role::Child)),
        );
    }
    v
}

fn describe(h: &Holder) -> String {
    let name = if h.name.is_empty() { "?" } else { &h.name };
    format!("{name} (pid {})", h.pid)
}

fn docker_hint(lang: Lang, names: &[String]) -> String {
    if names.is_empty() {
        return lang
            .t(
                "Stop the container (docker ps, docker stop <name>) or pass --allow-docker.",
                "Arrêtez le conteneur (docker ps, docker stop <nom>) ou ajoutez --allow-docker.",
            )
            .to_string();
    }
    lang.pick(
        format!(
            "Published by container {}: docker stop {}. Or pass --allow-docker.",
            names.join(", "),
            names.join(" ")
        ),
        format!(
            "Publié par le conteneur {} : docker stop {}. Ou ajoutez --allow-docker.",
            names.join(", "),
            names.join(" ")
        ),
    )
}

fn remaining(
    port: u16,
    proto: Proto,
    addr: Option<std::net::IpAddr>,
    ignore: &HashSet<u32>,
    protected: &Protected,
) -> Vec<Holder> {
    let Ok(all) = finder::snapshot() else {
        return Vec::new();
    };
    finder::select(&all, port, proto, addr)
        .into_iter()
        .filter(|h| !ignore.contains(&h.pid) && !protected.contains(h.pid))
        .collect()
}

/// Is the port really free? The kernel may release the socket a moment after
/// the process is gone, so wait up to a second; and once it is free, look
/// again shortly after: a supervisor may be about to grab it back.
fn verify(
    port: u16,
    proto: Proto,
    addr: Option<std::net::IpAddr>,
    ignore: &HashSet<u32>,
    protected: &Protected,
) -> Vec<Holder> {
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        let left = remaining(port, proto, addr, ignore, protected);
        if left.is_empty() {
            break;
        }
        if Instant::now() >= deadline {
            return left;
        }
        sleep(Duration::from_millis(100));
    }
    sleep(Duration::from_millis(300));
    remaining(port, proto, addr, ignore, protected)
}

fn free(cli: &Cli, all: &[Listener], table: &Table, proto: Proto, lang: Lang) -> ExitCode {
    let mode = if cli.json {
        Mode::Json
    } else if cli.quiet {
        Mode::Quiet
    } else {
        Mode::Text
    };
    let protected = Protected::current(table);
    let grace = Duration::from_secs(cli.timeout);
    let has_range = cli.ports.iter().any(|s| matches!(s, PortSpec::Range(..)));
    let mut rep = Reporter::new(mode, lang);
    let mut summary = Summary::default();
    let mut ok = true;

    for (port, explicit) in ports::expand(&cli.ports) {
        summary.checked += 1;
        let holders = finder::select(all, port, proto, cli.addr);
        if holders.is_empty() {
            summary.already_free += 1;
            if explicit {
                rep.emit(Item::new(port, proto, Status::Free));
            }
            continue;
        }

        // Holders already reported (blocked, declined…): not blamed again below.
        let mut reported: HashSet<u32> = HashSet::new();
        let mut touched = false;
        let mut port_ok = true;

        for h in &holders {
            let item = |status| Item::new(port, proto, status).process(h.pid, &h.name);
            if protected.contains(h.pid) {
                reported.insert(h.pid);
                port_ok = false;
                rep.emit(item(Status::Protected));
                continue;
            }
            if docker::is_docker_holder(&h.name) && !cli.allow_docker {
                reported.insert(h.pid);
                port_ok = false;
                let hint = docker_hint(lang, &docker::containers(port, proto));
                rep.emit(item(Status::Docker).detail(hint));
                continue;
            }
            let plan = plan(table, &protected, h, cli.tree);

            if cli.dry_run {
                for t in &plan {
                    let cmd = table
                        .cmd(t.pid)
                        .or((t.role == Role::Holder).then_some(h.path.as_str()));
                    rep.emit(
                        Item::new(port, proto, Status::WouldStop)
                            .process(t.pid, &t.name)
                            .role(t.role)
                            .command(cmd),
                    );
                }
                continue;
            }

            if cli.interactive {
                let related = plan.len() - 1;
                let question = lang.pick(
                    format!(
                        "Stop {} on port {port}/{proto}{}? [y/N] ",
                        describe(h),
                        if related > 0 {
                            format!(" (+{related} related)")
                        } else {
                            String::new()
                        }
                    ),
                    format!(
                        "Arrêter {} sur le port {port}/{proto}{} ? [o/N] ",
                        describe(h),
                        if related > 0 {
                            format!(" (+{related} liés)")
                        } else {
                            String::new()
                        }
                    ),
                );
                let yes = prompt::confirm(&question, &mut io::stdin().lock(), &mut io::stderr())
                    .unwrap_or(false);
                if !yes {
                    reported.insert(h.pid);
                    port_ok = false;
                    rep.emit(item(Status::Skipped));
                    continue;
                }
            }

            for t in &plan {
                let it = |status| {
                    Item::new(port, proto, status)
                        .process(t.pid, &t.name)
                        .role(t.role)
                };
                match killer::terminate(t.pid, cli.force, grace) {
                    Ok(Outcome::Terminated) => {
                        touched = true;
                        rep.emit(it(Status::Stopped));
                    }
                    Ok(Outcome::Killed) => {
                        touched = true;
                        rep.emit(it(Status::Killed));
                    }
                    Err(KillError::Denied) => {
                        reported.insert(t.pid);
                        port_ok = false;
                        rep.emit(it(Status::Denied));
                    }
                    Err(KillError::Survived) => {
                        reported.insert(t.pid);
                        port_ok = false;
                        rep.emit(it(Status::Survived));
                    }
                }
            }
        }

        if touched {
            let left = verify(port, proto, cli.addr, &reported, &protected);
            if !left.is_empty() {
                port_ok = false;
                let names: Vec<String> = left.iter().map(describe).collect();
                rep.emit(Item::new(port, proto, Status::StillBusy).detail(names.join(", ")));
            }
        }
        if port_ok {
            summary.freed += 1;
        } else {
            summary.failed += 1;
        }
        ok &= port_ok;
    }

    if has_range && !cli.dry_run {
        rep.summary(&summary);
    }
    rep.finish();
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
