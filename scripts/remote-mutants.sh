#!/usr/bin/env bash
# Runs the mutants gate on another machine over ssh, because it is the slowest gate and it scales with cores.
#
# `setup` installs the same versions of just, cargo-nextest and cargo-mutants as this machine has, in ~/.local on the host.
# The host needs rustc, cargo, clippy and rsync. `run` mirrors this checkout, uncommitted work included, into a directory
# on the host that exists only for this, then runs `just mutants` there and exits with its status. Gitignored paths such
# as private/ are never copied. The host is an argument so that no machine's name is written into the repository.
#
# OCCAM_MUTANTS_JOBS, if set here, is passed on as the number of mutants to test at once.
#
# Usage: scripts/remote-mutants.sh setup <host>
#        scripts/remote-mutants.sh run <host> [--shard k/n]
set -euo pipefail

mirror=.cache/occam-remote/occam

mode=${1:-}
host=${2:-}
if [ -z "$host" ] || { [ "$mode" != setup ] && [ "$mode" != run ]; }; then
  echo "usage: $0 setup|run <host> [--shard k/n]" >&2
  exit 2
fi
shift 2

cd "$(dirname "${BASH_SOURCE[0]}")/.."

# on_host <command>: runs it in a login shell on the host, with ~/.local/bin on PATH.
on_host() {
  ssh "$host" "bash -lc ${1@Q}"
}

case "$mode" in
  setup)
    tools="just@$(just --version | awk '{print $2}')"
    tools+=" cargo-nextest@$(cargo nextest --version | awk 'NR==1 {print $2}')"
    tools+=" cargo-mutants@$(cargo mutants --version | awk '{print $2}')"
    on_host "cargo install --locked --root \"\$HOME/.local\" $tools"
    ;;
  run)
    on_host "mkdir -p $mirror"
    rsync -a --delete --filter=':- .gitignore' ./ "$host:$mirror/"
    on_host "export PATH=\"\$HOME/.local/bin:\$PATH\"${OCCAM_MUTANTS_JOBS:+ OCCAM_MUTANTS_JOBS=$OCCAM_MUTANTS_JOBS}; cd $mirror && just mutants ${*@Q}"
    ;;
esac
