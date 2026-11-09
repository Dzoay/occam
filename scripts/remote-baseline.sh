#!/usr/bin/env bash
# Runs `tools/baseline/baseline.py` on another machine over ssh, because timings need a quiet machine and the timing host
# has more physical cores than this one. Everything happens under ~/.cache/occam-remote/occam-census on the host, and
# the run holds the shared speed lock there so that only one timing runs at a time; split a long measurement into stretches
# of about 15 minutes by group and thread count. The host needs uv and rsync, and node for the WebAssembly numbers. The
# results are copied back to this machine's cache. The host is an argument so that no machine's name is written into the
# repository.
#
# Usage: scripts/remote-baseline.sh <host> run --groups booleans --threads 1,2
#        scripts/remote-baseline.sh <host> node --groups booleans
set -euo pipefail

base=.cache/occam-remote/occam-census

host=${1:-}
if [ -z "$host" ] || [ $# -lt 2 ]; then
  echo "usage: $0 <host> run|node [options]" >&2
  exit 2
fi
shift

cd "$(dirname "${BASH_SOURCE[0]}")/.."

# on_host <command>: runs it in a login shell on the host.
on_host() {
  ssh "$host" "bash -lc ${1@Q}"
}

on_host "mkdir -p $base/mirror $base/xdg"
rsync -a --delete --filter=':- .gitignore' tools/ "$host:$base/mirror/tools/"

environment="UV_CACHE_DIR=\$HOME/$base/uv-cache UV_PYTHON_INSTALL_DIR=\$HOME/$base/uv-python"
environment+=" UV_PROJECT_ENVIRONMENT=\$HOME/$base/venv XDG_CACHE_HOME=\$HOME/$base/xdg"
environment+=" OCCAM_BASELINE_OCJS=\$HOME/$base/ocjs"
on_host "cd $base/mirror && export $environment && flock \$HOME/.cache/occam-remote/speed.lock uv run --frozen --project tools/oracle python tools/baseline/baseline.py ${*@Q}"

results=${XDG_CACHE_HOME:-$HOME/.cache}/occam-baseline/results
mkdir -p "$results"
rsync -a "$host:$base/xdg/occam-baseline/results/" "$results/"
