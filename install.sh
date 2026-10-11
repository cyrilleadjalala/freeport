#!/bin/sh
# Install freeport (Linux, macOS): downloads the binary of the latest release,
# checks its SHA-256, and needs nothing else installed.
#
#   curl -fsSL https://raw.githubusercontent.com/cyrilleadjalala/freeport/main/install.sh | sh
#
# Optional environment variables:
#   FREEPORT_VERSION       version to install (e.g. v0.2.0); default: the latest
#   FREEPORT_INSTALL_DIR   install directory; default: /usr/local/bin if writable,
#                          otherwise ~/.local/bin
#   FREEPORT_SKIP_VERIFY=1 skip the SHA-256 check (not recommended)
#   FREEPORT_LANG          en or fr (default: from LC_ALL / LC_MESSAGES / LANG)
#   FREEPORT_BASE_URL      release URL prefix, for testing only
set -eu

REPO="cyrilleadjalala/freeport"
BASE_URL="${FREEPORT_BASE_URL:-https://github.com/$REPO/releases}"

case "${FREEPORT_LANG:-${LC_ALL:-${LC_MESSAGES:-${LANG:-}}}}" in
  fr*|FR*) lang=fr ;;
  *) lang=en ;;
esac

say() { if [ "$lang" = fr ]; then printf '%s\n' "$2"; else printf '%s\n' "$1"; fi; }
warn() { if [ "$lang" = fr ]; then printf 'freeport : %s\n' "$2" >&2; else printf 'freeport: %s\n' "$1" >&2; fi; }
err() { warn "$1" "$2"; exit 1; }

if command -v curl >/dev/null 2>&1; then
  download() { curl -fsSL -o "$2" "$1" 2>/dev/null; }
  final_url() { curl -fsSLI -o /dev/null -w '%{url_effective}' "$1"; }
elif command -v wget >/dev/null 2>&1; then
  download() { wget -qO "$2" "$1" 2>/dev/null; }
  final_url() { wget --spider -S "$1" 2>&1 | sed -n 's/^ *[Ll]ocation: *//p' | tail -n 1; }
else
  err "curl or wget is required" "curl ou wget est requis"
fi

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | cut -d' ' -f1
  elif command -v shasum >/dev/null 2>&1; then shasum -a 256 "$1" | cut -d' ' -f1
  elif command -v openssl >/dev/null 2>&1; then openssl dgst -sha256 "$1" | sed 's/^.*= //'
  else return 1
  fi
}

# ver_ge A B: is version A >= version B? (both X.Y.Z, optional -suffix)
ver_ge() {
  old_ifs=$IFS; IFS=.; set -- $1 $2; IFS=$old_ifs
  a3=${3%%[!0-9]*}; b3=${6%%[!0-9]*}
  [ "$1" -gt "$4" ] && return 0; [ "$1" -lt "$4" ] && return 1
  [ "$2" -gt "$5" ] && return 0; [ "$2" -lt "$5" ] && return 1
  [ "${a3:-0}" -ge "${b3:-0}" ]
}

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64)              target=x86_64-unknown-linux-musl ;;
  Linux-aarch64|Linux-arm64) target=aarch64-unknown-linux-musl ;;
  Darwin-x86_64)             target=x86_64-apple-darwin ;;
  Darwin-arm64)              target=aarch64-apple-darwin ;;
  *) err "unsupported platform: $(uname -s) $(uname -m)" "plateforme non prise en charge : $(uname -s) $(uname -m)" ;;
esac

version="${FREEPORT_VERSION:-}"
if [ -z "$version" ]; then
  version="$(final_url "$BASE_URL/latest")"
  version="${version##*/}"
fi
case "$version" in
  v[0-9]*) ;;
  *) err "cannot determine the latest version (got: '$version')" "impossible de déterminer la dernière version (obtenu : '$version')" ;;
esac

dir="${FREEPORT_INSTALL_DIR:-}"
if [ -z "$dir" ]; then
  if [ -w /usr/local/bin ]; then dir=/usr/local/bin; else dir="$HOME/.local/bin"; fi
fi
mkdir -p "$dir" || err "cannot create $dir" "impossible de créer $dir"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT INT TERM

name="freeport-$version-$target"
url="$BASE_URL/download/$version/$name.tar.gz"
say "Installing freeport $version ($target) into $dir" "Installation de freeport $version ($target) dans $dir"
download "$url" "$tmp/freeport.tar.gz" \
  || err "download failed: does version $version exist for $target?" "téléchargement impossible : la version $version existe-t-elle pour $target ?"

if [ "${FREEPORT_SKIP_VERIFY:-}" = 1 ]; then
  warn "SHA-256 check skipped (FREEPORT_SKIP_VERIFY=1)" "vérification SHA-256 ignorée (FREEPORT_SKIP_VERIFY=1)"
elif download "$url.sha256" "$tmp/freeport.sha256"; then
  expected="$(cut -d' ' -f1 "$tmp/freeport.sha256" | head -n 1 | tr 'A-F' 'a-f')"
  case "$expected" in
    *[!0-9a-f]*|"") err "malformed checksum file for $version" "fichier d'empreinte invalide pour $version" ;;
  esac
  [ "${#expected}" -eq 64 ] || err "malformed checksum file for $version" "fichier d'empreinte invalide pour $version"
  actual="$(sha256_of "$tmp/freeport.tar.gz")" \
    || err "no tool to compute SHA-256 (install sha256sum, shasum or openssl, or set FREEPORT_SKIP_VERIFY=1)" \
           "aucun outil pour calculer le SHA-256 (installez sha256sum, shasum ou openssl, ou définissez FREEPORT_SKIP_VERIFY=1)"
  [ "$actual" = "$expected" ] \
    || err "SHA-256 mismatch, refusing to install (expected $expected, got $actual)" \
           "SHA-256 différent, installation refusée (attendu $expected, obtenu $actual)"
  say "SHA-256 verified" "SHA-256 vérifié"
elif ver_ge "${version#v}" 0.2.0; then
  err "no checksum is published for $version, refusing to install" "aucune empreinte publiée pour $version, installation refusée"
else
  warn "no checksum is published for $version (releases before 0.2.0 have none), continuing" \
       "aucune empreinte publiée pour $version (les versions antérieures à 0.2.0 n'en ont pas), poursuite"
fi

tar xzf "$tmp/freeport.tar.gz" -C "$tmp" || err "invalid archive" "archive invalide"
install -m 755 "$tmp/$name/freeport" "$dir/freeport" 2>/dev/null \
  || { cp "$tmp/$name/freeport" "$dir/freeport" && chmod 755 "$dir/freeport"; } \
  || err "cannot write to $dir (try FREEPORT_INSTALL_DIR=\$HOME/.local/bin)" "impossible d'écrire dans $dir (essayez FREEPORT_INSTALL_DIR=\$HOME/.local/bin)"

"$dir/freeport" --version
case ":$PATH:" in
  *":$dir:"*) ;;
  *)
    say "
Add $dir to your PATH to use 'freeport':" "
Ajoutez $dir à votre PATH pour utiliser « freeport » :"
    printf '  export PATH="%s:$PATH"\n' "$dir"
    ;;
esac
