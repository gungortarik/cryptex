# Incoming validation entry point — 2026-11-07

## Implemented behavior

The daemon now exposes an explicit batch-validation command using the existing
bounded staging and locked M3/M4 plus M8 policy verifiers.

```text
cryptexd replication validate-incoming --runtime-root <absolute-runtime> --environment <dev|prod> <relative-segment>...
```

Inputs are sealed segment paths relative to that runtime's `transport/incoming`
directory, with the existing `<device-id-hex>/<20-digit-index>.seg` shape. The
command requires the normal daemon to be stopped. It retains the instance lock
through cleanup, opens the existing projection read-only, loads its local trust
anchor through the selected OS secure-store environment, snapshots the batch,
releases the staging slot before canonical locks, verifies the current retrieval
and post-import trust/cutoff gates, and discards unprepared staging afterward.

On success it reports JSON with `outcome: validated_only`, segment/event counts,
and `published: false`. Validation can create the existing import machinery's
lock directories. No segment move or prepared import manifest is issued.
Errors propagate through the daemon CLI, including staging cleanup failures.
Existing prepared or unknown journal state is preserved for recovery.

## Verification on the Windows development host

- Five targeted library tests passed, covering valid validation/cleanup,
  two segments from one device in reverse input order, signature rejection
  followed by a valid retry, refusal of an unapproved candidate device, and
  live-instance/prepared-state refusal.
- One targeted binary argument test passed, requiring an explicit environment
  and nonempty candidate list.
- Daemon library/binary/tests strict Clippy passed. Rust formatting and diff
  whitespace checks passed.
- Fixtures use a fake secure store and synthetic events. These results cover
  this command's orchestration on Windows, with a valid local-author stream.
  Native peer import, macOS, and the full daemon suite were not run here.

## Continuation boundary

`import_staging::publish_verified_batch` explicitly remains unwired until its
publication/durability/recovery gates pass. ADR-0019's owner-approved publication
flow in section 5.1.1 authorizes implementation direction while preserving those
activation gates. This entry point advances validation through the real daemon
CLI without enabling the canonical publisher.

For the native nonzero-cutoff restriction test, remaining work includes sealing
and transporting a peer's approved stream, verifying it with the recipient's
actual local trust binding, completing publication/recovery evidence and review,
and then publishing through the daemon. The validation report alone does not
make the peer's records available to the native restriction snapshot builder.
