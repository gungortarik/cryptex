# P0-2 Windows Mode B — INVALID HARNESS EVIDENCE

**Date (UTC):** 2026-10-02  
**Host:** Windows physical machine  
**Branch:** `m0/p0-2-cursor`  
**Classification:** FAILED / INVALID HARNESS EVIDENCE  
**Not a lifecycle/supervision failure**

## Summary

The first physical Windows Mode B run is recorded as **invalid harness evidence**, not as a
failure of Mode B supervision semantics or of single-instance locking.

## What the run did verify (partial, non-decisive)

| Step | Observed |
|---|---|
| Windows build | passed |
| Daemon start | succeeded |
| Duplicate daemon rejection | correctly rejected (exit 1 / already running) |

## Where the harness failed

After successful start + duplicate rejection, the Mode B comparison path called
`observe_instance()` while the live daemon still held the exclusive `daemon.lock`.

Observed error:

```text
The process cannot access the file because another process has locked a portion of the file. (os error 33)
```

That is Windows `ERROR_LOCK_VIOLATION` (33). Root cause: `observe_instance()` previously
read `daemon.lock` PID bytes *before* probing the exclusive lock. On Windows, reading the
locked portion fails while the holder owns the fs2 exclusive lock. This path was not
exposed on macOS.

## Consequence for remaining Mode B fields

Because observation aborted, the remaining crash / restart / window-close fields stayed
`NOT_YET_VERIFIED` for this attempt. Those gaps are **not** evidence that Mode B lifecycle
behavior failed on Windows; the harness could not continue past the observation bug.

## Disposition

- Status: **INVALID HARNESS EVIDENCE**
- Do **not** treat this run as Mode B lifecycle failure
- Do **not** use it to select or reject ADR-0012 Mode A/B
- Safe to **rerun Windows Mode B** after the observe-without-locked-byte-read fix lands

## Fix required before rerun

`observe_instance()` must determine lock state without reading bytes currently locked by
another process. While the lock is held, holder PID must come from the existing
`lifecycle.status` side-channel. Exclusive locking itself must remain authoritative and
unchanged.