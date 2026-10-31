# M4 macOS native visual review — scoped PASS

**Branch:** `m4/knowledge-engine`
**Source HEAD:** `ebdd8c975c103eb437c035fdda70956b9c9be4ef`
**Code equivalence:** `ebdd8c9` is documentation-only after the product-code SHA
tested by final dual-OS CI, `b1c43e2c36534e360cb06fb5cacef5bf6fd210ee`. No
product-code delta in this tip.
**Host:** macOS 27.0.1 (Build 26A434), arm64
**Runtime:** fresh isolated empty M4 pilot under
`~/Library/Application Support/Cryptex-Dev/m4/macos-visual-shutdown-20261010T023604-d97a7902-d1a4-4cdd-b0da-612ed8ca502b`;
no personal or Cortex memory used. Earlier failed/diagnostic/final pilot
runtimes were preserved and not reused.

## Build and launch

`pnpm --filter @cryptex/desktop tauri:package --bundles app` completed
successfully on this host. The packaged `Cryptex.app` was copied into the
isolated pilot input tree and launched with:

`open -n -W …/Cryptex.app --args --cryptex-pilot-runtime <runtime>`

Live Desktop PID **58134** and daemon PID **58147** were confirmed before UI
interaction. Writing `show.request` to the runtime root reported by the app
consumed the request and set `window_visible=true`
(`window_restored_via_show.request`).

## Visible retrieval-safety behavior

Searching the synthetic query `m4-visual-smoke-test` displayed the expected
fail-closed RetrievalSafetyState hold:

- Status **Attention**
- Banner **RETRIEVAL PAUSED**
- Copy: “Cryptex could not verify the complete local event history. Search is
  paused to protect your memory.”
- No search results were shown

Screenshot retained outside Git at the pilot’s `proof/02-hold.png` (binary not
copied into the repository).

## Shutdown

An earlier orphan daemon (**54655**) from a prior incomplete session on
`macos-visual-final-20261010T015815-…` was stopped by writing `stop\n` to that
runtime’s `stop` file only (no force-kill). It exited within two seconds;
lifecycle status recorded `note=clean shutdown released instance lock`.

On the fresh visual session, Accessibility could not open the Cryptex
**status-item** tray menu (`menu bar 2` item 1; `Invalid index` / no menu). The
status-item tray **Quit Cryptex** action was therefore **NOT VERIFIED** and is
not claimed as passed. The documented `pilot-quit.request` mechanism was used
as an explicitly labeled equivalent clean-shutdown check:

- Both Desktop and daemon PIDs exited
- Runtime recorded `clean_quit` in `gui-proof.snapshot` and
  `desktop-supervisor.state`
- Lifecycle status: `clean shutdown released instance lock`
- `open -W` exited **0**

The M4 contract and Definition of Done require visual review of retrieval
safety; they do not list tray-menu interaction as a separate M4 acceptance
test. This record preserves the tray automation limitation explicitly.

## Gate status

**macOS native retrieval-safety visual review: scoped PASS** at documentation
tip `ebdd8c9` (product behavior covered by CI at `b1c43e2`). This does not
complete M4. Final Definition of Done and owner review remain open. Production
v2 event writing remains disabled. No merge or M4-complete claim is made.
