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
# What gives `web-test`'s headless Chromium (156, Playwright 1.64.1) its software WebGPU adapter. Without the first flag
# it gives none. Compositing through GL, every read-back after a canvas frame fails ("A valid external Instance
# reference no longer exists"), so the next three make it composite through Vulkan on its own SwiftShader, which needs
# no Vulkan driver on the machine (CI has none). The last asks for SwiftShader's adapter, whatever GPU the machine has.
webgpu_flags := "--enable-unsafe-webgpu --enable-features=Vulkan --use-vulkan=swiftshader --use-angle=swiftshader --use-webgpu-adapter=swiftshader"

[private]
default:
    @just --list --unsorted

# Install whichever gate tools are missing, safe to repeat
[group('setup')]
setup:
    scripts/toolchain.sh install

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

# What to do next: each open milestone's steps that can start, by its diagram
[env("CARGO_TARGET_DIR", target)]
[group('plan')]
next *args:
    cargo xtask next "$@"

# Tests, or those nextest's arguments pick: `just test -p occam-xtask docs`
[env("CARGO_TARGET_DIR", target)]
[group('test')]
test *args:
    cargo nextest run "$@"

# The hybrid on P5's 625-component surface against a plane, all 577 curves. Minutes in a debug build, so nightly, not
# per PR (docs/PLAN.md section 5)
[env("CARGO_TARGET_DIR", target)]
[group('test')]
hybrid-many-loops *args:
    cargo nextest run -p occam-intersect p5s_625_component_surface --run-ignored only --no-capture "$@"

# Format the Rust code
[group('check')]
fmt:
    cargo fmt --all

# The viewer's npm packages, as web/package-lock.json pins them, installed again only when the lock is newer
[private]
web-deps:
    if [ ! web/node_modules/.package-lock.json -nt web/package-lock.json ]; then npm ci --prefix web --no-audit --no-fund; fi

# Build the viewer with esbuild into crates/occam-cli/page, which `occam` embeds as `occam view`'s page when next built, or into the directory given; its workers' kernel is occam-wasm, built for the browser with simd128
[env("CARGO_TARGET_DIR", target)]
[group('build')]
web-build *args: web-deps
    CARGO_TARGET_DIR="$CARGO_TARGET_DIR/simd" RUSTFLAGS="-Ctarget-feature=+simd128" cargo build --quiet -p occam-wasm --release --target wasm32-unknown-unknown
    wasm-bindgen --target web --out-dir "$CARGO_TARGET_DIR/occam-web" "$CARGO_TARGET_DIR/simd/wasm32-unknown-unknown/release/occam_wasm.wasm"
    OCCAM_WASM_JS="$CARGO_TARGET_DIR/occam-web" node web/build.mjs "$@"

# Type-check docs/sketch, the compiled sketch of the seams in docs/ARCHITECTURE.md, which no workspace build includes
[env("CARGO_TARGET_DIR", target)]
[group('check')]
sketch:
    cargo check --manifest-path docs/sketch/Cargo.toml

# Formatting, unchanged
[group('gates')]
fmt-check:
    cargo fmt --all --check

# Clippy on every crate and target, warnings denied, and on occam-topo's and occam-mesh's determinism probes, which only a feature builds
[env("CARGO_TARGET_DIR", target)]
[group('gates')]
clippy:
    cargo clippy --workspace --all-targets -- -D warnings
    cargo clippy -p occam-topo --all-targets --features determinism -- -D warnings
    cargo clippy -p occam-mesh --all-targets --features determinism -- -D warnings

# The viewer's types, strict and declaration files included, against @types/web's, with `tsc --noEmit`; esbuild builds it without checking them
[group('gates')]
web-check: web-deps
    web/node_modules/.bin/tsc --noEmit -p web

