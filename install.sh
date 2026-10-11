#!/bin/sh
# Installe freeport (Linux, macOS) : télécharge le binaire de la dernière
# version, sans rien d'autre à installer.
#
#   curl -fsSL https://raw.githubusercontent.com/cyrilleadjalala/freeport/main/install.sh | sh
#
# Variables facultatives :
#   FREEPORT_VERSION      version à installer (ex. v0.1.2) ; défaut : la dernière
#   FREEPORT_INSTALL_DIR  dossier d'installation ; défaut : /usr/local/bin si
#                         accessible en écriture, sinon ~/.local/bin
set -eu

REPO="cyrilleadjalala/freeport"

err() { printf 'freeport : %s\n' "$*" >&2; exit 1; }

if command -v curl >/dev/null 2>&1; then
  download() { curl -fsSL -o "$2" "$1"; }
  final_url() { curl -fsSLI -o /dev/null -w '%{url_effective}' "$1"; }
elif command -v wget >/dev/null 2>&1; then
  download() { wget -qO "$2" "$1"; }
  final_url() { wget --spider -S "$1" 2>&1 | sed -n 's/^ *[Ll]ocation: *//p' | tail -n 1; }
else
  err "curl ou wget est requis"
fi

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64)              target=x86_64-unknown-linux-musl ;;
  Linux-aarch64|Linux-arm64) target=aarch64-unknown-linux-musl ;;
  Darwin-x86_64)             target=x86_64-apple-darwin ;;
  Darwin-arm64)              target=aarch64-apple-darwin ;;
  *) err "plateforme non prise en charge : $(uname -s) $(uname -m)" ;;
esac

version="${FREEPORT_VERSION:-}"
if [ -z "$version" ]; then
  version="$(final_url "https://github.com/$REPO/releases/latest")"
  version="${version##*/}"
fi
case "$version" in
  v[0-9]*) ;;
  *) err "impossible de déterminer la dernière version (obtenu : '$version')" ;;
esac

dir="${FREEPORT_INSTALL_DIR:-}"
if [ -z "$dir" ]; then
  if [ -w /usr/local/bin ]; then dir=/usr/local/bin; else dir="$HOME/.local/bin"; fi
fi
mkdir -p "$dir" || err "impossible de créer $dir"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT INT TERM

name="freeport-$version-$target"
printf 'Installation de freeport %s (%s) dans %s\n' "$version" "$target" "$dir"
download "https://github.com/$REPO/releases/download/$version/$name.tar.gz" "$tmp/freeport.tar.gz" \
  || err "téléchargement impossible : la version $version existe-t-elle pour $target ?"
tar xzf "$tmp/freeport.tar.gz" -C "$tmp" || err "archive invalide"
install -m 755 "$tmp/$name/freeport" "$dir/freeport" 2>/dev/null \
  || { cp "$tmp/$name/freeport" "$dir/freeport" && chmod 755 "$dir/freeport"; } \
  || err "impossible d'écrire dans $dir (essayez FREEPORT_INSTALL_DIR=\$HOME/.local/bin)"

"$dir/freeport" --version
case ":$PATH:" in
  *":$dir:"*) ;;
  *) printf '\nAjoutez %s à votre PATH pour utiliser « freeport » :\n  export PATH="%s:$PATH"\n' "$dir" "$dir" ;;
esac
