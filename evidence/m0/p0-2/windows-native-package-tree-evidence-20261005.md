# Windows complete native package tree staging — scoped PASS

- Implementation: `e5423f201543ff6a11cea0274b4a69230b545f23`.
- Windows x64, Node v24.19.0.
- The owner explicitly approved the isolated native handover pilot. ADR-0012
  remains PROPOSED; no production updater selection follows.

## Verification

Combined Node suites: **79/79 PASS** (51 transaction + 4 health adapter +
6 native recovery + 18 native tree); failures/skips 0, exit 0. All three new
module/test/runner syntax checks and staged diff check passed. No Rust source
or dependency changed; no new Rust format/lint/build claim is made.

The 18 new tests cover complete resources/empty directories, missing/extra/
modified entries, schema/target errors, safe portable paths, case collisions,
roles/protocol, directory junction refusal, existing and overlapping destinations,
manifest metadata, host permission representation, injected copy failure,
source mutation during staging and runner evidence/root protection. POSIX mode
preservation executes only on Mac; Windows verifies null mode representation.
Fault tests use synthetic inputs, not actual disk failure. Test fixtures were
removed by their cleanup hooks and the runner tests execute no package binary.

## Retained real Windows tree copy

Read-only source:
`C:\Users\Tarik\AppData\Local\Cryptex-Dev\p0-2\native-package-proof-1003\installed`.

Fresh retained proof:
`C:\Users\Tarik\AppData\Local\Cryptex-Dev\p0-2\native-tree-20261005-fea20b4849ce459897b4b672063a00f9`.

`native-tree-report.json` SHA-256:
`ac8da4bc8673289c7e5d246f2eb1447b7e20826795dfccf8de3a7368a81d55d1`.

All three current installed files were inventoried and copied, then source and
destination reverified. Desktop digest
`2b91af7c6d497ca91399f9dd831ffd0759bbff4f7491476b738260a90f8002a7`
and daemon digest
`a958e52813d5f59561e20e1f80a1d261f3028b9fefc5fa3df093fa1589493658`
match retained baseline evidence. Uninstaller digest:
`fbf7289095ce78700d88c8b327a667672cf0e99ab34c63981bd67e3265997ead`.
No source file was rewritten, no binaries launched, no startup registration
changed, no GUI manipulated and no Actions run.

## Remaining limits

This is complete-tree staging, not an actual Desktop/daemon handover or runtime
protocol proof. Inventory protocol 1 is declared experiment metadata. Digests
do not authenticate publisher identity. No slot activation, transition journal,
whole-handover native lock, successor launch or canonical path is connected.
Partial staging is retained and never reused automatically. File/directory
power-loss durability and same-user adversarial tampering are not established.
Windows read-only attributes are not preserved as a permission proof. POSIX
directory root mode is not part of the package manifest; entry modes are.
Mac complete-tree parity remains pending, with its exact plan committed beside
this evidence. Signing, actual native resources/launch and canonical acceptance
remain separate gates.