# The viewer's tests in headless Chromium on its software adapter, asserting buffers, not pixels, its reports against the catalogue, its workers' meshes against the native `occam mesh`, the page `occam view` serves, built in, following the model file, and its clicks against `occam pick`; and the demo with the agent's calls scripted
[env("CARGO_TARGET_DIR", target)]
[env("OCCAM_WEBGPU_FLAGS", webgpu_flags)]
[group('gates')]
web-test: web-build
    mkdir -p "$CARGO_TARGET_DIR"
    cargo run --quiet -p occam-report --example catalogue > "$CARGO_TARGET_DIR/occam-catalogue.json"
    cargo build --quiet -p occam-cli
    OCCAM_CATALOGUE="$CARGO_TARGET_DIR/occam-catalogue.json" OCCAM="$CARGO_TARGET_DIR/debug/occam" node --test --test-timeout=300000 web/test/viewer.test.mjs web/test/demo.test.mjs web/test/pick-timing.test.mjs

# The live agent test, by hand on your own key and account, never in CI: headless Claude Code on Haiku, pinned, with the occam skill, builds a plate with a hole as headless Chromium follows the page, then stands a cylinder on the face selected there; scored as the scripted demo is, everything kept for docs/evidence/. `--dry-run`: a stand-in plays the agent, and no model is asked; `--score KIT --why REASON`: a kit scored again from its logs
[env("CARGO_TARGET_DIR", target)]
[env("OCCAM_WEBGPU_FLAGS", webgpu_flags)]
[group('tools')]
agent-demo *args: web-build
    cargo build --quiet -p occam-cli
    OCCAM="$CARGO_TARGET_DIR/debug/occam" node web/test/demo/live.mjs "$@"

# What headless Chromium gives the viewer's tests: its WebGPU adapter, its GPU status, and whether a read-back works
[env("OCCAM_WEBGPU_FLAGS", webgpu_flags)]
[group('tools')]
web-gpu: web-deps
    node web/test/gpu-report.mjs

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
    node crates/occam-wasm/js/session.test.mjs "$CARGO_TARGET_DIR/occam-wasm-js/occam_wasm.js"
    node crates/occam-wasm/js/mesh.test.mjs "$CARGO_TARGET_DIR/occam-wasm-js/occam_wasm.js"
    cargo check --workspace --target wasm32-unknown-unknown
    CARGO_TARGET_DIR="$CARGO_TARGET_DIR/simd" RUSTFLAGS="-Ctarget-feature=+simd128" cargo check --workspace --exclude occam-xtask --target wasm32-unknown-unknown
    cargo check --workspace --all-targets --target x86_64-pc-windows-msvc

# One gate, or `all`: `just gate docs`, `just gate counts --base origin/main`, `just gate all --except counts`
[env("CARGO_TARGET_DIR", target)]
[group('gates')]
gate *args="all":
    cargo xtask gates "$@"

# The Python tools' tests, standard library only
[group('gates')]
tools-test: corpus-test
    python3 -m unittest discover -s tools/census/tests
    python3 -B -m unittest discover -s tools/determinism/tests -t tools/determinism
    python3 -B -m unittest discover -s tools/coefficients/tests -t tools/coefficients

# What takes seconds and builds only occam-xtask: formatting, the gates that read the repository, and the mutants gate's check that every exclusion still excludes a mutant; a worker runs it before committing
[group('gates')]
preflight: fmt-check (gate "all" "--except" "deps,errors,determinism,counts,mutants,perf") (gate "mutants" "--exclusions")

# Everything except mutants and perf, the quick checks first: `preflight` and the viewer's types, then clippy, the tests, wasm, the viewer's tests and the gates that build or run something
[group('gates')]
gates: preflight web-check clippy test-all wasm tools-test web-test (gate "all" "--except" "layers,complexity,provenance,docs,plan,seam,mutants,perf")

# occam-geom's frozen seam against ci/seam.txt; `just seam --update docs/decisions/<decision>.md`: re-record it under it
[env("CARGO_TARGET_DIR", target)]
[group('gates')]
seam *args:
    cargo xtask gates seam "$@"

