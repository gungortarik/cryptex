#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
usage: run-local-macos.sh <events> <payload_bytes> <segment_events>

<events> must be one of: 10000 100000 500000
All generated data stays under:
  ~/Library/Application Support/Cryptex-Dev/benchmarks/p0-1/
EOF
}

if [[ $# -ne 3 ]]; then
  usage
  exit 2
fi

events="$1"
payload_bytes="$2"
segment_events="$3"

case "$events" in
  10000|100000|500000) ;;
  *)
    echo "error: events must be 10000, 100000, or 500000" >&2
    exit 2
    ;;
esac

for value in "$payload_bytes" "$segment_events"; do
  if [[ ! "$value" =~ ^[1-9][0-9]*$ ]]; then
    echo "error: payload_bytes and segment_events must be positive integers" >&2
    exit 2
  fi
done

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../../.." && pwd)"
runtime_root="$HOME/Library/Application Support/Cryptex-Dev/benchmarks/p0-1"
stamp="$(date -u +%Y%m%dT%H%M%SZ)"
run_root="$runtime_root/macos-${events}-${stamp}"
results_dir="$run_root/results"
data_dir="$run_root/data"

mkdir -p "$results_dir" "$data_dir"

cd "$repo_root"

if [[ -n "$(git status --porcelain)" ]]; then
  echo "error: benchmark requires a clean Git worktree" >&2
  exit 2
fi

cargo build --locked --release -p m0-storage-bench
bench="$repo_root/target/release/m0-storage-bench"

if [[ ! -x "$bench" ]]; then
  echo "error: benchmark binary not found: $bench" >&2
  exit 2
fi

syncthing_version="$(syncthing --version 2>/dev/null || printf 'unavailable')"

{
  echo "platform=macos"
  echo "events=$events"
  echo "payload_bytes=$payload_bytes"
  echo "segment_events=$segment_events"
  echo "git_commit=$(git rev-parse HEAD)"
  echo "git_branch=$(git rev-parse --abbrev-ref HEAD)"
  echo "rustc=$(rustc --version)"
  echo "cargo=$(cargo --version)"
  echo "os_product_version=$(sw_vers -productVersion)"
  echo "os_build_version=$(sw_vers -buildVersion)"
  echo "arch=$(uname -m)"
  echo "syncthing_version=$syncthing_version"
  echo "run_root=$run_root"
} > "$results_dir/metadata.txt"

run_layout() {
  local layout="$1"
  local root="$data_dir/$layout-$events"
  local output="$results_dir/$layout-$events.txt"

  if [[ "$layout" == "files" ]]; then
    "$bench" generate files "$events" "$payload_bytes" "$root" | tee "$output"
  else
    "$bench" generate segments "$events" "$payload_bytes" "$root" "$segment_events" | tee "$output"
  fi

  local allocated_kib
  allocated_kib="$(du -sk "$root" | awk '{print $1}')"
  echo "allocated_kib_du=$allocated_kib" | tee -a "$output"

  "$bench" scan "$layout" "$root" > "$results_dir/$layout-$events-rescan.txt"
}

run_layout files
run_layout segments

"$bench" crash-probe files "$payload_bytes" "$data_dir/crash-files" \
  > "$results_dir/crash-files.txt"
"$bench" crash-probe segments "$payload_bytes" "$data_dir/crash-segments" \
  > "$results_dir/crash-segments.txt"

cat <<EOF
P0-1 local macOS run complete.
Results: $results_dir
Data:    $data_dir

Do not delete the data yet if it will be used for the Syncthing phase.
This run is evidence only; it does not select the production layout.
EOF
