# freeport

[![CI](https://github.com/cyrilleadjalala/freeport/actions/workflows/ci.yml/badge.svg)](https://github.com/cyrilleadjalala/freeport/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/freeport-cli.svg)](https://crates.io/crates/freeport-cli)
[![license: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

**Free a port by stopping the process that holds it.** One small static binary for Linux, macOS, Windows and
the BSDs. TCP and UDP. No runtime, nothing else to install.

[Français](README.fr.md)

```console
$ freeport 3000
port 3000/tcp: node (pid 4821) stopped

$ freeport 3000
port 3000/tcp: already free
```

## Install

One command, no prerequisite (no Rust, no Visual Studio, no administrator rights). It downloads the binary
of the latest release and verifies its SHA-256.

**Linux and macOS**

```sh
curl -fsSL https://raw.githubusercontent.com/cyrilleadjalala/freeport/main/install.sh | sh
```

Installs into `/usr/local/bin` when writable (e.g. as root), otherwise into `~/.local/bin`.

**Windows (PowerShell)**

```powershell
irm https://raw.githubusercontent.com/cyrilleadjalala/freeport/main/install.ps1 | iex
```

Installs into `%LOCALAPPDATA%\freeport` and adds it to your user `PATH` (open a new terminal afterwards).

Then check it: `freeport --version`.

**Other ways**

- Prebuilt archives (and their `.sha256`) on the [releases page](https://github.com/cyrilleadjalala/freeport/releases).
- `cargo binstall freeport-cli`: downloads the release binary, no compilation.
- `cargo install freeport-cli`: compiles from source (needs Rust).

The installers read `FREEPORT_VERSION` (e.g. `v0.2.0`), `FREEPORT_INSTALL_DIR`, `FREEPORT_LANG` (`en`/`fr`) and
`FREEPORT_SKIP_VERIFY=1` (skips the checksum; not recommended). From 0.2.0 on, a release without a published
checksum is refused. The checksum protects against corrupted or altered downloads; it does not replace trust in
the GitHub account that publishes the release.

## Usage

```console
$ freeport --list                       # what is listening? (also the default without arguments)
PROTO  PORT  PID   PROCESS  ADDRESS    COMMAND
tcp    3000  3907  python3  0.0.0.0    python3 -m http.server 3000
tcp    3001  3908  python3  127.0.0.1  python3 -m http.server 3001 --bind 127.0.0.1

$ freeport -n 3000                      # dry run: show, stop nothing
port 3000/tcp: held by python3 (pid 3907) — python3 -m http.server 3000

$ freeport 3000 8080                    # several ports
$ freeport 3000-3010                    # a range (at most 1000 ports)
$ freeport -f 3000                      # kill immediately, no polite stop
$ freeport -u 5353                      # UDP instead of TCP
$ freeport -i 3000                      # ask before stopping
$ freeport --addr 127.0.0.1 3000        # only what listens on that address
$ freeport --tree 3000                  # also stop supervisors and child processes
$ freeport --json -n 3000               # machine-readable output
```

## Why freeport

`kill $(lsof -ti:3000)` works until it does not. `freeport` is built for the cases around it:

- **Safe by default.** It never touches system processes, itself, or **any of its own ancestors** (your
  shell, `npm run`, your terminal). Use `-n` to preview and `-i` to confirm.
- **Polite, then firm.** SIGTERM first, SIGKILL after a timeout (`-t`, 3 s by default). On Windows it asks with
  `taskkill` and falls back to `TerminateProcess`.
- **It checks the result.** After stopping, it re-reads the sockets and tells you if something took the port back,
  for instance a supervisor restarting the service.
- **`--tree` for supervisors.** When `nodemon`, a wrapper script or a systemd-like loop restarts your service,
  `--tree` stops the supervisors first, then the holder, then its children. The climb stops at shells,
  terminals, init systems and `freeport`'s own ancestors; `-n` shows exactly what would be stopped.
- **Docker-aware.** A port published by a container is held by `docker-proxy` (or Docker Desktop's backend).
  Killing it breaks the container without stopping it, so `freeport` refuses and tells you which container to
  `docker stop`. `--allow-docker` overrides.
- **Scriptable.** `--json`, `--quiet`, stable exit codes.
- **Cross-platform, one static binary.** Linux (x86_64, ARM64), macOS (Intel, Apple Silicon), Windows.

## Options

| Option | Meaning |
|---|---|
| `PORT...` | Ports or ranges: `3000`, `8080`, `3000-3010`. None: list listening ports. |
| `-l`, `--list` | List listening ports (TCP, or UDP with `-u`); optionally filtered by `PORT`. |
| `-n`, `--dry-run` | Show what would be stopped; stop nothing. |
| `-f`, `--force` | Kill immediately (SIGKILL), no polite stop first. |
| `-u`, `--udp` | Target UDP instead of TCP. |
| `-t`, `--timeout <s>` | Seconds for the polite stop before forcing (default 3). |
| `-i`, `--interactive` | Ask before each stop. Needs a terminal (exit code 2 otherwise). |
| `--addr <IP>` | Only processes listening on this address. A wildcard listener (`0.0.0.0`, `::`) also matches, since it conflicts. |
| `--tree` | Also stop supervisors and child processes. |
| `--allow-docker` | Allow stopping a Docker-managed process (not recommended). |
| `--json` | One JSON document on standard output. |
| `-q`, `--quiet` | Nothing on standard output; problems still go to standard error. |
| `--completions <SHELL>` | Print the completion script (`bash`, `zsh`, `fish`, `powershell`, `elvish`). |

**Exit codes:** `0` every requested port is free at the end; `1` at least one is not (blocked, refused,
declined, taken back); `2` invalid arguments.

**Language.** Messages and `--help` are in English, or in French on a French system. Force with
`FREEPORT_LANG=en` or `fr`. (Argument errors produced by the parser itself stay in English.)

### JSON

```console
$ freeport --json -n 3000 3001
{"results":[{"port":3000,"proto":"tcp","status":"would_stop","role":"holder","pid":3907,"name":"python3","command":"python3 -m http.server 3000"},{"port":3001,"proto":"tcp","status":"free","role":"holder"}]}
```

`status` is one of `free`, `would_stop`, `stopped`, `killed`, `protected`, `denied`, `survived`, `docker`,
`still_busy`, `skipped`. `role` is `holder`, `supervisor` or `child` (with `--tree`). `detail` carries a hint
when there is one (e.g. the Docker container). `--list --json` gives
`{"listeners":[{"proto","port","pid","name","command","addresses"}]}`.

### Shell completions

```sh
freeport --completions bash > ~/.local/share/bash-completion/completions/freeport
freeport --completions zsh  > "${fpath[1]}/_freeport"
freeport --completions fish > ~/.config/fish/completions/freeport.fish
```

## Limits

- Stopping a process owned by another user, or a system service, needs `sudo` / an administrator shell.
- Windows has no real SIGTERM: windowed apps close politely through `taskkill`; console apps are terminated.
- Ranges are capped at 1000 ports so a typo cannot stop half your machine.
- Releases are not code-signed. Downloading the binary with a browser may trigger SmartScreen (Windows) or
  Gatekeeper (macOS); the install commands above do not.

## Development

```sh
cargo test
cargo clippy --all-targets -- -D warnings
sh tests/install_test.sh          # installer checks, offline
```

Release: bump `version` in `Cargo.toml`, merge, then create a `vX.Y.Z` release on GitHub. GitHub Actions builds
the binaries for the five platforms, attaches them with their `.sha256`, generates the release notes and
publishes the crate (`CARGO_REGISTRY_TOKEN` secret).

## License

[MIT](LICENSE)