# Instruction counts per operation class, and browser timing; `just perf --update`: rewrite the record
[env("CARGO_TARGET_DIR", target)]
[group('gates')]
perf *args:
    cargo xtask gates perf "$@"

# Mutation testing on the Rust changed since origin/main. `--shard 1/4`: one slice of it; `--iterate`: skip the mutants a run here caught
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

# Intersection's hits on every corpus case's operands, the box test against the unculled scan by digest, printing the
# totals. Minutes in a debug build, so nightly, not per PR (docs/PLAN.md section 5)
[env("CARGO_TARGET_DIR", target)]
[group('corpus')]
corpus-hits *args:
    cargo nextest run -p occam-verify --test corpus_hits --run-ignored only --no-capture "$@"

# The mesher's adversarial catalogue's slow case, P9's plate with 1,024 holes: minutes in a debug build, so nightly
[env("CARGO_TARGET_DIR", target)]
[group('test')]
mesh-holes *args:
    cargo nextest run -p occam-ops --lib --run-ignored only --no-capture adversarial::a_plate_with_1024_holes "$@"

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

# Time loading a model of about 10⁵ faces from the native format, parsing and checking apart: `construction`'s goal
[env("CARGO_TARGET_DIR", target)]
[group('tools')]
load *args:
    cargo run --quiet --release -p occam-io --example load -- "$@"

# The `view` milestone's goals in headless Chromium, as JSON lines: `pool <model> [workers...]`, the page's mesh workers at 1 to 6, cold and warm; `editing`, the editing worker's call on construction's requests; `redraw <model> [rounds]`, from a run's save to the page's redraw. A model from `just view-timing save`
[env("CARGO_TARGET_DIR", target)]
[env("OCCAM_WEBGPU_FLAGS", webgpu_flags)]
[group('tools')]
view-timing-browser *args: web-build
    cargo build --quiet --release -p occam-cli
    OCCAM="$CARGO_TARGET_DIR/release/occam" node web/test/timing.mjs "$@"

# A pick timed on this machine's real GPU, `<model> [chromium|firefox] [--headed] [--workers N]`, on a model from `just view-timing save`: headed, opening a browser window, with `--headed` or where headless reaches only a software adapter, and refused on a software adapter. A goal, not a gate
[env("CARGO_TARGET_DIR", target)]
[group('tools')]
view-pick-gpu *args: web-build
    cargo build --quiet --release -p occam-cli
    OCCAM="$CARGO_TARGET_DIR/release/occam" node web/test/pick-timing.mjs "$@"

# The `view` milestone's native goals on P9's models (`build`, `quality`, `scale`, `save`, `open`): its quality table, its speed-up at 1 to 6 threads, and its models saved for `occam view`. Goals, not gates
[env("CARGO_TARGET_DIR", target)]
[group('tools')]
view-timing *args:
    cargo run --quiet --release -p occam --example view_timing -- "$@"

# The determinism stream's record, from its separate implementation; `--check` compares it with ci/determinism-hashes.txt
[group('tools')]
determinism-record *args:
    python3 -B tools/determinism/stream.py "$@"

# The topology's determinism record, as the x86-64 build prints it on one thread: it has no separate implementation
[env("CARGO_TARGET_DIR", target)]
[group('tools')]
determinism-topology:
    cargo run --quiet --release -p occam-topo --features determinism --example determinism -- --record > ci/determinism-topology.txt.new
    mv ci/determinism-topology.txt.new ci/determinism-topology.txt

# The mesher's determinism record, as the x86-64 build prints it on one thread: it has no separate implementation
[env("CARGO_TARGET_DIR", target)]
[group('tools')]
determinism-mesh:
    cargo run --quiet --release -p occam-mesh --features determinism --example determinism -- --record > ci/determinism-mesh.txt.new
    mv ci/determinism-mesh.txt.new ci/determinism-mesh.txt

# Regenerate the owned trigonometry's fitted coefficients and constants; `--check` says whether the committed file is current
[group('tools')]
coefficients *args:
    python3 -B tools/coefficients/fit.py "$@"

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
