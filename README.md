# freeport

Libère un port en arrêtant le processus qui l'occupe. Linux, macOS, Windows,
FreeBSD, OpenBSD, NetBSD ; TCP et UDP.

## Installation

Une seule commande, sans rien d'autre à installer (ni Rust, ni Visual Studio) :
elle télécharge le petit binaire (moins de 1 Mo) de la dernière version.

**Linux et macOS**

```sh
curl -fsSL https://raw.githubusercontent.com/cyrilleadjalala/freeport/main/install.sh | sh
```

Installe dans `/usr/local/bin` si le dossier est accessible en écriture
(par exemple en root), sinon dans `~/.local/bin`.

**Windows (PowerShell)**

```powershell
irm https://raw.githubusercontent.com/cyrilleadjalala/freeport/main/install.ps1 | iex
```

Installe dans `%LOCALAPPDATA%\freeport` et l'ajoute au `PATH` de l'utilisateur
(aucun droit administrateur requis).

Ensuite : `freeport --version`.

Options : `FREEPORT_VERSION=v0.1.2` pour une version précise,
`FREEPORT_INSTALL_DIR=<dossier>` pour choisir le dossier d'installation.

**Autres méthodes**

- Archives par plateforme dans les
  [releases GitHub](https://github.com/cyrilleadjalala/freeport/releases).
- Avec Rust déjà installé : `cargo install freeport-cli`.

## Utilisation

```
freeport 3000            # arrêt propre (SIGTERM), puis forcé après 3 s si besoin
freeport 3000 8080       # plusieurs ports
freeport -n 3000         # simulation : liste sans rien arrêter
freeport -f 3000         # arrêt forcé immédiat
freeport -u 5353         # UDP au lieu de TCP
freeport -t 10 3000      # délai d'arrêt propre avant de forcer
```

## Comportement

- **TCP** : seuls les sockets en écoute sont visés.
- **Arrêt** : SIGTERM puis SIGKILL sous Unix ; `TerminateProcess` sous Windows
  (pas d'arrêt « propre » possible).
- **Garde-fous** : jamais le PID 1 / les PID système, ni `freeport` lui-même,
  ni son processus parent.
- **Vérification** : après l'arrêt, le port est re-testé ; si un autre processus
  (ou un superviseur qui relance le service) l'a repris, c'est signalé.
- **Droits** : pour un processus d'un autre utilisateur, relancer avec `sudo`
  ou en administrateur.
- **Code de sortie** : `0` si tous les ports sont libres à la fin, `1` sinon,
  `2` si les arguments sont invalides.

## Développement

```
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
```

## Publier une version

1. Mettre à jour `version` dans `Cargo.toml` et fusionner dans `main`.
2. Créer une release `vX.Y.Z` sur GitHub, avec le tag sur `main`.

GitHub Actions construit alors les binaires des 5 plateformes, les attache à la
release et publie le paquet sur crates.io (secret `CARGO_REGISTRY_TOKEN`).
