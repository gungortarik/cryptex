# M7 P1 macOS observation-plugin lifecycle check

**Date:** 2026-10-18  
**Scope:** Synthetic Obsidian vault; event metadata only. No personal vault,
product source, Actions, or M7 completion claim.

## Result

The installed observation plugin hash matched the verified source:

`0F1C024F48120D955C2EDD6A0415F0B5E6D3DD4BEEB49D17217A82384F081F24`

The synthetic vault's plugin list initially contained only
`cryptex-m7-spike`. Baseline event counts were
`{"modify":0,"create":0,"rename":0,"delete":0}`.

| Step | Modify count | Result |
|---|---:|---|
| Baseline | 0 | Plugin hash matched |
| Edit disposable file while enabled | 1 | Event observed |
| Soft disable (`community-plugins.json=[]` + Cmd+R) | 2 | Listener remained active; reload did not unload it |
| Hard Quit, reopen with plugin disabled, edit disposable file | 2 | Count stayed frozen; hard process shutdown unloaded the listener |
| Restore plugin list | — | Only `cryptex-m7-spike` restored |

During the enabled edit, Cmd+A selected the title, so the test file was renamed
to `m7-disposable-edit-1.md` and `rename` increased once. Only the disposable
file was deleted through Obsidian UI. The remaining `Cryptex/notes/` file was
`managed-conflict-probe.md`; its SHA-256 stayed
`0fef4ba6…e1d9`.

Proof was retained outside Git at:

`~/Library/Application Support/Cryptex-Dev/m7-p1-obsidian-038a331d135f423da4fe8a7473fea2f8/observation-lifecycle-modify-20261018T170015Z/`

## Limits

This verifies observation of one synthetic edit and that the listener stops
after a hard Quit/reopen with the plugin disabled. It shows that the tested
soft reload did not unload the plugin. It does not prove managed-note physical
containment, peer/proposal authorization, real-daemon in-flight cancellation,
or safe write-back. Managed-note write-back remains disabled. The accepted M7
P1 physical-containment prerequisite is still unresolved.
