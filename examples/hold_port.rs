//! Aide pour les tests : ouvre un port TCP (et UDP) éphémère, affiche-le, puis dort.
use std::io::Write;
use std::net::{TcpListener, UdpSocket};

fn main() {
    let tcp = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = tcp.local_addr().unwrap().port();
    let udp = UdpSocket::bind(("127.0.0.1", 0)).unwrap();
    let uport = udp.local_addr().unwrap().port();
    println!("{port} {uport}");
    std::io::stdout().flush().unwrap();
    std::thread::sleep(std::time::Duration::from_secs(600));
}
