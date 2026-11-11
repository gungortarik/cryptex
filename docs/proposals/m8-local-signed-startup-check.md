# Local signed Desktop/daemon startup check

This is the default-off, headless continuation of ADR-0020. The owner authorized
the existing fee-free local certificate profile for actual Desktop-to-daemon
startup. It does not activate Mac owner operations or select a distribution
publisher.

The `m8-local-startup-check` Cargo feature is absent from ordinary builds.
It requires macOS and an explicit build-time certificate DER pin. Missing,
oversized or invalid certificate input fails the build; Cargo tracks both
the selection and file content. The pin is embedded before signing the two
distinct role images. Runtime arguments provide locators, never trust policy.

The feature-only Desktop entry runs before Tauri. It exclusively creates a
fresh temporary runtime, verifies a CMS inventory against the embedded pin,
and retains the verified content snapshot. It uses the ordinary Desktop
lifecycle-hold command constructor to launch the actual daemon. The daemon's
feature-only hook runs before ordinary lifecycle/runtime initialization.
Both independently verify the same inventory and content snapshot.

Before the short native handshake starts, the child emits one fixed public
verification-progress hint through its private stdout pipe, flushes it and
restores stdout to a discard sink. The parent reads exactly that bounded hint
and EOF nonblockingly under the original outer startup deadline. This is only
scheduling progress: arbitrary programs can emit it, and it grants no image,
ready, request or owner authority. Missing, extra, partial or late bytes fail
closed. Every existing native sender/image/epoch/frame check must still pass.
The 2.5-second native handshake and 10/15-second outer budgets are unchanged;
child inventory verification no longer consumes the native HELLO wait itself.
Each retained verified release caches its immutable frontend catalog once;
fresh restart observations never share that cache.

The Desktop opens a fresh rendezvous before spawning. Native checks retain
the actual child/parent instance and validate the opposite role identifier,
certificate leaf and signed image hash. The only exchanged application frames
are 54 bytes: magic, version, role-bound phase, native epoch and inventory
SHA-256. They have no owner-operation, capability or domain fields.
Neither peer accepts a different inventory observation.
After ordinary lifecycle initialization and ready-file publication, the daemon
sends a separate authenticated runtime-ready phase. The Desktop validates that
phase against the same live process, epoch, inventory and remaining budget
before reporting startup success.

The Desktop owns the snapshot, native channel and exact child through
authentication, strict absolute-deadline readiness and cleanup. The daemon
retains its channel/snapshot across its ordinary lifecycle and closes it before
exiting. An existing runtime is refused. Crash/restart uses a fresh verified
inventory observation, child and epoch; the old live channel must refuse.

The hosted-Mac fixture builds and locally signs both real executables and the
inventory inside its disposable keychain scope, without OS trust writes.
It tests startup, forced child exit, restart, old-channel refusal and clean
shutdown. Negatives cover changed listed content, wrong release/hash/role,
absent signing leaf, missing rendezvous and wrong live parent role/image hash.
Each refusal must expose its expected fixed stage code, not only an exit status.
Shared readiness tests
include a complete PID file observed only after the absolute budget expired.
Native short-budget tests leave HELLO/ACK unanswered and ensure caller budget
is retained rather than opening a new handshake wait. Synchronous platform
identity calls still cannot be forcibly preempted; deadlines are rechecked
before sending or publishing.

This establishes only the tested headless signed-process startup binding.
The content snapshot does not make an executable path atomic or seal an
existing webview's resources. The startup-only command constructs no authenticated owner
request, pending approval grant or native domain dispatcher. Product factories,
ordinary Unix owner refusals and Windows policies remain unchanged. Tauri
protected-content consumption and isolated approval/session integration are
now covered by the separate [default-off continuation](m8-local-content-approval-check.md).
Signed interactive acceptance and product activation remain open.
