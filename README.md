# freeport

Libère un port en arrêtant le processus qui l'occupe. Linux, macOS, Windows,
FreeBSD, OpenBSD, NetBSD ; TCP et UDP.

## Installation

### Linux et macOS

Télécharge le binaire de la dernière version pour votre système et l'installe
dans `/usr/local/bin` :

```sh
V=$(curl -fsSLI -o /dev/null -w '%{url_effective}' https://github.com/cyrilleadjalala/freeport/releases/latest); V=${V##*/}
case "$(uname -s)-$(uname -m)" in
  Linux-x86_64)               T=x86_64-unknown-linux-musl ;;
  Linux-aarch64|Linux-arm64)  T=aarch64-unknown-linux-musl ;;
  Darwin-x86_64)              T=x86_64-apple-darwin ;;
  Darwin-arm64)               T=aarch64-apple-darwin ;;
  *) echo "plateforme non prise en charge" >&2; false ;;
esac &&
curl -fsSL "https://github.com/cyrilleadjalala/freeport/releases/download/$V/freeport-$V-$T.tar.gz" | tar xz &&
sudo mv "freeport-$V-$T/freeport" /usr/local/bin/ && rm -rf "freeport-$V-$T" &&
freeport --version
```

Les binaires Linux sont statiques : ils fonctionnent sur toute distribution,
quelle que soit la version de glibc.

### Windows (PowerShell)

Installe `freeport.exe` dans `%LOCALAPPDATA%\freeport` et l'ajoute au `PATH`
de l'utilisateur (ouvrir un nouveau terminal ensuite) :

```powershell
$V = (Invoke-RestMethod https://api.github.com/repos/cyrilleadjalala/freeport/releases/latest).tag_name
$T = "x86_64-pc-windows-msvc"; $Dir = "$env:LOCALAPPDATA\freeport"
Invoke-WebRequest "https://github.com/cyrilleadjalala/freeport/releases/download/$V/freeport-$V-$T.zip" -OutFile "$env:TEMP\freeport.zip"
Expand-Archive "$env:TEMP\freeport.zip" -DestinationPath $env:TEMP -Force
New-Item -ItemType Directory -Force $Dir | Out-Null
Move-Item -Force "$env:TEMP\freeport-$V-$T\freeport.exe" $Dir
$P = [Environment]::GetEnvironmentVariable("Path", "User")
if (";$P;" -notlike "*;$Dir;*") { [Environment]::SetEnvironmentVariable("Path", "$P;$Dir", "User") }
```

### Avec Cargo (tous systèmes)

```sh
cargo install freeport-cli
```

La commande installée s'appelle `freeport`.

### Manuellement

Les archives de chaque plateforme sont disponibles dans les
[releases GitHub](https://github.com/cyrilleadjalala/freeport/releases).

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
