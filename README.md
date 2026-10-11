# freeport

Libère un port en arrêtant le processus qui l'occupe.

```
freeport 3000            # SIGTERM, puis SIGKILL après 3 s si besoin
freeport 3000 8080       # plusieurs ports
freeport -n 3000         # simulation : liste sans rien arrêter
freeport -f 3000         # SIGKILL immédiat
freeport -u 5353         # UDP au lieu de TCP
freeport -t 10 3000      # délai avant SIGKILL
```

Linux : lecture de `/proc` (aucune dépendance externe). Autres Unix : `lsof`.
Code de sortie non nul si un port n'a pas pu être libéré.

```
cargo build --release   # binaire dans target/release/freeport
cargo test
```
