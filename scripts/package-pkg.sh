#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 3 ]]; then
    printf 'usage: %s VERSION BINARY OUTPUT\n' "$0" >&2
    exit 2
fi

version="$1"
binary="$2"
output="$3"

command -v pkgbuild >/dev/null 2>&1 || {
    printf 'pkgbuild is required to build the macOS package\n' >&2
    exit 1
}

staging="$(mktemp -d)"
trap 'rm -rf "$staging"' EXIT

mkdir -p "$staging/usr/local/bin" "$staging/usr/local/share/doc/nexac"
install -m 0755 "$binary" "$staging/usr/local/bin/nexac"
install -m 0644 README.md "$staging/usr/local/share/doc/nexac/README.md"
install -m 0644 LICENSE "$staging/usr/local/share/doc/nexac/LICENSE"

mkdir -p "$(dirname "$output")"
pkgbuild \
    --root "$staging" \
    --identifier com.baicie.tao.nexac \
    --version "$version" \
    --install-location / \
    "$output"
