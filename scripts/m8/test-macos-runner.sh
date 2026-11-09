#!/bin/bash
# Test only the real summary/exit helper, without invoking native Mac actions.
set -euo pipefail
base="$(cd "$(dirname "$0")" && pwd)"
work="$(mktemp -d "${TMPDIR:-/tmp}/cryptex-mac-runner-test.XXXXXX")"
report="$work/result.txt"
eval "$(sed -n '/^run_tests() {/,/^}/p' "$base/MAC-TEST.command")"
fake_cargo() { printf '%s\n' "$fake_summary"; return "$fake_exit"; }
rustup_bin=fake_cargo
fake_exit=0
failures=0
good='test result: ok. 5 passed; 0 failed; 0 ignored; 578 filtered out; finished in 0.10s'
fake_summary="$good"
run_tests valid unused 5
[ "$failures" -eq 0 ]
for fake_summary in '' 'test result: ok. 0 passed; 0 failed; 0 ignored;' 'test result: ok. 5 passed; 0 failed; 1 ignored;' "$(printf '%s\n%s' "$good" "$good")"; do
    before="$failures"
    run_tests invalid unused 5
    [ "$failures" -eq "$((before + 1))" ]
done
fake_summary="$good"
run_tests wrong_count unused 6
[ "$failures" -eq 5 ]
fake_exit=7
run_tests exit_failure unused 5
[ "$failures" -eq 6 ]
echo 'PASS: Mac summary helper rejects empty, zero, ignored, duplicate, wrong-count and nonzero-exit results'
