# freeport

Libère un port en arrêtant le processus qui l'occupe. Linux, macOS, Windows,
FreeBSD, OpenBSD, NetBSD ; TCP et UDP.

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
- **Code de sortie** : `0` si tous les ports sont libres à la fin, `1` sinon.

## Développement

```
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
```

Les releases (binaires par plateforme) sont construites par GitHub Actions à
chaque tag `vX.Y.Z`.
