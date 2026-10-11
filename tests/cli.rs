use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

fn freeport() -> Command {
    Command::new(env!("CARGO_BIN_EXE_freeport"))
}

fn holder_path() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_BIN_EXE_freeport"));
    p.pop();
    p.push("examples");
    p.push(format!("hold_port{}", std::env::consts::EXE_SUFFIX));
    p
}

/// Lance un processus qui tient un port TCP et un port UDP.
fn spawn_holder() -> (Child, u16, u16) {
    let mut child = Command::new(holder_path())
        .stdout(Stdio::piped())
        .spawn()
        .expect("exemple hold_port introuvable (cargo test le compile)");
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let mut it = line.split_whitespace().map(|s| s.parse::<u16>().unwrap());
    (child, it.next().unwrap(), it.next().unwrap())
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

#[test]
fn dry_run_reports_without_killing() {
    let (mut child, port, _) = spawn_holder();
    let out = freeport().args(["-n", &port.to_string()]).output().unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("occupé par"));
    assert!(
        child.try_wait().unwrap().is_none(),
        "le processus ne doit pas être tué"
    );
    child.kill().unwrap();
    child.wait().unwrap();
}

#[test]
fn frees_tcp_port_then_reports_already_free() {
    let (mut child, port, _) = spawn_holder();
    let out = freeport().arg(port.to_string()).output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(wait_exit(&mut child), "le processus aurait dû s'arrêter");

    let again = freeport().arg(port.to_string()).output().unwrap();
    assert!(again.status.success());
    assert!(String::from_utf8_lossy(&again.stdout).contains("déjà libre"));
}

#[test]
fn force_and_udp() {
    let (mut child, _, uport) = spawn_holder();
    let out = freeport()
        .args(["-f", "-u", &uport.to_string()])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(wait_exit(&mut child));
}

#[test]
fn rejects_invalid_port() {
    assert!(!freeport().arg("0").status().unwrap().success());
    assert!(!freeport().arg("70000").status().unwrap().success());
    assert!(!freeport().status().unwrap().success());
}
