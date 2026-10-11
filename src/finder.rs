//! Recherche des processus qui occupent un port.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Proto {
    Tcp,
    Udp,
}

impl fmt::Display for Proto {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Proto::Tcp => "tcp",
            Proto::Udp => "udp",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Holder {
    pub pid: u32,
    pub name: String,
    pub cmdline: String,
}

#[cfg(target_os = "linux")]
pub use linux::find;
#[cfg(not(target_os = "linux"))]
pub use lsof::find;

#[cfg(target_os = "linux")]
mod linux {
    use super::{Holder, Proto};
    use std::collections::HashSet;
    use std::fs;
    use std::io;

    /// Extrait les inodes des sockets locales sur `port` d'un fichier /proc/net/*.
    /// Pour TCP seuls les sockets en écoute (état 0A) comptent ; pour UDP, tous.
    pub(super) fn parse_inodes(table: &str, port: u16, proto: Proto) -> Vec<u64> {
        table
            .lines()
            .skip(1)
            .filter_map(|line| {
                let f: Vec<&str> = line.split_whitespace().collect();
                if f.len() < 10 {
                    return None;
                }
                let local_port = u16::from_str_radix(f[1].rsplit(':').next()?, 16).ok()?;
                if local_port != port || (proto == Proto::Tcp && f[3] != "0A") {
                    return None;
                }
                f[9].parse().ok()
            })
            .collect()
    }

    pub fn find(port: u16, proto: Proto) -> io::Result<Vec<Holder>> {
        let files: &[&str] = match proto {
            Proto::Tcp => &["/proc/net/tcp", "/proc/net/tcp6"],
            Proto::Udp => &["/proc/net/udp", "/proc/net/udp6"],
        };
        let mut inodes = HashSet::new();
        for file in files {
            if let Ok(content) = fs::read_to_string(file) {
                inodes.extend(parse_inodes(&content, port, proto));
            }
        }
        inodes.remove(&0);
        if inodes.is_empty() {
            return Ok(vec![]);
        }

        let mut holders = Vec::new();
        for entry in fs::read_dir("/proc")?.flatten() {
            let Some(pid) = entry.file_name().to_str().and_then(|s| s.parse::<u32>().ok()) else {
                continue;
            };
            let Ok(fds) = fs::read_dir(entry.path().join("fd")) else {
                continue; // processus disparu ou non lisible
            };
            let owns = fds.flatten().any(|fd| {
                fs::read_link(fd.path())
                    .ok()
                    .and_then(|l| {
                        let l = l.to_string_lossy().into_owned();
                        l.strip_prefix("socket:[")?
                            .strip_suffix(']')?
                            .parse::<u64>()
                            .ok()
                    })
                    .is_some_and(|i| inodes.contains(&i))
            });
            if owns {
                let name = fs::read_to_string(entry.path().join("comm"))
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                let cmdline = fs::read(entry.path().join("cmdline"))
                    .map(|b| {
                        String::from_utf8_lossy(&b)
                            .trim_end_matches('\0')
                            .replace('\0', " ")
                    })
                    .unwrap_or_default();
                holders.push(Holder { pid, name, cmdline });
            }
        }
        holders.sort_by_key(|h| h.pid);
        Ok(holders)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        const TCP: &str = "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode
   0: 0100007F:1F90 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 12345 1 0000000000000000 100 0 0 10 0
   1: 0100007F:1F90 0100007F:D2A0 01 00000000:00000000 00:00000000 00000000  1000        0 22222 1 0000000000000000 100 0 0 10 0
   2: 00000000:0050 00000000:0000 0A 00000000:00000000 00:00000000 00000000     0        0 33333 1 0000000000000000 100 0 0 10 0
";

        #[test]
        fn tcp_only_listening_on_port() {
            assert_eq!(parse_inodes(TCP, 8080, Proto::Tcp), vec![12345]);
            assert_eq!(parse_inodes(TCP, 80, Proto::Tcp), vec![33333]);
            assert!(parse_inodes(TCP, 9999, Proto::Tcp).is_empty());
        }

        #[test]
        fn udp_ignores_state() {
            assert_eq!(parse_inodes(TCP, 8080, Proto::Udp), vec![12345, 22222]);
        }
    }
}

#[cfg(not(target_os = "linux"))]
mod lsof {
    use super::{Holder, Proto};
    use std::io;
    use std::process::Command;

    pub fn find(port: u16, proto: Proto) -> io::Result<Vec<Holder>> {
        let spec = match proto {
            Proto::Tcp => format!("-iTCP:{port}"),
            Proto::Udp => format!("-iUDP:{port}"),
        };
        let mut cmd = Command::new("lsof");
        cmd.args(["-nP", "-t", &spec]);
        if proto == Proto::Tcp {
            cmd.arg("-sTCP:LISTEN");
        }
        let out = cmd.output()?;
        let mut holders: Vec<Holder> = String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|l| l.trim().parse::<u32>().ok())
            .map(|pid| {
                let ps = |field: &str| {
                    Command::new("ps")
                        .args(["-p", &pid.to_string(), "-o", field])
                        .output()
                        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                        .unwrap_or_default()
                };
                Holder { pid, name: ps("comm="), cmdline: ps("command=") }
            })
            .collect();
        holders.sort_by_key(|h| h.pid);
        holders.dedup_by_key(|h| h.pid);
        Ok(holders)
    }
}
