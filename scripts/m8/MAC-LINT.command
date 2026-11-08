#!/bin/bash
set -eu
base="$(cd "$(dirname "$0")" && pwd)"
exec /bin/bash "$base/MAC-TEST.command" --lint-only
