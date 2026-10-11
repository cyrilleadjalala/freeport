#!/bin/sh
# Tests install.sh offline against a fake release served locally.
#   sh tests/install_test.sh
set -eu

root="$(cd "$(dirname "$0")/.." && pwd)"
work="$(mktemp -d)"
trap 'kill "$srv" 2>/dev/null || true; rm -rf "$work"' EXIT INT TERM

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64)              target=x86_64-unknown-linux-musl ;;
  Linux-aarch64|Linux-arm64) target=aarch64-unknown-linux-musl ;;
  Darwin-x86_64)             target=x86_64-apple-darwin ;;
  Darwin-arm64)              target=aarch64-apple-darwin ;;
  *) echo "unsupported platform"; exit 1 ;;
esac

sha() { if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | cut -d' ' -f1; else shasum -a 256 "$1" | cut -d' ' -f1; fi; }

# make_release VERSION: a fake release whose "freeport" prints its version.
make_release() {
  v="$1"; name="freeport-$v-$target"
  mkdir -p "$work/srv/download/$v" "$work/build/$name"
  printf '#!/bin/sh\necho "freeport %s"\n' "${v#v}" > "$work/build/$name/freeport"
  chmod +x "$work/build/$name/freeport"
  tar czf "$work/srv/download/$v/$name.tar.gz" -C "$work/build" "$name"
}
add_sum() { f="$work/srv/download/$1/freeport-$1-$target.tar.gz"; echo "$(sha "$f")  $(basename "$f")" > "$f.sha256"; }

make_release v0.1.9            # old release: no checksum published
make_release v0.2.0; add_sum v0.2.0   # good release
make_release v0.2.1            # new release but checksum missing
make_release v0.2.2; add_sum v0.2.2   # then replaced by a different, valid archive
printf '#!/bin/sh\necho tampered\n' > "$work/build/freeport-v0.2.2-$target/freeport"
tar czf "$work/srv/download/v0.2.2/freeport-v0.2.2-$target.tar.gz" -C "$work/build" "freeport-v0.2.2-$target"
make_release v0.2.3; echo "not-a-hash" > "$work/srv/download/v0.2.3/freeport-v0.2.3-$target.tar.gz.sha256"

port=$(python3 -c 'import socket;s=socket.socket();s.bind(("127.0.0.1",0));print(s.getsockname()[1])')
(cd "$work/srv" && exec python3 -m http.server "$port" --bind 127.0.0.1 >/dev/null 2>&1) &
srv=$!
i=0; while ! curl -fs "http://127.0.0.1:$port/" >/dev/null 2>&1; do i=$((i+1)); [ $i -gt 50 ] && { echo "server did not start"; exit 1; }; sleep 0.1; done

fail=0
check() { if [ "$1" = ok ]; then echo "ok   - $2"; else echo "FAIL - $2"; fail=1; fi; }

# run VERSION [ENV=VALUE...]: install.sh into a fresh dir; sets $out and $status.
run() {
  v="$1"; shift
  rm -rf "$work/bin"
  out="$(env FREEPORT_BASE_URL="http://127.0.0.1:$port" FREEPORT_VERSION="$v" FREEPORT_INSTALL_DIR="$work/bin" FREEPORT_LANG=en "$@" sh "$root/install.sh" 2>&1)" && status=0 || status=$?
}

run v0.2.0
[ $status -eq 0 ] && [ -x "$work/bin/freeport" ] && echo "$out" | grep -q "SHA-256 verified" && echo "$out" | grep -q "freeport 0.2.0"
check "$([ $? -eq 0 ] && echo ok)" "valid release installs and is verified"

run v0.2.2
[ $status -ne 0 ] && [ ! -e "$work/bin/freeport" ] && echo "$out" | grep -q "mismatch"
check "$([ $? -eq 0 ] && echo ok)" "corrupted archive is refused"

run v0.2.2 FREEPORT_SKIP_VERIFY=1
[ $status -eq 0 ] && [ -x "$work/bin/freeport" ] && echo "$out" | grep -q "skipped"
check "$([ $? -eq 0 ] && echo ok)" "FREEPORT_SKIP_VERIFY=1 skips the check"

run v0.2.1
[ $status -ne 0 ] && [ ! -e "$work/bin/freeport" ] && echo "$out" | grep -q "no checksum is published"
check "$([ $? -eq 0 ] && echo ok)" "missing checksum is refused from 0.2.0 on"

run v0.1.9
[ $status -eq 0 ] && [ -x "$work/bin/freeport" ] && echo "$out" | grep -q "before 0.2.0 have none"
check "$([ $? -eq 0 ] && echo ok)" "old release without checksum installs with a warning"

run v0.2.3
[ $status -ne 0 ] && echo "$out" | grep -q "malformed"
check "$([ $? -eq 0 ] && echo ok)" "malformed checksum file is refused"

run v9.9.9
[ $status -ne 0 ] && echo "$out" | grep -q "download failed"
check "$([ $? -eq 0 ] && echo ok)" "unknown version gives a clear error"

run v0.2.0 FREEPORT_LANG=fr
[ $status -eq 0 ] && echo "$out" | grep -q "SHA-256 vérifié"
check "$([ $? -eq 0 ] && echo ok)" "messages follow FREEPORT_LANG=fr"

exit $fail
