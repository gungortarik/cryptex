# macOS Update / Rollback Protocol (P0-2 test-only)

**Status:** VERIFIED for synthetic fixtures (see `macos-update-rollback-evidence.md`)  
**ADR-0012:** UNRESOLVED (do not accept from this protocol)

## Purpose

Prove a deterministic compatible update + failed update + rollback path for a
supervised `cryptexd` using **synthetic staged binaries** only.

This is not a production installer/updater and does not select Mode A, Mode B,
or hybrid supervision.

## Fixture identities

| Identity | Role |
|---|---|
| `v1` | initial known-good staged binary (copy of built `cryptexd`) |
| `v2` | compatible replacement (same binary family, new identity) |
| `v_bad` | intentionally non-starting fixture (shell script exiting 3) |

## Runtime layout (outside repository)

Default proof root:

`~/Library/Application Support/Cryptex-Dev/p0-2/compare/update-rollback`

Staging tree under that root:

```
update-bin/
  active/     cryptexd-fixture + identity
  previous/   last-known-good
  staging/    candidate awaiting activate
  failed/     retained bad active after rollback
synthetic-canonical.marker
```

## Supervisor mode for this proof

`desktop_supervisor` (Mode B process harness) is used only as the restart
owner for the proof. This does **not** freeze ADR-0012.

## Stage contract

Record for each stage:

- experiment ID
- branch / commit
- runtime root
- supervisor mode
- binary / version identity
- daemon PID
- startup / restart timestamps
- compatibility / start result
- marker digest before / after
- rollback attempted / success
- final version
- final daemon count
- errors

## Commands

```bash
cargo build -p cryptexd -p m0-lifecycle-compare
CRYPTEXD="$(cargo metadata --locked --format-version 1 --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')/debug/cryptexd"
COMPARE="$(dirname "$CRYPTEXD")/m0-lifecycle-compare"

"$COMPARE" run-update-rollback \
  --experiment-id macos-update-rollback-1 \
  --cryptexd "$CRYPTEXD" \
  --branch m0/p0-2-cursor \
  --commit "$(git rev-parse --short HEAD)"
```

Equivalent CLI:

```bash
"$CRYPTEXD" lifecycle update-proof \
  --base-bin "$CRYPTEXD" \
  --runtime-root "$HOME/Library/Application Support/Cryptex-Dev/p0-2/compare/update-rollback" \
  --experiment-id macos-update-rollback-1
```

## Expected outcomes

1. INITIAL: identity=`v1`, instance count=1, marker present
2. COMPATIBLE: stage+activate `v2`, restart succeeds, identity=`v2`, instance=1, marker unchanged
3. FAILED: stage+activate `v_bad`, start fails, no duplicate daemon, marker unchanged, previous retained
4. ROLLBACK: restore previous (`v2`), start succeeds, instance=1, marker unchanged, status truthful

## Explicit non-goals

- production updater UX / packaging
- Windows proof (separate)
- login / reboot persistence
- ADR-0012 acceptance
- real canonical event storage safety beyond the synthetic marker fixture
