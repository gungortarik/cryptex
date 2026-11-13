# Disabled Mac combined supervisor-loss acceptance fixture

Scope: ADR-0021's isolated development/test path only. Both actual supervisors
must be gone while a real engine remains alive; an old ready identity must not
authorize restart/adoption. This is not production orphan cleanup or Windows Job
equivalence. Normal helper runs and native Observe/Stop wire operations do not
gain a guardian, a kill request or new renderer capability.

## Containment and injection

Only the combined-loss driver selects a separate fixed helper fixture purpose.
After real engine initialization, that helper captures a same-image guardian
child in its existing private session/group. A separate native purpose, exact
parent/child image, epoch/release/root and kernel group/session checks authenticate
the guardian before it arms. The parent consumes ARMED before exposing readiness;
the guardian retains its native inbox throughout its armed lifetime. Its
inherited stdin is an anonymous pipe, not a terminal or arbitrary control path.
No credential, managed-profile lease or canonical writer is delegated to it.

After identity readiness the driver pauses for a fixed X byte. The collector
captures the full file/marker baseline and live engine rows BEFORE either loss;
the guardian does not read stdin while its helper parent is alive. Only after
that baseline does the collector send X. The daemon worker kills its exact helper, observes its exit with
waitid(WNOWAIT), and parks retaining that Child without normal cleanup/Drop. The
collector then kills only its own captured Popen daemon. Read-only group evidence
and the guardian's retained parent liveness establish helper loss separately.
The guardian is explicitly an additional test-containment process, not either
supervisor and not an engine counted in the live-engine assertion.

Before releasing cleanup, require two non-zombie engine processes distinct from
the guardian, unchanged active marker/profile/config/cert/key/receipts and two
refused replacement attempts. A fixed signed read-only integrity driver checks
the existing OS credential against the unchanged binding before and after; it
must acquire the same exclusive profile lease while the guardian is alive. That
also detects accidental inheritance of the helper's lease. No regeneration,
marker removal or credential deletion is a success path for replacement.

## Cleanup and evidence

Only after those assertions does the collector send the fixed cleanup byte.
EOF, invalid input or a fixed 120-second guardian watchdog also terminate the
guardian's own current group using `kill(0, SIGKILL)`. [Apple's public kill(2)
documentation](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/kill.2.html)
defines PID zero as the sender's current process group. No numeric PID/PGID from
ps, file metadata or a stale snapshot becomes a signal target. The guardian never
changes group/session after authentication. Direct invocation refuses before
arming group cleanup.

The collector reaps its driver, observes the group gone and only then uses the
existing signed, profile-lease-fenced API-key cleanup. An unresolved wait/group
or unsafe metadata fails and preserves evidence; there is no PID adoption or raw
Keychain deletion fallback. Group containment does not promise automatic Keychain
cleanup if the collector itself is killed.

The new MAC-BIRLIKTE one-click package runs only this new scenario. It returns an
aggregate receipt with the exact source and separate scenario/credential/runtime
booleans. Required acceptance: source review, exact-source native Mac/Windows CI
and execution of the exported launcher, followed by one owner-Mac result. Earlier
Mac identity/pairing owner receipts remain accepted without an unchanged rerun.
