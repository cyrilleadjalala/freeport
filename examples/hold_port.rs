//! Test helper: holds a TCP port and a UDP port, prints them ("tcp udp"), then
//! sleeps. Options: `--port N` (TCP port, default: any), `--ttl SECS` (exit
//! after that long, default 600). Other arguments are ignored (they show up in
//! the process command line, which the tests look for).
use std::io::Write;
use std::net::{TcpListener, UdpSocket};

fn arg(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

fn main() {
    let port: u16 = arg("--port").and_then(|p| p.parse().ok()).unwrap_or(0);
    let ttl: u64 = arg("--ttl").and_then(|p| p.parse().ok()).unwrap_or(600);
    let tcp = TcpListener::bind(("127.0.0.1", port)).unwrap();
    let tcp_port = tcp.local_addr().unwrap().port();
    let udp = UdpSocket::bind(("127.0.0.1", 0)).unwrap();
    let udp_port = udp.local_addr().unwrap().port();
    println!("{tcp_port} {udp_port}");
    std::io::stdout().flush().unwrap();
    std::thread::sleep(std::time::Duration::from_secs(ttl));
}
