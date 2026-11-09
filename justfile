# Common tasks for occam. `just` lists them by group; every recipe runs from the repository root. CI runs the same
# commands. The gates are defined in docs/PLAN.md section 5.

set shell := ["bash", "-euo", "pipefail", "-c"]
set positional-arguments := true
set ignore-comments := true
set lazy := true

# This checkout's shared cargo target directory (L48), asked for once per `just` run, only by the recipes that run cargo.
target := `scripts/target-dir.sh`
# On disk, not in a RAM-backed /tmp: cargo-mutants copies the tree there and builds each copy.
tmp := env("XDG_CACHE_HOME", env("HOME") / ".cache") / "occam-tmp"
# The OCCT wheel's virtualenv lives outside the repository too (docs/decisions/2026-11-09-occt-oracle.md).
oracle_venv := env("XDG_CACHE_HOME", env("HOME") / ".cache") / "occam-oracle" / "venv"

[private]
default:
    @just --list --unsorted

# Install whichever gate tools are missing and set up the test counts' merge driver, safe to repeat
[group('setup')]
setup:
    scripts/toolchain.sh install
    git config merge.occam-counts.name "ci/test-counts.txt, per test binary"
    git config merge.occam-counts.driver "scripts/merge-counts.sh %O %A %B"

# Check that the gate tools are installed, changing nothing
[group('setup')]
toolchain:
    scripts/toolchain.sh check

# Debug build of every crate
[env("CARGO_TARGET_DIR", target)]
[group('build')]
build:
    cargo build --workspace

# The API docs of every crate, intra-doc links checked
[env("CARGO_TARGET_DIR", target)]
[env("RUSTDOCFLAGS", "-D rustdoc::broken_intra_doc_links -D rustdoc::private_intra_doc_links")]
[group('build')]
doc *args:
    cargo doc --workspace --no-deps "$@"

# What to do next: each open milestone's first unfinished step
[env("CARGO_TARGET_DIR", target)]
[group('plan')]
next *args:
    cargo xtask next "$@"

# Tests, or those nextest's arguments pick: `just test -p occam-xtask docs`
[env("CARGO_TARGET_DIR", target)]
[group('test')]
test *args:
    cargo nextest run "$@"

# Format the Rust code
[group('check')]
fmt:
    cargo fmt --all

# Formatting, unchanged
[group('gates')]
fmt-check:
    cargo fmt --all --check

# Clippy on every crate and target, warnings denied
[env("CARGO_TARGET_DIR", target)]
[group('gates')]
clippy:
    cargo clippy --workspace --all-targets -- -D warnings

# Every test on the host, then the doctests of any library
[env("CARGO_TARGET_DIR", target)]
[group('gates')]
test-all:
    cargo nextest run --workspace --profile ci
    if cargo metadata --no-deps --format-version 1 | grep -q '"kind":\["lib"\]'; then cargo test --doc --workspace; fi

# The tests on wasm32-wasip2, xtask aside, baseline and simd128; the browser target, both builds; and a Windows check (no MSVC linker here)
[env("CARGO_TARGET_DIR", target)]
[group('gates')]
wasm:
    if ls crates | grep -qvx occam-xtask; then cargo nextest run --workspace --exclude occam-xtask --target wasm32-wasip2 --profile ci; else echo "wasm: no crate but occam-xtask yet, so no tests to run on wasm32-wasip2"; fi
    CARGO_TARGET_DIR="$CARGO_TARGET_DIR/simd" RUSTFLAGS="-Ctarget-feature=+simd128" cargo nextest run --workspace --exclude occam-xtask --target wasm32-wasip2 --profile ci
    cargo build -p occam-wasm --release --target wasm32-unknown-unknown
    wasm-bindgen --target nodejs --out-dir "$CARGO_TARGET_DIR/occam-wasm-js" "$CARGO_TARGET_DIR/wasm32-unknown-unknown/release/occam_wasm.wasm"
    node crates/occam-wasm/js/distance.test.mjs "$CARGO_TARGET_DIR/occam-wasm-js/occam_wasm.js"
    cargo check --workspace --target wasm32-unknown-unknown
    CARGO_TARGET_DIR="$CARGO_TARGET_DIR/simd" RUSTFLAGS="-Ctarget-feature=+simd128" cargo check --workspace --exclude occam-xtask --target wasm32-unknown-unknown
    cargo check --workspace --all-targets --target x86_64-pc-windows-msvc

