//! Finding the processes that hold a port (via `listeners`, cross-platform).

use listeners::{Listener, Protocol, SocketState};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fmt;
use std::net::{IpAddr, SocketAddr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
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
    pub path: String,
    pub addrs: Vec<SocketAddr>,
}

/// One row of the "what is listening" view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub proto: Proto,
    pub port: u16,
    pub pid: u32,
    pub name: String,
    pub path: String,
    pub addrs: Vec<SocketAddr>,
}

fn proto_matches(l: &Listener, proto: Proto) -> bool {
    match proto {
        // Only listening TCP sockets block a `bind` (not outgoing connections
        // or TIME_WAIT).
        Proto::Tcp => l.protocol == Protocol::TCP && l.state == SocketState::Listen,
        Proto::Udp => l.protocol == Protocol::UDP,
    }
}

/// A socket conflicts with `want` when it is bound to that address or to the
/// wildcard address (`0.0.0.0` / `::`), which covers every interface.
fn addr_matches(sock: &SocketAddr, want: Option<IpAddr>) -> bool {
    match want {
        None => true,
        Some(ip) => sock.ip() == ip || sock.ip().is_unspecified(),
    }
}

/// Processes holding `port`. A PID appears once even with IPv4 + IPv6 sockets.
pub fn select(all: &[Listener], port: u16, proto: Proto, addr: Option<IpAddr>) -> Vec<Holder> {
    let mut by_pid: BTreeMap<u32, Holder> = BTreeMap::new();
    for l in all {
        if proto_matches(l, proto) && l.socket.port() == port && addr_matches(&l.socket, addr) {
            by_pid
                .entry(l.process.pid)
                .or_insert_with(|| Holder {
                    pid: l.process.pid,
                    name: l.process.name.clone(),
                    path: l.process.path.clone(),
                    addrs: Vec::new(),
                })
                .addrs
                .push(l.socket);
        }
    }
    by_pid.into_values().collect()
}

/// Every listening socket, one row per (port, pid), sorted by port.
pub fn listening(all: &[Listener], proto: Proto, addr: Option<IpAddr>) -> Vec<Entry> {
    let mut rows: BTreeMap<(u16, u32), Entry> = BTreeMap::new();
    for l in all {
        if proto_matches(l, proto) && addr_matches(&l.socket, addr) {
            rows.entry((l.socket.port(), l.process.pid))
                .or_insert_with(|| Entry {
                    proto,
                    port: l.socket.port(),
                    pid: l.process.pid,
                    name: l.process.name.clone(),
                    path: l.process.path.clone(),
                    addrs: Vec::new(),
                })
                .addrs
                .push(l.socket);
        }
    }
    rows.into_values().collect()
}

/// A consistent view of every socket, taken once and reused.
pub fn snapshot() -> Result<Vec<Listener>, String> {
    if !listeners::IS_OS_SUPPORTED {
        return Err("operating system not supported".into());
    }
    listeners::get_all()
        .map(|set| set.into_iter().collect())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use listeners::Process;

    fn l(pid: u32, addr: &str, protocol: Protocol, state: SocketState) -> Listener {
        Listener {
            process: Process {
                pid,
                name: format!("p{pid}"),
                path: String::new(),
            },
            socket: addr.parse::<SocketAddr>().unwrap(),
            protocol,
            state,
        }
    }

    fn pids(h: &[Holder]) -> Vec<u32> {
        h.iter().map(|h| h.pid).collect()
    }

    #[test]
    fn tcp_keeps_only_listening_on_port_and_dedups() {
        let input = vec![
            l(10, "0.0.0.0:8080", Protocol::TCP, SocketState::Listen),
            l(10, "[::]:8080", Protocol::TCP, SocketState::Listen),
            l(
                11,
                "127.0.0.1:8080",
                Protocol::TCP,
                SocketState::Established,
            ),
            l(12, "0.0.0.0:9090", Protocol::TCP, SocketState::Listen),
            l(13, "0.0.0.0:8080", Protocol::UDP, SocketState::Unknown),
        ];
        let got = select(&input, 8080, Proto::Tcp, None);
        assert_eq!(pids(&got), vec![10]);
        assert_eq!(got[0].addrs.len(), 2);
    }

    #[test]
    fn udp_matches_any_state_and_sorts_by_pid() {
        let input = vec![
            l(30, "0.0.0.0:5353", Protocol::UDP, SocketState::Unknown),
            l(20, "[::]:5353", Protocol::UDP, SocketState::Unknown),
            l(21, "0.0.0.0:5353", Protocol::TCP, SocketState::Listen),
        ];
        assert_eq!(pids(&select(&input, 5353, Proto::Udp, None)), vec![20, 30]);
    }

    #[test]
    fn free_port_gives_nothing() {
        assert!(select(&[], 80, Proto::Tcp, None).is_empty());
    }

    #[test]
    fn addr_filter_matches_exact_or_wildcard() {
        let input = vec![
            l(1, "127.0.0.1:3000", Protocol::TCP, SocketState::Listen),
            l(2, "10.0.0.5:3000", Protocol::TCP, SocketState::Listen),
            l(3, "0.0.0.0:3000", Protocol::TCP, SocketState::Listen),
            l(4, "[::]:3000", Protocol::TCP, SocketState::Listen),
        ];
        let lo: IpAddr = "127.0.0.1".parse().unwrap();
        // 127.0.0.1 itself, plus the wildcards that also cover it.
        assert_eq!(
            pids(&select(&input, 3000, Proto::Tcp, Some(lo))),
            vec![1, 3, 4]
        );
        let other: IpAddr = "192.168.1.9".parse().unwrap();
        assert_eq!(
            pids(&select(&input, 3000, Proto::Tcp, Some(other))),
            vec![3, 4]
        );
    }

    #[test]
    fn listing_is_one_row_per_port_and_pid_sorted_by_port() {
        let input = vec![
            l(7, "0.0.0.0:9000", Protocol::TCP, SocketState::Listen),
            l(7, "[::]:9000", Protocol::TCP, SocketState::Listen),
            l(8, "127.0.0.1:80", Protocol::TCP, SocketState::Listen),
            l(9, "127.0.0.1:81", Protocol::TCP, SocketState::Established),
            l(5, "0.0.0.0:53", Protocol::UDP, SocketState::Unknown),
        ];
        let rows = listening(&input, Proto::Tcp, None);
        assert_eq!(
            rows.iter().map(|r| (r.port, r.pid)).collect::<Vec<_>>(),
            vec![(80, 8), (9000, 7)]
        );
        assert_eq!(rows[1].addrs.len(), 2);
        assert_eq!(listening(&input, Proto::Udp, None).len(), 1);
    }
}
