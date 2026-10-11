//! What `freeport` says: one `Item` per process/port, rendered as text (English
//! or French), as JSON, or not at all (`--quiet`).

use crate::finder::Proto;
use crate::i18n::Lang;
use serde::Serialize;
use std::io::{self, Write};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Free,
    WouldStop,
    Stopped,
    Killed,
    Protected,
    Denied,
    Survived,
    Docker,
    StillBusy,
    Skipped,
}

impl Status {
    /// Problems go to stderr in text mode.
    fn is_problem(self) -> bool {
        matches!(
            self,
            Status::Protected
                | Status::Denied
                | Status::Survived
                | Status::Docker
                | Status::StillBusy
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Holder,
    Supervisor,
    Child,
}

#[derive(Clone, Debug, Serialize)]
pub struct Item {
    pub port: u16,
    pub proto: Proto,
    pub status: Status,
    pub role: Role,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl Item {
    pub fn new(port: u16, proto: Proto, status: Status) -> Item {
        Item {
            port,
            proto,
            status,
            role: Role::Holder,
            pid: None,
            name: None,
            command: None,
            detail: None,
        }
    }

    pub fn process(mut self, pid: u32, name: &str) -> Item {
        self.pid = Some(pid);
        self.name = Some(name.to_string()).filter(|n| !n.is_empty());
        self
    }

    pub fn role(mut self, role: Role) -> Item {
        self.role = role;
        self
    }

    pub fn command(mut self, cmd: Option<&str>) -> Item {
        self.command = cmd.map(str::to_string).filter(|c| !c.is_empty());
        self
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Item {
        self.detail = Some(detail.into());
        self
    }
}

fn who(it: &Item) -> String {
    let name = it.name.as_deref().unwrap_or("?");
    match it.pid {
        Some(pid) => format!("{name} (pid {pid})"),
        None => name.to_string(),
    }
}

fn role_prefix(lang: Lang, role: Role) -> &'static str {
    match role {
        Role::Holder => "",
        Role::Supervisor => lang.t("supervisor ", "superviseur "),
        Role::Child => lang.t("child process ", "processus enfant "),
    }
}

/// One human-readable line (without trailing newline).
pub fn render(lang: Lang, it: &Item) -> String {
    let target = format!("port {}/{}", it.port, it.proto);
    let who = who(it);
    let role = role_prefix(lang, it.role);
    let detail = it.detail.as_deref().unwrap_or("");
    let cmd = it
        .command
        .as_deref()
        .map(|c| format!(" — {c}"))
        .unwrap_or_default();
    match it.status {
        Status::Free => lang.pick(
            format!("{target}: already free"),
            format!("{target} : déjà libre"),
        ),
        Status::WouldStop if it.role == Role::Holder => lang.pick(
            format!("{target}: held by {who}{cmd}"),
            format!("{target} : occupé par {who}{cmd}"),
        ),
        Status::WouldStop => lang.pick(
            format!("{target}: would also stop {role}{who}{cmd}"),
            format!("{target} : arrêterait aussi {role}{who}{cmd}"),
        ),
        Status::Stopped => lang.pick(
            format!("{target}: {role}{who} stopped"),
            format!("{target} : {role}{who} arrêté"),
        ),
        Status::Killed => lang.pick(
            format!("{target}: {role}{who} killed"),
            format!("{target} : {role}{who} tué de force"),
        ),
        Status::Protected => lang.pick(
            format!("{target}: {role}{who} is protected, skipped"),
            format!("{target} : {role}{who} est protégé, ignoré"),
        ),
        Status::Denied => lang.pick(
            format!("{target}: permission denied for {role}{who} (retry with sudo / as administrator)"),
            format!("{target} : permission refusée pour {role}{who} (relancez avec sudo / en administrateur)"),
        ),
        Status::Survived => lang.pick(
            format!("{target}: {role}{who} refuses to stop"),
            format!("{target} : {role}{who} refuse de s'arrêter"),
        ),
        Status::Docker => lang.pick(
            format!("{target}: held by Docker ({who}), not stopped. {detail}"),
            format!("{target} : occupé via Docker ({who}), non arrêté. {detail}"),
        ),
        Status::StillBusy => lang.pick(
            format!("{target}: still held by {detail}"),
            format!("{target} : toujours occupé par {detail}"),
        ),
        Status::Skipped => lang.pick(
            format!("{target}: {role}{who} skipped"),
            format!("{target} : {role}{who} ignoré"),
        ),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Text,
    Json,
    Quiet,
}

#[derive(Default)]
pub struct Summary {
    pub checked: usize,
    pub freed: usize,
    pub already_free: usize,
    pub failed: usize,
}

pub struct Reporter {
    mode: Mode,
    lang: Lang,
    items: Vec<Item>,
}

/// Writes ignoring errors: a closed pipe (`freeport -l | head`) must not panic.
fn out(line: &str) {
    let _ = writeln!(io::stdout().lock(), "{line}");
}

fn err(line: &str) {
    let _ = writeln!(io::stderr().lock(), "{line}");
}

impl Reporter {
    pub fn new(mode: Mode, lang: Lang) -> Reporter {
        Reporter {
            mode,
            lang,
            items: Vec::new(),
        }
    }

    pub fn emit(&mut self, it: Item) {
        let line = render(self.lang, &it);
        match self.mode {
            Mode::Json => {}
            Mode::Quiet => {
                if it.status.is_problem() {
                    err(&line);
                }
            }
            Mode::Text => {
                if it.status.is_problem() {
                    err(&line);
                } else {
                    out(&line);
                }
            }
        }
        self.items.push(it);
    }

    /// Final line for range runs (text mode only).
    pub fn summary(&self, s: &Summary) {
        if self.mode == Mode::Text {
            out(&self.lang.pick(
                format!(
                    "{} ports checked: {} freed, {} already free, {} failed",
                    s.checked, s.freed, s.already_free, s.failed
                ),
                format!(
                    "{} ports vérifiés : {} libérés, {} déjà libres, {} en échec",
                    s.checked, s.freed, s.already_free, s.failed
                ),
            ));
        }
    }

    pub fn finish(self) {
        if self.mode == Mode::Json {
            #[derive(Serialize)]
            struct Doc<'a> {
                results: &'a [Item],
            }
            if let Ok(s) = serde_json::to_string(&Doc {
                results: &self.items,
            }) {
                out(&s);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(status: Status, role: Role) -> Item {
        Item::new(3000, Proto::Tcp, status)
            .process(42, "node")
            .role(role)
            .command(Some("node server.js"))
            .detail("detail text")
    }

    const ALL: [Status; 10] = [
        Status::Free,
        Status::WouldStop,
        Status::Stopped,
        Status::Killed,
        Status::Protected,
        Status::Denied,
        Status::Survived,
        Status::Docker,
        Status::StillBusy,
        Status::Skipped,
    ];

    #[test]
    fn every_status_is_rendered_in_both_languages_and_they_differ() {
        for status in ALL {
            for role in [Role::Holder, Role::Supervisor, Role::Child] {
                let it = sample(status, role);
                let (en, fr) = (render(Lang::En, &it), render(Lang::Fr, &it));
                assert!(en.contains("3000/tcp"), "{status:?}: {en}");
                assert!(fr.contains("3000/tcp"), "{status:?}: {fr}");
                assert_ne!(en, fr, "{status:?} is not translated");
            }
        }
    }

    #[test]
    fn english_wording() {
        let it = sample(Status::Stopped, Role::Holder);
        assert_eq!(
            render(Lang::En, &it),
            "port 3000/tcp: node (pid 42) stopped"
        );
        let it = sample(Status::WouldStop, Role::Supervisor);
        assert_eq!(
            render(Lang::En, &it),
            "port 3000/tcp: would also stop supervisor node (pid 42) — node server.js"
        );
        let it = Item::new(80, Proto::Udp, Status::Free);
        assert_eq!(render(Lang::En, &it), "port 80/udp: already free");
        assert_eq!(render(Lang::Fr, &it), "port 80/udp : déjà libre");
    }

    #[test]
    fn json_shape_is_stable() {
        let it = sample(Status::WouldStop, Role::Holder);
        let v = serde_json::to_value(&it).unwrap();
        assert_eq!(v["port"], 3000);
        assert_eq!(v["proto"], "tcp");
        assert_eq!(v["status"], "would_stop");
        assert_eq!(v["role"], "holder");
        assert_eq!(v["pid"], 42);
        let free = serde_json::to_value(Item::new(1, Proto::Udp, Status::Free)).unwrap();
        assert!(free.get("pid").is_none() && free.get("detail").is_none());
    }

    #[test]
    fn empty_process_name_is_omitted() {
        assert!(
            Item::new(1, Proto::Tcp, Status::Free)
                .process(5, "")
                .name
                .is_none()
        );
    }
}
