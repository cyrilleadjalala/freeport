//! Recherche des processus qui occupent un port (via `listeners`, multiplateforme).

use listeners::{Listener, Protocol, SocketState};
use std::collections::BTreeMap;
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
    pub path: String,
}

/// Retient les processus qui occupent `port`. Pour TCP, seuls les sockets en
/// écoute comptent (les connexions sortantes ou en TIME_WAIT ne bloquent pas
/// un `bind`). Un même PID n'apparaît qu'une fois (IPv4 + IPv6).
pub fn select(
    listeners: impl IntoIterator<Item = Listener>,
    port: u16,
    proto: Proto,
) -> Vec<Holder> {
    let mut by_pid = BTreeMap::new();
    for l in listeners {
        let proto_ok = match proto {
            Proto::Tcp => l.protocol == Protocol::TCP && l.state == SocketState::Listen,
            Proto::Udp => l.protocol == Protocol::UDP,
        };
        if proto_ok && l.socket.port() == port {
            by_pid.entry(l.process.pid).or_insert_with(|| Holder {
                pid: l.process.pid,
                name: l.process.name,
                path: l.process.path,
            });
        }
    }
    by_pid.into_values().collect()
}

pub fn find(port: u16, proto: Proto) -> Result<Vec<Holder>, String> {
    if !listeners::IS_OS_SUPPORTED {
        return Err("système d'exploitation non pris en charge".into());
    }
    let all = listeners::get_all().map_err(|e| e.to_string())?;
    Ok(select(all, port, proto))
}

#[cfg(test)]
mod tests {
    use super::*;
    use listeners::Process;
    use std::net::SocketAddr;

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
        let got = select(input, 8080, Proto::Tcp);
        assert_eq!(got.iter().map(|h| h.pid).collect::<Vec<_>>(), vec![10]);
    }

    #[test]
    fn udp_matches_any_state_and_sorts_by_pid() {
        let input = vec![
            l(30, "0.0.0.0:5353", Protocol::UDP, SocketState::Unknown),
            l(20, "[::]:5353", Protocol::UDP, SocketState::Unknown),
            l(21, "0.0.0.0:5353", Protocol::TCP, SocketState::Listen),
        ];
        let got = select(input, 5353, Proto::Udp);
        assert_eq!(got.iter().map(|h| h.pid).collect::<Vec<_>>(), vec![20, 30]);
    }

    #[test]
    fn free_port_gives_nothing() {
        assert!(select(vec![], 80, Proto::Tcp).is_empty());
    }
}
