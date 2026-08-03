#!/usr/bin/env bash
set -euo pipefail

dist="${1:-dist}"

for archive in \
    "$dist"/*.tar.gz \
    "$dist"/*.deb \
    "$dist"/*.pkg \
    "$dist"/*.msi; do
    [[ -f "$archive" ]] || continue

    if command -v sha256sum >/dev/null 2>&1; then
        checksum="$(sha256sum "$archive" | cut -d ' ' -f 1)"
    else
        checksum="$(shasum -a 256 "$archive" | cut -d ' ' -f 1)"
    fi
    printf '%s  %s\n' "$checksum" "$(basename "$archive")" > "${archive}.sha256"
done
