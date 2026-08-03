#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 3 ]]; then
    printf 'usage: %s VERSION BINARY OUTPUT\n' "$0" >&2
    exit 2
fi

version="$1"
binary="$2"
output="$3"

command -v dpkg-deb >/dev/null 2>&1 || {
    printf 'dpkg-deb is required to build the Debian package\n' >&2
    exit 1
}

staging="$(mktemp -d)"
trap 'rm -rf "$staging"' EXIT

mkdir -p "$staging/DEBIAN" "$staging/usr/bin" "$staging/usr/share/doc/nexac"
install -m 0755 "$binary" "$staging/usr/bin/nexac"
install -m 0644 README.md "$staging/usr/share/doc/nexac/README.md"
install -m 0644 LICENSE "$staging/usr/share/doc/nexac/LICENSE"

architecture="$(dpkg --print-architecture)"
mkdir -p "$(dirname "$output")"
cat > "$staging/DEBIAN/control" <<EOF
Package: nexac
Version: $version
Section: devel
Priority: optional
Architecture: $architecture
Maintainer: baicie <zl316546@gmail.com>
Description: Nexa bootstrap compiler command-line interface
 The nexac compiler checks and runs Nexa and Futao source files.
EOF

dpkg-deb --build --root-owner-group "$staging" "$output" >/dev/null
