# Disabled protected UI and native approval continuation

This extends the explicitly selected local-certificate startup experiment under
ADR-0020. Ordinary builds and all ordinary Mac product owner routes remain
closed. No release publisher, new platform floor or product activation is chosen.

## Content

The signed inventory lists actual built frontend files. After the complete
snapshot verifies, only members under Contents/Resources/frontend become an
immutable shared content catalog. The actual Tauri Context installs an Assets
provider over those bytes and discards the embedded provider. It clears devUrl,
forces a directory-style frontendDist, preserves compiled CSP for both normal
and development policy, and forces local App window URLs. Tauri's own index
routing fallback may still select verified index bytes; it cannot select disk,
embedded or development-server content through this provider.

The candidate has its own identifier and app-data directory inside the owned
temporary runtime. The actual window forces incognito WKWebView storage and
rejects navigation outside its local Tauri origin. Its builder registers only
health and the three owner review commands, not arbitrary filesystem, shell,
ordinary capture/retrieval, autostart or product lifecycle commands.

## Bound requests and sessions

The native channel remains on one worker thread; only immutable assets and
typed queue messages cross threads. After actual Context installation, Desktop
sends the catalog binding. The daemon independently derives it from its retained
snapshot. The content-bound connection then mints a private non-clone received
request only after native per-message validation and closed decoding. Its
private allocation seal cannot be supplied by JSON, a PID or a boolean.

The adapter invokes the existing approval domain and journal. Each pending
session retains its original root, registry, store and admission. Prepared and
final replies retain admission through the actual native send; owner think-time
holds no operation lease. Peer process/endpoint loss, malformed requests,
mismatched or replayed followups, and delivery loss invalidate ephemeral grants.
No failure invents durable cancellation or rolls back an uncertain append.

Desktop retains wire capabilities privately and returns opaque local handles.
Queued calls have an absolute deadline and interest lease; cancelled or expired
work cannot start later. Failed result delivery and all post-prepare failures
burn pending local state and the connection. Shutdown uses an out-of-band flag,
so a full work queue cannot prevent cleanup.

## Explicit synthetic profile

FakeSecureStore is selected only by the authenticated feature-specific content
mode, on a fresh owned TEMP directory containing only its fixed marker before
lifecycle initialization. The ordinary IPC server is omitted for that mode,
and the native driver consistently uses the same process-local synthetic store.
The daemon remains the only canonical writer. Enabling the feature alone does
not switch any ordinary runtime's store or unlock any product route.

The native fixture creates one public synthetic event, then exercises real
domain prepare/cancel/confirm. It verifies unchanged canonical prefixes,
one append, mismatched digest/replay refusal, lost connection and old local
handle isolation. Additional faults cover accepted prepare without its snapshot,
failed handle allocation, expired queued work and lost result receivers.
The synthetic fault dispatcher is not registered as a renderer command.

Headless Context/provider checks do not establish WebView loading or human
review. Fresh-connection handle tests do not establish same-runtime restart
recovery because the fixture store is intentionally ephemeral and the content
mode refuses existing profiles. Signed interactive acceptance, durable recovery
with the intended platform store and product activation remain separate gates.