# One gate, or `all`: `just gate docs`, `just gate all --except counts`, `just gate plan --json`
[env("CARGO_TARGET_DIR", target)]
[group('gates')]
gate *args="all":
    cargo xtask gates "$@"

# The survey scripts' tests, Python standard library only
[group('gates')]
tools-test: corpus-test
    python3 -m unittest discover -s tools/census/tests

# Everything except mutants and perf
[group('gates')]
gates: fmt-check clippy test-all wasm tools-test (gate "all" "--except" "mutants")

# Mutation testing on the Rust changed since origin/main; `just mutants --shard 1/4`: one slice of it
[env("CARGO_TARGET_DIR", target)]
[env("TMPDIR", tmp)]
[group('mutants')]
mutants *args:
    mkdir -p "$TMPDIR"
    cargo xtask gates mutants "$@"

# Regenerate tools/corpus/generated from its seeds, and print the family table
[group('corpus')]
corpus-generate *args:
    python3 -B tools/corpus/generate.py "$@"

# Download the locked corpus sources (the NIST parts) into ~/.cache/occam-corpus, each verified against the lock
[group('corpus')]
corpus-fetch *args:
    python3 -B tools/corpus/fetch.py "$@"

# The corpus pipeline's own tests; they never touch the network
[group('corpus')]
corpus-test *args:
    python3 -B -m unittest discover -s tools/corpus/tests -t tools/corpus "$@"

# Regenerate tools/corpus/imported from a clone of arris at the locked commit, taking only what the per-case gate allows
[group('corpus')]
corpus-import-arris repo:
    python3 -B tools/corpus/import_arris.py "$1"

# Install what `just mutants-on` needs on another machine over ssh, at this machine's versions
[group('mutants')]
mutants-setup host:
    scripts/remote-mutants.sh setup "$1"

# `just mutants` on another machine over ssh, on a mirror of this checkout; `OCCAM_MUTANTS_JOBS=6`: six at once
[group('mutants')]
mutants-on host *args:
    scripts/remote-mutants.sh run "$@"

# List every report code and remedy as JSON
[env("CARGO_TARGET_DIR", target)]
[group('tools')]
errors:
    cargo run --quiet -p occam-report --example catalogue

# Check locked dependencies against the RustSec advisories; the nightly workflow runs this
[group('tools')]
advisories:
    cargo deny check advisories

# Lint the GitHub workflows, after editing them
[group('tools')]
workflows:
    actionlint

# Raise ci/test-counts.txt's minimums to this tree's
[env("CARGO_TARGET_DIR", target)]
[group('tools')]
counts:
    cargo xtask gates counts --update

# OCCT's whole-operation timings on the generated corpus: the numbers occam has to beat. `just baseline run`, `node`, `report`
[env("UV_PROJECT_ENVIRONMENT", oracle_venv)]
[group('baseline')]
baseline *args:
    uv run --frozen --project tools/oracle python tools/baseline/baseline.py "$@"

# `just baseline` on another machine over ssh, on a mirror of `tools/`, under the shared speed lock, results copied back
[group('baseline')]
baseline-on host *args:
    scripts/remote-baseline.sh "$@"

# The baseline harness's own tests, standard library only
[group('baseline')]
baseline-test *args:
    python3 -B -m unittest discover -s tools/baseline/tests -t tools/baseline "$@"

# The oracle harness (`just oracle run tools/oracle/examples`, then `check`, `compare`, `register`, `build-second`); a large download the first time
[env("UV_PROJECT_ENVIRONMENT", oracle_venv)]
[group('oracle')]
oracle *args:
    uv run --frozen --project tools/oracle python tools/oracle/oracle.py "$@"

# The oracle harness's own tests, through the pinned OCCT wheel; the nightly workflow runs this
[env("UV_PROJECT_ENVIRONMENT", oracle_venv)]
[group('oracle')]
oracle-test:
    uv run --frozen --project tools/oracle python -m unittest discover -s tools/oracle/tests -t tools/oracle
