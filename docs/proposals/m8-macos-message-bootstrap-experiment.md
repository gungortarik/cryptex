# M8 public message bootstrap: disposable mechanism experiment

Status: **authorized isolated research, not a product adapter** (2026-11-10).
The owner selected Option A in the [carrier reassessment](m8-macos-launch-carrier-reassessment.md).
Accepted Unix IPC, Mac owner mutation gates and product minimum OS are unchanged.

Phase-one mechanism result: [reviewed exact-source CI passed](../../evidence/m8/m8-macos-public-bootstrap-mechanism-20261110.md)
13 native rows, required fault controls and independent forced-exit cleanup on
source8816c04. This is the ad-hoc mechanism scope below, not the remaining signed
two-binary, hostile-endpoint or product/session acceptance.

## Phase one and trust boundary

First establish that public `bootstrap_check_in` can create a fresh transient
service in a disposable CI process, that its own spawned child can look it up,
and that simple Mach messages carry the actual sender audit trailer both ways.
No launchd plist, login item, service installation, task registration, inherited
token descriptor, private header or deprecated register API is used. Failure of
dynamic registration ends this experiment on that host, without a fallback.

The random service name and session bytes are nonsecret routing/freshness data,
not authentication. The parent records its actual posix_spawn Child PID before
receiving; the child records getppid. A receiver obtains the sender token only
from the kernel audit trailer, jointly validates PID/token through Security and
the exact fixture image hash, retains a task-name right across the exchange,
then validates again after acquisition and on every subsequent frame. Public
PID and generation getters are used; no opaque token slots or mutable token
byte-string comparisons. An exit, changed parent or changed generation refuses.

Phase one uses an explicitly **ad-hoc signed CI fixture**, a fixed fixture code
identifier and same-image hash. That tests process/message binding, not a Cryptex
publisher, certified organization, sealed product bundle or release signature.
Its receipt always sets product activation, certified-bundle acceptance, owner
session acceptance and M8 completion false. No owner package is produced here.

## Exchange and lifetime

1. Parent checks in a new service, then spawns a fixed fixture with only public
   mode/name/session arguments and explicit standard streams. No inherited Mach
   registration list is modified and no synthetic control FD is passed.
2. Child creates its own reply receive right, looks up the untrusted name and
   sends a fixed-size HELLO with a send-once reply right in the Mach header.
3. Parent requires the captured Child PID and current image/generation, then
   replies to the send-once right. Child authenticates the kernel sender as its
   actual live parent, retaining that instance right.
4. A second sequenced frame and reply must pass the same retained pins; duplicate
   sequence, wrong session, malformed or complex messages refuse. Invalid frames
   never advance the sequence or replace the pin.
5. All receives/sends and waits have deadlines. Every owned child is reaped;
   send-once rights on rejected messages are destroyed, and retained pins and
   receive/send rights are released on all normal paths. A separate lookup
   process must observe UNKNOWN_SERVICE after normal close and forced process
   exit. Closing a receive right alone is not called namespace cleanup.

Failed sends may return port rights under changed names. This disposable fixture
therefore immediately exits on send failure (typed timeout/other code), with the
owned-group Python supervisor providing process cleanup. This process-exit policy
is not a proposed product recovery implementation. Kill/wait completion and all
tracked normal right-release calls must succeed; injected setup failure cannot be
counted as an expected refusal. Required native controls exercise those boundaries.

## Required phase-one observations

Public registration/lookup and two-way pinned exchange; active-name collision
refusal without takeover; same-image other process with forged claimed PID
refused followed by the actual child succeeding; replay, wrong session,
short/complex message refusal; same-PID exec generation refusal; queued message
after child exit refused; fresh child/session exchange; distinct supervisor
timeout; independent normal/forced-exit namespace removal. Reports must preserve
typed observed stage/code rather than an expected-outcome label alone.

## Phase-two adversarial fixtures (native verification pending)

The root process checks in a separate control service and captures its helper's
actual spawn PID. The helper checks in an attack service, sends a READY frame,
and accepts configuration only from its actual parent authenticated through the
kernel trailer. Root authenticates that captured helper and spawns the victim
itself; the victim's getppid therefore remains the real root. Only then does root
send the exact captured victim PID as authenticated helper configuration.

Three fixed fixtures preserve this control/data separation:

- Substituted endpoint: victim discovers the helper's service. Helper validates
  the actual configured victim HELLO and sends a correctly formed ACK over its
  real reply right, claiming the root PID. Only victim exit WRONG_PID52 plus
  helper normal exit0 can pass; timeout/layout/setup refusal cannot.
- Transferred reply: root validates the victim HELLO, then moves its real
  send-once reply right to the authenticated helper via a one-descriptor control
  message. The helper sends the valid-looking ACK. Victim must again reject the
  actual kernel sender specifically as WRONG_PID52.
- Transferred service: root copies an actual service send right to the helper.
  Helper sends the exact valid next sequence/session/op with the victim's claimed
  PID. Root requires WRONG_PID52 and unchanged retained pin/generation/sequence;
  the legitimate victim must subsequently succeed using sequence2.

Control messages have a distinct message ID, fixed operation/sequence/session,
and an exact one-right descriptor layout. Send-once rights move; ordinary send
rights copy. A received descriptor is detached before message destruction, and
the donor field is cleared only after a successful move. All fifteen owned
control/data/attack service names require independent removal checks.

These fixtures are still CI-only ad-hoc mechanism tests. The own-image versus
peer-image selector is trusted local policy state, never an imported field; it
prepares separate Desktop/helper and daemon roles without accepting signatures
from an untrusted sibling. Certified two-binary policy remains a separate gate.

Remaining research includes native verification of these attack fixtures,
concurrent attacker flooding/deadline budgeting, transferred send rights, bundle
replacement/snapshot consistency, signed two-binary policy, and the actual product
owner-session lifecycle. Prior raw-Mach/signing receipts do not satisfy these.
Successful startup binding also does not solve steady-state Unix forwarded-FD
or per-frame authorization. No phase-one pass enables owner mutations.

## Public-source basis

[Apple's public bootstrap interface](https://github.com/apple-oss-distributions/launchd/blob/main/liblaunch/bootstrap.h)
and [non-strict public wrapper](https://github.com/apple-oss-distributions/launchd/blob/main/liblaunch/libbootstrap.c)
support checking the operation, while historical
[launchd implementation](https://github.com/apple-oss-distributions/launchd/blob/main/src/core.c)
contains dynamic registration and dead-name cleanup. These are design evidence,
not proof that a current sandbox/bootstrap context allows it. Disposable native
CI must demonstrate actual registration, lookup and independent removal first.
