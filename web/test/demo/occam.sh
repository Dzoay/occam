#!/usr/bin/env bash
# The demo's `occam`, first on the agent's PATH. As in the construction blind run
# (docs/evidence/2026-11-13-construction-blind-run.md), it logs each call outside the agent's directory, its arguments,
# the request files it names, its output and its exit status, then runs the real occam; past the calls a prompt allows,
# it refuses. The harness sets OCCAM_DEMO_REAL (the real command), OCCAM_DEMO_LOG, OCCAM_DEMO_PROMPT (which prompt is
# running) and OCCAM_DEMO_CALLS (how many calls a prompt allows).
set -u
real=${OCCAM_DEMO_REAL:?the harness names the real occam}
log=${OCCAM_DEMO_LOG:?the harness names the log}
prompt=${OCCAM_DEMO_PROMPT:-1}
limit=${OCCAM_DEMO_CALLS:-12}
used=$(grep -c "^=== call [0-9]* prompt $prompt " "$log" 2>/dev/null || true)
used=${used:-0}
all=$(grep -c '^=== call ' "$log" 2>/dev/null || true)
now=$(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)
if [ "$used" -ge "$limit" ]; then
  {
    printf '=== refused prompt %s %s\n' "$prompt" "$now"
    printf 'argv:'; printf ' %q' "$@"; printf '\n'
  } >>"$log"
  echo "occam: this prompt allows $limit calls, and all $limit are used; the call is refused." >&2
  exit 3
fi
out=$(mktemp) err=$(mktemp)
"$real" "$@" >"$out" 2>"$err"
status=$?
{
  printf '=== call %d prompt %s %s\n' "$((${all:-0} + 1))" "$prompt" "$now"
  printf 'cwd: %s\n' "$PWD"
  printf 'argv:'; printf ' %q' "$@"; printf '\n'
  for a in "$@"; do
    case "$a" in
      *.json) if [ -f "$a" ]; then printf -- '--- file %s\n' "$a"; head -c 65536 "$a"; printf '\n'; fi ;;
    esac
  done
  printf -- '--- stdout\n'; head -c 262144 "$out"; printf '\n'
  printf -- '--- stderr\n'; head -c 65536 "$err"; printf '\n'
  printf -- '--- exit %d\n' "$status"
} >>"$log"
cat "$out"
cat "$err" >&2
rm -f "$out" "$err"
exit "$status"
