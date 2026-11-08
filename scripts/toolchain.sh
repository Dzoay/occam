#!/usr/bin/env bash
# The tools occam's gates need, in one list. `install` adds what is missing and is safe to repeat. `check` changes nothing.
# System packages need sudo, so this script names the command and leaves running it to the user.
#
# Usage: scripts/toolchain.sh check|install
set -euo pipefail

# wasm-bindgen-cli and gungraun-runner must be the same version as the crates of those names.
WASM_BINDGEN=0.2.129
GUNGRAUN=0.20.0
PLAYWRIGHT=1.64.1
REUSE=6.2.0

# binary:pacman-package
SYSTEM_TOOLS=(just:just rustup:rustup uv:uv node:nodejs npm:npm valgrind:valgrind)
# binary:crate, with @version where it is pinned
CARGO_TOOLS=(
  cargo-nextest:cargo-nextest
  cargo-mutants:cargo-mutants
  cargo-deny:cargo-deny
  cargo-shear:cargo-shear
  wasmtime:wasmtime-cli
  "wasm-bindgen:wasm-bindgen-cli@$WASM_BINDGEN"
  "gungraun-runner:gungraun-runner@$GUNGRAUN"
)
COMPONENTS=(clippy rustfmt)
TARGETS=(wasm32-unknown-unknown wasm32-wasip2 x86_64-pc-windows-msvc)

mode=${1:-}
case "$mode" in
  check | install) ;;
  *)
    echo "usage: $0 check|install" >&2
    exit 2
    ;;
esac

failures=0
ok() { printf 'ok    %s\n' "$*"; }
fail() {
  printf 'FAIL  %s\n' "$*"
  failures=$((failures + 1))
}
has() { command -v "$1" >/dev/null 2>&1; }

# has_version <binary> <version or empty>: the binary is on PATH and, when a version is given, reports it.
has_version() {
  has "$1" || return 1
  [ -z "$2" ] || [[ $("$1" --version 2>&1) == *"$2"* ]]
}

for entry in "${SYSTEM_TOOLS[@]}"; do
  bin=${entry%%:*}
  if has "$bin"; then
    ok "$bin"
  else
    fail "$bin: not found (run: sudo pacman -S ${entry#*:})"
  fi
done

for entry in "${CARGO_TOOLS[@]}"; do
  bin=${entry%%:*}
  crate=${entry#*:}
  version=""
  if [[ $crate == *@* ]]; then
    version=${crate#*@}
  fi
  if has_version "$bin" "$version"; then
    ok "$bin${version:+ $version}"
  elif [ "$mode" = check ]; then
    fail "$bin${version:+ $version}: missing or the wrong version (run: just setup)"
  else
    cargo install --locked --root "$HOME/.local" "$crate"
    if has_version "$bin" "$version"; then
      ok "$bin${version:+ $version}, installed"
    else
      fail "$bin: installed in ~/.local/bin but not found (is it on PATH?)"
    fi
  fi
done

# rustup_item <component|target> <prefix pattern> <name>: the component or target is installed.
rustup_item() {
  local kind=$1 pattern=$2 name=$3 installed
  installed=$(rustup "$kind" list --installed)
  if grep -q "$pattern" <<<"$installed"; then
    ok "$kind $name"
  elif [ "$mode" = check ]; then
    fail "$kind $name: not installed (run: just setup)"
  else
    rustup "$kind" add "$name"
    ok "$kind $name, installed"
  fi
}

if has rustup; then
  for component in "${COMPONENTS[@]}"; do
    rustup_item component "^$component" "$component"
  done
  for target in "${TARGETS[@]}"; do
    rustup_item target "^$target\$" "$target"
  done
fi

# Browser timing runs in Playwright's Chromium, which lives in a cache shared with other projects.
if compgen -G "$HOME/.cache/ms-playwright/chromium-*" >/dev/null; then
  ok "Playwright's Chromium"
elif [ "$mode" = check ]; then
  fail "Playwright's Chromium: not installed (run: just setup)"
elif has npx; then
  npx --yes "playwright@$PLAYWRIGHT" install chromium
  ok "Playwright's Chromium, installed"
else
  fail "Playwright's Chromium: npx not found"
fi

# reuse runs through uvx, so nothing is installed. `install` fills uv's cache so that later runs need no network.
if has uv; then
  if [ "$mode" = install ]; then
    uvx "reuse==$REUSE" --version >/dev/null
  fi
  ok "reuse $REUSE, through uvx"
fi

if ((failures > 0)); then
  printf '\n%d check(s) failed.\n' "$failures"
  exit 1
fi
printf '\nAll the gate tools are present.\n'
