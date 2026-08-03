#!/usr/bin/env bash

set -Eeuo pipefail

readonly DEFAULT_REMOTE="${RELEASE_REMOTE:-origin}"
readonly DEFAULT_BRANCH="${RELEASE_BRANCH:-mvp}"
readonly EXAMPLE_ENTRY="examples/futao-2-full-stack/baseline-1.0/main.ft"
readonly EXAMPLE_EXPECTED="examples/futao-2-full-stack/baseline-1.0/expected.stdout"

remote="$DEFAULT_REMOTE"
release_branch="$DEFAULT_BRANCH"
publish=false
dry_run=false
skip_checks=false
install_root=""

cleanup() {
    if [[ -n "$install_root" && -d "$install_root" ]]; then
        rm -rf "$install_root"
    fi
}

trap cleanup EXIT

fail() {
    printf 'release: error: %s\n' "$*" >&2
    exit 1
}

info() {
    printf 'release: %s\n' "$*"
}

usage() {
    cat <<'EOF'
Usage: scripts/release.sh [OPTIONS]

Validate the release candidate on mvp. The script never creates or pushes a
tag unless --publish is supplied.

Options:
  --publish             Create and push the matching annotated version tag.
  --dry-run             Run all checks without creating or pushing a tag.
  --skip-checks         Skip release-check and install smoke (dry-run only).
  --remote NAME         Git remote to use (default: origin).
  --branch NAME         Release branch to require (default: mvp).
  -h, --help            Show this help.

Environment:
  RELEASE_REMOTE        Default value for --remote.
  RELEASE_BRANCH        Default value for --branch.
EOF
}

while (($# > 0)); do
    case "$1" in
        --publish)
            publish=true
            ;;
        --dry-run)
            dry_run=true
            ;;
        --skip-checks)
            skip_checks=true
            ;;
        --remote)
            shift
            (($# > 0)) || fail "--remote requires a name"
            remote="$1"
            ;;
        --branch)
            shift
            (($# > 0)) || fail "--branch requires a name"
            release_branch="$1"
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            fail "unknown option: $1 (use --help for usage)"
            ;;
    esac
    shift
done

if [[ "$publish" == true && "$dry_run" == true ]]; then
    fail "--publish and --dry-run cannot be combined"
fi

if [[ "$publish" == true && "$skip_checks" == true ]]; then
    fail "--skip-checks is only allowed for a non-publishing preflight"
fi

command -v git >/dev/null 2>&1 || fail "git is required"
command -v cargo >/dev/null 2>&1 || fail "cargo is required"

repo_root="$(git rev-parse --show-toplevel 2>/dev/null)" \
    || fail "run this script inside the repository"
cd "$repo_root"

current_branch="$(git symbolic-ref --quiet --short HEAD 2>/dev/null || true)"
[[ "$current_branch" == "$release_branch" ]] \
    || fail "release must run on $release_branch (current: ${current_branch:-detached})"

[[ -z "$(git status --porcelain=v1)" ]] \
    || fail "worktree is not clean; commit or stash changes first"

git remote get-url "$remote" >/dev/null 2>&1 \
    || fail "git remote does not exist: $remote"

info "fetching $remote/$release_branch and tags"
git fetch --quiet "$remote" "$release_branch" --tags

local_head="$(git rev-parse HEAD)"
remote_head="$(git rev-parse "$remote/$release_branch" 2>/dev/null || true)"
[[ -n "$remote_head" ]] || fail "remote branch does not exist: $remote/$release_branch"
[[ "$local_head" == "$remote_head" ]] \
    || fail "local $release_branch is not synchronized with $remote/$release_branch"

package_id="$(cargo pkgid --locked -p nexac)"
version="${package_id##*#}"
[[ "$version" =~ ^0\.0\.[1-9][0-9]*(-[0-9A-Za-z.-]+)?$ ]] \
    || fail "unsupported nexac version: $version (expected 0.0.x, x >= 1)"

tag="v$version"
if git rev-parse --verify --quiet "refs/tags/$tag" >/dev/null; then
    fail "local tag already exists: $tag"
fi
if git ls-remote --exit-code --tags "$remote" "refs/tags/$tag" >/dev/null 2>&1; then
    fail "remote tag already exists: $remote/$tag"
fi

info "candidate nexac $version at $local_head"

run_install_smoke() {
    install_root="$(mktemp -d "${TMPDIR:-/tmp}/nexac-release.XXXXXX")"
    cargo install --locked --path crates/nexac --root "$install_root"

    local installed="$install_root/bin/nexac"
    [[ -x "$installed" ]] || fail "installed nexac binary was not created"
    [[ "$("$installed" --version)" == "nexac $version" ]] \
        || fail "installed nexac version does not match $version"
    [[ "$("$installed" check "$EXAMPLE_ENTRY")" == "ok" ]] \
        || fail "installed nexac failed the .ft check smoke"

    local actual expected
    actual="$("$installed" run "$EXAMPLE_ENTRY")"
    expected="$(<"$EXAMPLE_EXPECTED")"
    [[ "$actual" == "$expected" ]] \
        || fail "installed nexac .ft run output differs from $EXAMPLE_EXPECTED"
}

if [[ "$skip_checks" == true ]]; then
    info "skipping release-check and install smoke by request"
else
    info "running cargo xtask release-check"
    cargo xtask release-check
    info "running installed .ft smoke"
    run_install_smoke
fi

if [[ "$dry_run" == true || "$publish" == false ]]; then
    info "preflight passed; no tag was created"
    info "publish after the release PR is merged with: scripts/release.sh --publish"
    exit 0
fi

info "creating annotated tag $tag"
git tag -a "$tag" -m "nexac $version"

if ! git push "$remote" "$tag"; then
    fail "tag $tag was created locally but could not be pushed; retry git push $remote $tag"
fi

info "pushed $tag; GitHub Release workflow will build and publish the prerelease"
