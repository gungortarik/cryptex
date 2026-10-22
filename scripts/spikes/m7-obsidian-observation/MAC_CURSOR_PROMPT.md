# Mac Cursor — M7 P1 observation-only follow-up

Continue the authorized M7 prerequisite spike on the Mac using this transferred
observation-only source, including the macOS half-close fix. The owner explicitly retained the current Master Plan
and selected: **keep managed-note writes disabled**. Do not replace Obsidian
editing with a Desktop-only workflow. No Actions, product implementation,
commit, push, merge, or M7-complete claim.

The repository reference is `codex/m5-capture-timeline`, product base
`74bc3b14127962579ae03838bafea9336612a033`. These diagnostic files are a local
transfer and are not claimed to be available from origin. Read the applicable
`AGENTS.md` and `docs/34_CURRENT_IMPLEMENTATION_HANDOFF.md`, but preserve the
current checkout, including all dirty/untracked files. A dirty checkout does
not prevent testing this external package. Do not fetch/pull, switch, reset,
clean, stash, or overwrite checkout files for this task.

1. Locate the exact extracted `Cryptex-M7-Observation-P1-20261016` package in
   Downloads or the owner-provided transfer location. Do not search personal
   vaults. From that package directory, verify `SHA256SUMS.txt` with
   `shasum -a 256 -c SHA256SUMS.txt`. If it is absent or a hash differs, report
   that exact blocker before installing anything.
2. Run `node --check main.js`, then
   `node --test guard.test.cjs real-socket.test.cjs`. The prior Mac package
   passed 16/18: two real-socket fixtures exposed half-open socket retention
   after a complete response. This package fixes response cleanup by destroying
   the completed transport. Windows rerun passed 18/18. Record your actual Mac
   result; local fixtures do not prove native Mac daemon interoperability.
3. Use only the previously owner-identified synthetic root
   `~/Library/Application Support/Cryptex-Dev/m7-p1-obsidian-038a331d135f423da4fe8a7473fea2f8`.
   Inspect that exact root to identify its synthetic vault and plugin. If the
   root is absent, report that fact; do not select a personal vault. Verify
   that the intended Obsidian window is this synthetic vault. Record hashes of
   its existing managed fixture and `owner-note.md` sentinel, without logging
   their content. Preserve the exact existing plugin files in a unique proof
   directory outside the vault before copying `main.js` and `manifest.json`
   into the synthetic vault's `.obsidian/plugins/cryptex-m7-spike/` directory.
4. Reload the existing synthetic Obsidian plugin using available app controls.
   The owner has already authorized control of this synthetic vault and this
   test plugin. Observe `M7 spike: observation only; note IO disabled`, and
   exactly three commands: event counts, local health IPC, and pending-connection
   cleanup. Do not execute any former mutation or junction/symlink probe.
5. Collect the resulting plugin `runtime-inventory.json`. Record the visible
   app version and installer/plist version as separate fields. Record the API
   version from the runtime inventory. Its API-types package field is correctly
   `none: plain CommonJS runtime probe`; the temporary `minAppVersion` is only
   a loader field and does not select a product support minimum. Do not turn a
   differing installer/app version into an invented conclusion.
6. Reload this synthetic vault twice and collect the two new
   `startup-events-*.json` metadata results. Exercise plugin disable/re-enable
   through supported controls and describe the observed lifecycle transitions.
   Do not edit a managed note to generate an event. Without an actual event
   during unload, do not claim native event-listener silence was proved.
7. Health IPC is optional for this turn: invoke it only if an already isolated
   synthetic daemon is identified. Do not add a command, capability, listener,
   or HTTP endpoint. The existing Mac socket is `/tmp/cpx-cryptex-<uid>/s`.
   A health success proves status interoperability only. Local socket fixtures
   remain distinct from real-daemon pending cleanup or authenticated Obsidian
   proposal authorization; mark unexercised cases as NOT RUN.
8. Recheck fixture/sentinel hashes and leave managed-note IO disabled. Preserve
   the proof artifacts. Report source hashes, app/API/installer metadata,
   test counts, startup/lifecycle observations, optional health result, and
   exact remaining gaps. Physical containment is still unresolved and M7 is
   not complete. Stop after this bounded follow-up.

The probe writes only bounded plugin metadata through Obsidian's adapter. It
does not read or mutate managed-note contents. The `Cryptex-Dev` path-component
check is synthetic-scope opt-in, not symlink/reparse containment proof.
