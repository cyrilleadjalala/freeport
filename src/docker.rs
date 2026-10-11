//! Detecting ports published by Docker. Stopping the proxy process behind a
//! published port breaks the container without stopping it, so `freeport`
//! refuses by default and points at the container instead.

use crate::finder::Proto;
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Process names (case-insensitive, without `.exe`) that front a Docker port.
const DOCKER_HOLDERS: &[&str] = &[
    "docker-proxy",
    "com.docker.backend",
    "vpnkit",
    "vpnkit-bridge",
];

pub fn is_docker_holder(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    let n = n.strip_suffix(".exe").unwrap_or(&n);
    DOCKER_HOLDERS.contains(&n)
}

/// One container name per non-empty line.
pub fn parse_names(output: &str) -> Vec<String> {
    output
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect()
}

/// Names of the containers publishing `port`, asked to the `docker` CLI
/// (best effort: empty when Docker is missing, slow, or says nothing).
pub fn containers(port: u16, proto: Proto) -> Vec<String> {
    let filter = format!("publish={port}/{proto}");
    let Ok(mut child) = Command::new("docker")
        .args(["ps", "--filter", &filter, "--format", "{{.Names}}"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return Vec::new();
    };
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break,
            Ok(Some(_)) | Err(_) => return Vec::new(),
            Ok(None) if start.elapsed() > Duration::from_secs(2) => {
                let _ = child.kill();
                let _ = child.wait();
                return Vec::new();
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
        }
    }
    let mut out = String::new();
    if let Some(mut so) = child.stdout.take() {
        let _ = so.read_to_string(&mut out);
    }
    parse_names(&out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_docker_front_processes() {
        for n in [
            "docker-proxy",
            "Docker-Proxy",
            "com.docker.backend.exe",
            "vpnkit.exe",
        ] {
            assert!(is_docker_holder(n), "{n}");
        }
        for n in ["node", "python3", "dockerd", "docker", ""] {
            assert!(!is_docker_holder(n), "{n}");
        }
    }

    #[test]
    fn parses_container_names() {
        assert_eq!(parse_names("web\n  db  \n\n"), vec!["web", "db"]);
        assert!(parse_names("").is_empty());
        assert!(parse_names("\n \n").is_empty());
    }
}
