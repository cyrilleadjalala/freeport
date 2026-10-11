# freeport

[![CI](https://github.com/cyrilleadjalala/freeport/actions/workflows/ci.yml/badge.svg)](https://github.com/cyrilleadjalala/freeport/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/freeport-cli.svg)](https://crates.io/crates/freeport-cli)
[![licence : MIT](https://img.shields.io/badge/licence-MIT-blue.svg)](LICENSE)

**Libère un port en arrêtant le processus qui l'occupe.** Un petit binaire statique pour Linux, macOS, Windows et
les BSD. TCP et UDP. Aucun runtime, rien d'autre à installer.

[English](README.md)

```console
$ freeport 3000
port 3000/tcp : node (pid 4821) arrêté

$ freeport 3000
port 3000/tcp : déjà libre
```

(Les messages sont en français sur un système en français, en anglais sinon ; voir « Langue » plus bas.)

## Installation

Une seule commande, sans aucun prérequis (ni Rust, ni Visual Studio, ni droits administrateur). Elle télécharge le
binaire de la dernière version et vérifie son SHA-256.

**Linux et macOS**

```sh
curl -fsSL https://raw.githubusercontent.com/cyrilleadjalala/freeport/main/install.sh | sh
```

Installe dans `/usr/local/bin` si le dossier est accessible en écriture (par exemple en root), sinon dans
`~/.local/bin`.

**Windows (PowerShell)**

```powershell
irm https://raw.githubusercontent.com/cyrilleadjalala/freeport/main/install.ps1 | iex
```

Installe dans `%LOCALAPPDATA%\freeport` et l'ajoute au `PATH` de l'utilisateur (ouvrez un nouveau terminal ensuite).

Vérifiez : `freeport --version`.

**Autres méthodes**

- Archives par plateforme (avec leur `.sha256`) sur la [page des releases](https://github.com/cyrilleadjalala/freeport/releases).
- `cargo binstall freeport-cli` : télécharge le binaire de la release, sans compilation. Il demande à l'API de
  GitHub quels fichiers existent ; si GitHub répond `403` (limite de requêtes anonymes, fréquente sur les
  machines de CI partagées), définissez `GITHUB_TOKEN` (n'importe quel jeton, sans permission particulière),
  sinon il retombe sur la compilation.
- `cargo install freeport-cli` : compile depuis les sources (nécessite Rust).

Les installeurs lisent `FREEPORT_VERSION` (ex. `v0.2.0`), `FREEPORT_INSTALL_DIR`, `FREEPORT_LANG` (`en`/`fr`) et
`FREEPORT_SKIP_VERIFY=1` (saute la vérification, déconseillé). À partir de la 0.2.0, une release sans empreinte
publiée est refusée. L'empreinte protège d'un téléchargement corrompu ou altéré ; elle ne remplace pas la confiance
dans le compte GitHub qui publie la release.

## Utilisation

```console
$ freeport --list                       # qui écoute ? (aussi le comportement sans argument)
PROTO  PORT  PID   PROCESSUS  ADRESSE    COMMANDE
tcp    3000  3907  python3    0.0.0.0    python3 -m http.server 3000
tcp    3001  3908  python3    127.0.0.1  python3 -m http.server 3001 --bind 127.0.0.1

$ freeport -n 3000                      # simulation : montre, n'arrête rien
port 3000/tcp : occupé par python3 (pid 3907) — python3 -m http.server 3000

$ freeport 3000 8080                    # plusieurs ports
$ freeport 3000-3010                    # une plage (1000 ports au plus)
$ freeport -f 3000                      # arrêt forcé immédiat
$ freeport -u 5353                      # UDP au lieu de TCP
$ freeport -i 3000                      # demande confirmation
$ freeport --addr 127.0.0.1 3000        # seulement ce qui écoute sur cette adresse
$ freeport --tree 3000                  # arrête aussi superviseurs et processus enfants
$ freeport --json -n 3000               # sortie lisible par un programme
```

## Pourquoi freeport

`kill $(lsof -ti:3000)` marche, jusqu'au jour où non. `freeport` couvre les cas autour :

- **Sûr par défaut.** Il ne touche jamais aux processus système, à lui-même, ni à **aucun de ses propres
  ancêtres** (votre shell, `npm run`, votre terminal). `-n` pour prévisualiser, `-i` pour confirmer.
- **Poli, puis ferme.** SIGTERM d'abord, SIGKILL après un délai (`-t`, 3 s par défaut). Sous Windows, il demande avec
  `taskkill` puis retombe sur `TerminateProcess`.
- **Il vérifie le résultat.** Après l'arrêt, il relit les sockets et signale si quelque chose a repris le port, par
  exemple un superviseur qui relance le service.
- **`--tree` pour les superviseurs.** Quand `nodemon`, un script d'enveloppe ou une boucle type systemd relance votre
  service, `--tree` arrête d'abord les superviseurs, puis le processus, puis ses enfants. La remontée s'arrête devant
  les shells, terminaux, systèmes d'init et les ancêtres de `freeport` ; `-n` montre exactement ce qui serait arrêté.
- **Conscient de Docker.** Un port publié par un conteneur est tenu par `docker-proxy` (ou le backend de Docker
  Desktop). L'arrêter casse le conteneur sans le stopper : `freeport` refuse et indique quel conteneur `docker stop`.
  `--allow-docker` force.
- **Scriptable.** `--json`, `--quiet`, codes de sortie stables.
- **Multiplateforme, un seul binaire statique.** Linux (x86_64, ARM64), macOS (Intel, Apple Silicon), Windows.

## Options

| Option | Signification |
|---|---|
| `PORT...` | Ports ou plages : `3000`, `8080`, `3000-3010`. Aucun : liste les ports en écoute. |
| `-l`, `--list` | Liste les ports en écoute (TCP, ou UDP avec `-u`) ; filtrable par `PORT`. |
| `-n`, `--dry-run` | Montre ce qui serait arrêté, n'arrête rien. |
| `-f`, `--force` | Arrêt immédiat (SIGKILL), sans arrêt propre préalable. |
| `-u`, `--udp` | Cible UDP au lieu de TCP. |
| `-t`, `--timeout <s>` | Secondes accordées à l'arrêt propre avant de forcer (3 par défaut). |
| `-i`, `--interactive` | Demande confirmation avant chaque arrêt. Nécessite un terminal (code 2 sinon). |
| `--addr <IP>` | Seulement les processus à l'écoute sur cette adresse. Un écouteur générique (`0.0.0.0`, `::`) correspond aussi, car il entre en conflit. |
| `--tree` | Arrête aussi superviseurs et processus enfants. |
| `--allow-docker` | Autorise l'arrêt d'un processus géré par Docker (déconseillé). |
| `--json` | Un document JSON sur la sortie standard. |
| `-q`, `--quiet` | Rien sur la sortie standard ; les problèmes restent sur l'erreur standard. |
| `--completions <SHELL>` | Écrit le script de complétion (`bash`, `zsh`, `fish`, `powershell`, `elvish`). |

**Codes de sortie :** `0` tous les ports demandés sont libres à la fin ; `1` au moins un ne l'est pas (bloqué,
refusé, décliné, repris) ; `2` arguments invalides.

**Langue.** Messages et `--help` en anglais, ou en français sur un système en français. Forcez avec
`FREEPORT_LANG=en` ou `fr`. (Les erreurs d'arguments produites par l'analyseur lui-même restent en anglais.)

### JSON

```console
$ freeport --json -n 3000 3001
{"results":[{"port":3000,"proto":"tcp","status":"would_stop","role":"holder","pid":3907,"name":"python3","command":"python3 -m http.server 3000"},{"port":3001,"proto":"tcp","status":"free","role":"holder"}]}
```

`status` vaut `free`, `would_stop`, `stopped`, `killed`, `protected`, `denied`, `survived`, `docker`, `still_busy`
ou `skipped`. `role` vaut `holder`, `supervisor` ou `child` (avec `--tree`). `detail` porte un indice quand il y en
a un (par exemple le conteneur Docker). `--list --json` donne
`{"listeners":[{"proto","port","pid","name","command","addresses"}]}`.

### Complétions du shell

```sh
freeport --completions bash > ~/.local/share/bash-completion/completions/freeport
freeport --completions zsh  > "${fpath[1]}/_freeport"
freeport --completions fish > ~/.config/fish/completions/freeport.fish
```

## Limites

- Arrêter le processus d'un autre utilisateur ou un service système demande `sudo` / un shell administrateur.
- Windows n'a pas de vrai SIGTERM : les applications à fenêtre se ferment poliment via `taskkill`, les applications
  console sont terminées.
- Les plages sont limitées à 1000 ports pour qu'une faute de frappe n'arrête pas la moitié de votre machine.
- Les releases ne sont pas signées. Télécharger le binaire avec un navigateur peut déclencher SmartScreen (Windows) ou
  Gatekeeper (macOS) ; les commandes d'installation ci-dessus, non.

## Développement

```sh
cargo test
cargo clippy --all-targets -- -D warnings
sh tests/install_test.sh          # vérifications des installeurs, hors ligne
```

Publier : augmenter `version` dans `Cargo.toml`, fusionner, puis créer une release `vX.Y.Z` sur GitHub. GitHub Actions
construit les binaires des cinq plateformes, les attache avec leur `.sha256`, génère les notes de version et publie le
paquet (secret `CARGO_REGISTRY_TOKEN`).

## Licence

[MIT](LICENSE)
