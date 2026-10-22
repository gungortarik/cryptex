# M7 P1 observation-only portability follow-up — macOS

**Scope:** synthetic-vault observation only. The owner-selected managed-note
write gate remains closed. No product implementation, Actions, commit, push,
or merge is claimed.

## First run — before socket fix

- `main.js`: `65008b0c75a4c975a00baeb3a0bcafe63a5058783fcdd3cedc53c11efe7cefd5`
- `manifest.json`: `89a514688e69dfcd7824644a439d27676ee563456c43e20ebf03e47418c2fd26`
- Mac ran `node --check` successfully and `node --test` returned **16/18**.
- The two failures were local socket fixture cases: completed ordinary health
  response retained a half-open client, and the unload probe retained its
  response socket when the peer kept its side open.
- The failure was confined to local fixtures; it did not prove native daemon
  interoperability or a daemon defect.

## Local correction after the Mac report

Both response-complete paths in the disposable observation probe now call
`socket.destroy()` after consuming the newline-framed status response, instead
of half-closing only the client write side with `socket.end()`. Corrected
`main.js` SHA-256 is
`0f1c024f48120d955c2edd6a0415f0b5e6d3dd4beeb49d17217a82384f081f24`. The change is
limited to the diagnostic plugin transport cleanup; it adds no endpoint,
capability, daemon behavior, or managed-note access.

On Windows, `node --check` and both Node test files passed **18/18** after the
correction, including the two formerly failing held-open-peer fixtures.

## Corrected Mac run — owner reported PASS

- `SHA256SUMS.txt` native `shasum -c` failed because the Windows-generated
  manifest had CRLF line endings (the parsed filename retained `\r`). After
  CRLF normalization, all **6/6** listed source files matched.
- Corrected `main.js` hash matched: `0f1c024f48120d955c2edd6a0415f0b5e6d3dd4beeb49d17217a82384f081f24`.
- `node --check`: PASS; `node --test`: **18/18 PASS**.
- Synthetic vault was verified at the owner-provided `m7-p1-obsidian-.../synthetic-vault` path. The prior plugin backup and proof are preserved under the reported `plugin-proof-before-halfclose-fix-20261016T214447Z` and `observation-p1-mac-halfclose-20261016T214447Z` directories.
- App/API `1.14.4`, installer/plist `1.13.7`, Electron `43.3.0`, Node `24.18.1`; these are separate observations and do not set a product support baseline. `api_types_package` is `none: plain CommonJS runtime probe`.
- Status was `M7 spike: observation only; note IO disabled`; the three observation commands were present.
- Two reloads each reported `create: 3`. Disable passed. First re-enable attempt was lost because Quit overwrote the plugin enable list; re-enabling after Quit passed. No event occurred during unload, so listener silence is not claimed.
- Fixture and sentinel hashes were unchanged. Health IPC, real-daemon pending cleanup, and proposal authorization were NOT RUN. Physical containment remains UNRESOLVED.

The corrected socket behavior is verified by Mac local fixtures. This still does
not establish native daemon interoperability, real-daemon cleanup, proposal
authorization, physical containment, or M7 completion.

## Transfer manifest correction

The first corrected-source ZIP used CRLF in `SHA256SUMS.txt`, which made
macOS `shasum -c` parse a trailing carriage return in each filename. The owner
normalized the file and verified 6/6 payloads. A follow-up archive carries the
same, already Mac-verified source bytes with LF-only manifest lines:

- `C:\Users\Tarik\Downloads\Cryptex-M7-Observation-P1-socketfix-lf-20261016-175009.zip`
- ZIP SHA-256: `1cbbb4478822c1c4f670d16dc297ab931dad8bd3493e81fd8aa622a5c81f98bd`

Local archive integrity and all 6/6 payload hashes passed. No further Mac
plugin reload is needed for this packaging-only correction.

## Remaining gates

- Native Mac health IPC and real-daemon pending cleanup remain NOT RUN.
- Authenticated Obsidian proposal authorization remains NOT RUN.
- Physical containment is UNRESOLVED; managed-note I/O stays disabled and M7
  is not complete.
