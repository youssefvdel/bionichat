#!/usr/bin/env bash
# Run the CEF build without sudo.
#
# Chromium refuses to start unless its SUID sandbox helper is root-owned and
# mode 4755. `cargo build` (CEF default) drops a NON-setuid copy next to the
# binary, which Chromium then finds and rejects. Two ways out:
#
#   a) sudo chown root:root target/debug/chrome-sandbox && sudo chmod 4755 ...
#   b) hide that copy and point CHROME_DEVEL_SANDBOX at the root-owned helper the
#      CEF build already installed under target/cef/debug. This script does (b).
#
# Usage: scripts/run-cef.sh [-- extra app args]
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
helper="$root/target/cef/debug/chrome-sandbox"
killer="$root/target/debug/chrome-sandbox"

if [[ ! -u "$helper" ]]; then
  echo "missing setuid helper: $helper" >&2
  echo "build it first: cargo build   (CEF is the default)" >&2
  exit 1
fi

# Chromium prefers the helper beside the executable; a non-setuid copy there is
# fatal, so move it out of the way (target/ is ignored, nothing is lost).
if [[ -e "$killer" ]] && [[ ! -u "$killer" ]]; then
  mv "$killer" "$killer.unsetuid-$$"
  echo "moved non-setuid helper aside: $killer.unsetuid-$$"
fi

export CHROME_DEVEL_SANDBOX="$helper"
exec "$root/target/debug/bionichat" "$@"
