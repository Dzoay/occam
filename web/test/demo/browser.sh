#!/usr/bin/env bash
# What `occam view` opens its page with in the demo, as $BROWSER names it: it notes the address in OCCAM_DEMO_OPENED,
# for headless Chromium to open as the person's browser would, and opens nothing on the machine.
printf '%s\n' "$1" >>"${OCCAM_DEMO_OPENED:?the harness names where the address goes}"
