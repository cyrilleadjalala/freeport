use std::io::{BufRead, BufReader};
use std::net::{TcpListener, TcpStream};
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

/// `freeport`, in English unless a test says otherwise.
fn freeport() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_freeport"));
    c.env("FREEPORT_LANG", "en");
    c
}

fn example(name: &str) -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_BIN_EXE_freeport"));
    p.pop();
    p.push("examples");
    p.push(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    p
}

fn run(args: &[&str]) -> Output {
    freeport().args(args).output().unwrap()
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

/// Test process, killed at the end even if an assertion fails.
struct Guard(Child);

impl Deref for Guard {
    type Target = Child;
    fn deref(&self) -> &Child {
        &self.0
    }
}

impl DerefMut for Guard {
    fn deref_mut(&mut self) -> &mut Child {
        &mut self.0
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// A process holding a TCP port and a UDP port.
fn spawn_holder(extra: &[&str]) -> (Guard, u16, u16) {
    let mut child = Command::new(example("hold_port"))
        .arg("--marker-for-tests")
        .args(extra)
        .stdout(Stdio::piped())
        .spawn()
        .expect("example hold_port not found (cargo test builds it)");
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let mut it = line.split_whitespace().map(|s| s.parse::<u16>().unwrap());
    (Guard(child), it.next().unwrap(), it.next().unwrap())
}

fn wait_exit(child: &mut Child) -> bool {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        if child.try_wait().unwrap().is_some() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn is_held(port: u16) -> bool {
    TcpStream::connect(("127.0.0.1", port)).is_ok()
}

fn wait_until(what: &str, mut cond: impl FnMut() -> bool) {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        if cond() {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("timed out waiting for: {what}");
}

#[test]
fn dry_run_reports_without_killing() {
    let (mut child, port, _) = spawn_holder(&[]);
    let out = run(&["-n", &port.to_string()]);
    assert!(out.status.success());
    let s = stdout(&out);
    assert!(s.contains("held by"), "{s}");
    // The simulation shows the full command line, arguments included.
    assert!(s.contains("--marker-for-tests"), "{s}");
    assert!(child.try_wait().unwrap().is_none(), "must not be killed");
}

#[test]
fn frees_tcp_port_then_reports_already_free() {
    let (mut child, port, _) = spawn_holder(&[]);
    let out = run(&[&port.to_string()]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("stopped"), "{}", stdout(&out));
    assert!(wait_exit(&mut child), "the process should have stopped");

    let again = run(&[&port.to_string()]);
    assert!(again.status.success());
    assert!(stdout(&again).contains("already free"));
}

#[test]
fn force_and_udp() {
    let (mut child, _, uport) = spawn_holder(&[]);
    let out = run(&["-f", "-u", &uport.to_string()]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(wait_exit(&mut child));
}

#[test]
fn rejects_invalid_ports() {
    for bad in ["0", "70000", "abc", "10-5", "1-65535"] {
        let out = run(&[bad]);
        assert_eq!(out.status.code(), Some(2), "{bad}");
    }
}

#[test]
fn version_uses_command_name() {
    let v = stdout(&run(&["--version"]));
    assert!(v.starts_with("freeport "), "{v}");
}

#[test]
fn list_shows_the_holder_with_or_without_flag() {
    let (_child, port, _) = spawn_holder(&[]);
    for args in [vec!["--list"], vec![]] {
        let out = run(&args);
        assert!(out.status.success());
        let s = stdout(&out);
        let row = s
            .lines()
            .find(|l| l.split_whitespace().nth(1) == Some(&port.to_string()))
            .unwrap_or_else(|| panic!("port {port} missing from:\n{s}"));
        assert!(row.contains("hold_port"), "{row}");
        assert!(s.lines().next().unwrap().starts_with("PROTO"), "{s}");
    }
    // A port filter narrows the list down to that port.
    let only = stdout(&run(&["--list", &port.to_string()]));
    assert_eq!(only.lines().count(), 2, "{only}");
}

#[test]
fn list_json_is_parsable() {
    let (_child, port, uport) = spawn_holder(&[]);
    let v: serde_json::Value =
        serde_json::from_str(&stdout(&run(&["--list", "--json"]))).expect("valid JSON");
    let rows = v["listeners"].as_array().unwrap();
    let row = rows.iter().find(|r| r["port"] == port).expect("tcp row");
    assert_eq!(row["proto"], "tcp");
    assert!(row["name"].as_str().unwrap().contains("hold_port"));
    assert!(
        row["addresses"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a == "127.0.0.1")
    );
    let v: serde_json::Value =
        serde_json::from_str(&stdout(&run(&["--list", "--json", "--udp"]))).unwrap();
    assert!(
        v["listeners"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["port"] == uport)
    );
}

#[test]
fn json_results_describe_each_port() {
    let (mut child, port, _) = spawn_holder(&[]);
    let spare = free_port();
    let v: serde_json::Value = serde_json::from_str(&stdout(&run(&[
        "--json",
        "-n",
        &port.to_string(),
        &spare.to_string(),
    ])))
    .unwrap();
    let results = v["results"].as_array().unwrap();
    let busy = results.iter().find(|r| r["port"] == port).unwrap();
    assert_eq!(busy["status"], "would_stop");
    assert_eq!(busy["role"], "holder");
    assert!(busy["pid"].is_u64());
    let free = results.iter().find(|r| r["port"] == spare).unwrap();
    assert_eq!(free["status"], "free");
    assert!(child.try_wait().unwrap().is_none());
}

#[test]
fn quiet_prints_nothing_on_stdout() {
    let (mut child, port, _) = spawn_holder(&[]);
    let out = run(&["-q", &port.to_string()]);
    assert!(out.status.success());
    assert!(stdout(&out).is_empty(), "{}", stdout(&out));
    assert!(wait_exit(&mut child));
}

/// `n` consecutive free ports, below the range operating systems hand out
/// for ephemeral ports (32768+ on Linux, 49152+ on macOS and Windows). The
/// other tests run in parallel and bind ephemeral ports: a range test that
/// looked at the ports next to one of theirs would stop their processes too.
fn free_window(n: u16) -> u16 {
    (20000..30000u16)
        .step_by(37)
        .find(|&start| {
            let held: Vec<_> = (start..start + n)
                .map(|p| TcpListener::bind(("127.0.0.1", p)))
                .collect();
            held.iter().all(Result::is_ok)
        })
        .expect("no free window of ports")
}

#[test]
fn range_stops_the_holder_and_summarises() {
    let start = free_window(5);
    let port = start + 2;
    let (mut child, _, _) = spawn_holder(&["--port", &port.to_string()]);
    let out = run(&[&format!("{start}-{}", start + 4)]);
    assert!(out.status.success(), "{}", stderr(&out));
    let s = stdout(&out);
    assert!(
        s.contains(&format!("port {port}/tcp")) && s.contains("stopped"),
        "{s}"
    );
    // Free ports of a range stay silent; one summary line closes the run.
    assert!(!s.contains("already free\n"), "{s}");
    assert!(
        s.contains("5 ports checked: 1 freed, 4 already free, 0 failed"),
        "{s}"
    );
    assert!(wait_exit(&mut child));
}

#[test]
fn addr_filter_only_targets_matching_sockets() {
    let (mut child, port, _) = spawn_holder(&[]);
    let miss = run(&["--addr", "192.0.2.1", &port.to_string()]);
    assert!(miss.status.success());
    assert!(stdout(&miss).contains("already free"), "{}", stdout(&miss));
    assert!(child.try_wait().unwrap().is_none());
    let hit = run(&["--addr", "127.0.0.1", &port.to_string()]);
    assert!(hit.status.success(), "{}", stderr(&hit));
    assert!(wait_exit(&mut child));
}

#[test]
fn interactive_needs_a_terminal() {
    let (mut child, port, _) = spawn_holder(&[]);
    let out = run(&["-i", &port.to_string()]);
    assert_eq!(out.status.code(), Some(2));
    assert!(
        stderr(&out).contains("needs a terminal"),
        "{}",
        stderr(&out)
    );
    assert!(child.try_wait().unwrap().is_none());
}

#[test]
fn completions_script_mentions_the_command() {
    for shell in ["bash", "zsh", "fish", "powershell", "elvish"] {
        let out = run(&["--completions", shell]);
        assert!(out.status.success(), "{shell}");
        assert!(stdout(&out).contains("freeport"), "{shell}");
    }
    // `--completions` stands alone.
    assert_eq!(
        run(&["--completions", "bash", "3000"]).status.code(),
        Some(2)
    );
}

#[test]
fn language_follows_freeport_lang() {
    let (_child, port, _) = spawn_holder(&[]);
    let fr = freeport()
        .env("FREEPORT_LANG", "fr")
        .args(["-n", &port.to_string()])
        .output()
        .unwrap();
    assert!(stdout(&fr).contains("occupé par"), "{}", stdout(&fr));
    let en = run(&["-n", &port.to_string()]);
    assert!(stdout(&en).contains("held by"), "{}", stdout(&en));
    let help_fr = freeport()
        .env("FREEPORT_LANG", "fr")
        .arg("--help")
        .output()
        .unwrap();
    assert!(
        stdout(&help_fr).contains("Utilisation"),
        "{}",
        stdout(&help_fr)
    );
    assert!(stdout(&run(&["--help"])).contains("Usage"));
}

/// A supervisor (like nodemon) restarts the service as soon as it dies.
fn spawn_supervised() -> (Guard, u16) {
    let port = free_port();
    let mut chain = Command::new(example("spawn_chain"))
        .args([
            example("hold_port").to_str().unwrap(),
            &port.to_string(),
            "30",
        ])
        .stdout(Stdio::piped())
        .spawn()
        .expect("example spawn_chain not found");
    let mut line = String::new();
    BufReader::new(chain.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    wait_until("supervised service listening", || is_held(port));
    (Guard(chain), port)
}

#[test]
fn without_tree_a_supervisor_takes_the_port_back() {
    let (_chain, port) = spawn_supervised();
    let out = run(&[&port.to_string()]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "{}{}",
        stdout(&out),
        stderr(&out)
    );
    assert!(stderr(&out).contains("still held by"), "{}", stderr(&out));
}

#[test]
fn tree_stops_the_supervisor_so_the_port_stays_free() {
    let (mut chain, port) = spawn_supervised();
    let preview = run(&["--tree", "-n", &port.to_string()]);
    let s = stdout(&preview);
    assert!(
        s.contains("held by") && s.contains("would also stop supervisor"),
        "{s}"
    );
    assert!(is_held(port), "a dry run must not stop anything");

    let out = run(&["--tree", &port.to_string()]);
    assert!(out.status.success(), "{}{}", stdout(&out), stderr(&out));
    assert!(stdout(&out).contains("supervisor"), "{}", stdout(&out));
    assert!(wait_exit(&mut chain), "the supervisor should be gone");
    std::thread::sleep(Duration::from_millis(500));
    assert!(!is_held(port), "nothing may restart the service");
}
