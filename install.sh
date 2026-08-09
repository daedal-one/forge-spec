#!/bin/sh

set -eu

repository=${FORGE_SPEC_GIT_URL:-https://github.com/daedal-one/forge-spec.git}
cargo_bin=${CARGO:-cargo}
git_bin=${GIT:-git}

if ! command -v "$cargo_bin" >/dev/null 2>&1; then
    printf '%s\n' "error: Cargo is required to install spec." >&2
    printf '%s\n' "Install Rust from https://rustup.rs/ and run this installer again." >&2
    exit 1
fi

if ! command -v "$git_bin" >/dev/null 2>&1; then
    printf '%s\n' "error: Git is required to download forge-spec." >&2
    exit 1
fi

install_dir=$(mktemp -d "${TMPDIR:-/tmp}/forge-spec-install.XXXXXX")
cleanup() {
    rm -rf "$install_dir"
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM

printf 'Installing spec from %s\n' "$repository"
if [ -n "${FORGE_SPEC_GIT_REF:-}" ]; then
    "$git_bin" clone --depth 1 --branch "$FORGE_SPEC_GIT_REF" --no-recurse-submodules \
        "$repository" "$install_dir/forge-spec"
else
    "$git_bin" clone --depth 1 --no-recurse-submodules \
        "$repository" "$install_dir/forge-spec"
fi
"$cargo_bin" install --path "$install_dir/forge-spec/spec-cli" --locked

if command -v spec >/dev/null 2>&1; then
    printf 'Installed %s\n' "$(spec --version)"
else
    printf '%s\n' "Installed spec, but it is not currently on PATH." >&2
    printf '%s\n' "Add Cargo's bin directory to PATH, then run: spec --version" >&2
fi
