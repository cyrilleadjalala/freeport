//! Test helper: a supervisor. Runs `hold_port` as a child and restarts it
//! whenever it exits, like nodemon or a systemd unit would.
//!
//!   spawn_chain <path-to-hold_port> <tcp-port> <ttl-secs>
//!
//! Prints the supervised child's "tcp udp" line once, then loops. It gives up
//! after `ttl-secs` so a failed test never leaves it running.
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (exe, port, ttl) = (&args[1], &args[2], args[3].parse::<u64>().unwrap());
    let deadline = Instant::now() + Duration::from_secs(ttl);
    let mut announced = false;
    while Instant::now() < deadline {
        let mut child = Command::new(exe)
            .args(["--port", port, "--ttl", &ttl.to_string()])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut line = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        if !announced {
            print!("{line}");
            std::io::stdout().flush().unwrap();
            announced = true;
        }
        let _ = child.wait();
        std::thread::sleep(Duration::from_millis(100)); // then restart it
    }
}
