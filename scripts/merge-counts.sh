#!/bin/sh
# git's merge driver for ci/test-counts.txt (L44), set up by `just setup`: `merge-counts.sh %O %A %B`. `cargo xtask
# merge-counts` merges the minimums per binary. Where that does not decide, or xtask does not build mid-rebase, git merges
# the text, leaving conflict markers.
cargo xtask merge-counts "$1" "$2" "$3" 2>/dev/null && exit 0
exec git merge-file -L ours -L base -L theirs "$2" "$1" "$3"
