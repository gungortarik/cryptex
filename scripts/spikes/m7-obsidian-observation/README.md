# M7 P1 observation/status probe

Disposable synthetic-only diagnostic source. This is not the M7 product plugin.
It does not read or mutate managed-note content and does not implement proposals.
The owner-selected physical-containment gate remains open.

The probe refuses to activate unless the desktop adapter's vault path contains
an exact `Cryptex-Dev` component. This scope opt-in is not physical-containment
proof. Install only in an explicitly identified synthetic vault; never a
personal vault. Plugin metadata JSON is written under
`.obsidian/plugins/cryptex-m7-spike/`; this does not make the entire vault
read-only. Only the default `.obsidian` configuration directory is supported by
this disposable probe.

## Local checks

From the repository root, using the installed Node runtime:

```sh
node --check scripts/spikes/m7-obsidian-observation/main.js
node --test scripts/spikes/m7-obsidian-observation/guard.test.cjs scripts/spikes/m7-obsidian-observation/real-socket.test.cjs
```

The real-socket suite uses unique local endpoints and closes them on teardown.
It does not connect to a daemon or modify any vault. It runs on Windows/macOS;
the Mac run must be reported separately. The macOS endpoint selection unit test
on Windows is not native macOS interoperability evidence.

## Synthetic-vault installation

Preserve the exact existing temporary plugin files outside the vault first.
Copy only `main.js` and `manifest.json` into the identified synthetic vault's
`.obsidian/plugins/cryptex-m7-spike/` directory, then reload that vault's existing
plugin through Obsidian. Do not overwrite unrelated plugins or user notes.
Observe `M7 spike: observation only; note IO disabled` and exactly three
commands: event counts, local health IPC, and pending-connection cleanup.

On load, `runtime-inventory.json` records the documented Obsidian `apiVersion`
export, Node/Electron versions, platform/architecture, and the temporary manifest
loader minimum. No TypeScript API package is used: this is plain CommonJS.
Neither that loader minimum nor one observed app/API version selects a Cryptex
product compatibility baseline. Record the visible Obsidian app version and
installer version separately when collecting platform evidence.

Health is opt-in and sends only `health.ping`/`status.read` over the existing
daemon endpoint: Windows named pipe `cryptex-cryptexd-v1`, or macOS
`/tmp/cpx-cryptex-<uid>/s`. The probe never starts a daemon or creates an endpoint.
Use it only with an already isolated synthetic daemon; a successful health reply
does not establish peer identity or proposal authorization. The pending cleanup
command has a 60-second transport timeout and is a fixture probe, not a canonical
operation or cancellation guarantee.

## Limits and continuation

No managed-note mutation, symlink escape re-exercise, new IPC capability,
production dependency, Actions, commit, push, or merge is needed for these
checks. Do not enable managed-note IO or claim M7 completion from the results.

Native Mac follow-up: preserve its dirty checkout, use this exact transferred
source, run the local checks, inventory the actual runtime, reload twice to
observe startup event metadata, and exercise unload in the synthetic vault.
Report each observation with its scope. Real-daemon pending cleanup and peer
authorization remain distinct from local-socket fixtures.

Sources: [Obsidian public API declarations](https://github.com/obsidianmd/obsidian-api/blob/master/obsidian.d.ts)
(`apiVersion`, `FileSystemAdapter.getBasePath`) and the existing daemon endpoint
functions in `services/cryptexd/src/ipc.rs`.
