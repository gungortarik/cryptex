# macOS P0-2 update compatibility follow-up — test request

**Status:** REQUESTED; not yet run  
**Scope:** Verify the architecture-neutral synthetic Desktop/daemon protocol
pair rejection and existing daemon update/rollback fixture on macOS. This does
not test or select ADR-0012.

Run this after the update-compatibility change is available on
`m0/p0-2-cursor`:

```bash
set -euo pipefail
cd "$HOME/github_repos/cryptex"
git fetch origin
git checkout m0/p0-2-cursor
git pull --ff-only origin m0/p0-2-cursor
cargo test -p cryptexd --locked
cargo build -p cryptexd --locked

DAEMON="$PWD/target/debug/cryptexd"
RUNTIME="$HOME/Library/Application Support/Cryptex-Dev/p0-2/compare/macos-update-compat-20261003"
test ! -e "$RUNTIME"
mkdir -p "$RUNTIME/measurements"
"$DAEMON" lifecycle update-proof \
  --base-bin "$DAEMON" \
  --runtime-root "$RUNTIME" \
  --experiment-id macos-update-compat-20261003 \
  | tee "$RUNTIME/measurements/macos-update-compat-20261003.kv"
```

Confirm the output and retained KV include:

```text
incompatible_pair_rejected=true
active_identity_after_incompatible_pair=v1
compatible_pair_accepted=true
compatible_identity=v2
compatible_marker_unchanged=true
failed_detected=true
failed_marker_unchanged=true
rollback_identity=v2
rollback_marker_unchanged=true
final_instance_count=0
```

Report the source commit, macOS version/architecture, complete test summary, the
retained measurement path and process exit code. If all fields match and the
test command passes, add a macOS evidence note to this directory and commit the
evidence. Do not edit the implementation as part of this validation request.

The protocol pair is test metadata: the fixture does not replace or launch an
actual Desktop package. Keep that limitation in any PASS statement. Runtime and
measurement files must remain under `Cryptex-Dev/p0-2`; do not touch P0-1 paths.
