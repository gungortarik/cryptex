# Managed Syncthing notice-byte portability — failure and correction

## Mac original failure

Source `502abf409925b368ff1377380c719069207f0d61`; macOS 27.0.1 arm64,
Node v24.19.0. The project owner supplied the Mac executor's report:

`node --test .../syncthing-pilot.test.mjs .../syncthing-bundle.test.mjs`
exited **1**, **7 PASS / 8 FAIL**, no skip. Bundle errors were
`SYNCTHING_BUNDLE_SOURCE_CHANGED`. Pinned notice hashes represented Windows
CRLF working-tree bytes; Mac's repository notice files were LF. A read-only
CRLF-equivalent calculation matched the originally pinned digests. No notice
conversion or relaxed hash comparison was applied by Mac.

Native input was inventoried, not executed:
`/Applications/Syncthing.app/Contents/Resources/syncthing/syncthing`, stable
**2.1.5 darwin-arm64**, supplied binary SHA-256
`6de41132c8a3fc78fc57486df32f9ac69d6f53db1e1910bb46f548ee7f29725e`.
No live Syncthing proof, global config/startup change, commit/push or Actions.
This remains **NOT YET VERIFIED**, not a native Syncthing failure claim.

## Repository correction `f17bcf7`

Windows reproduction confirmed `git ls-files --eol`: index **LF**, working
tree **CRLF** under generic `text=auto`. Both notices' Git blobs exactly matched
their Windows content after CRLF-to-LF conversion. The repository now explicitly
sets `third_party/syncthing/2.1.5/*.txt text eol=lf` and pins the original **LF
Git blob** digests. Git notice blobs/wording did not change. Source copy and
post-copy verification remain exact-byte checks; no runtime normalization,
alternate accepted digest or swallowed mismatch.

Regression tests verify LF checkout/copy equality, rejection of CRLF payload
even with a rewritten self-declared manifest digest, and actual isolated Git
checkout under `core.autocrlf=true`. Existing content-tamper refusals remain.
The upstream CRLF and repository LF fingerprints are both documented in
`third_party/syncthing/2.1.5/README.md`.

## Additional Windows observation failures, retained

First fresh LF live root `managed-lf-proof-20261005-c02d1a42f65140e0aecfed5b5f895d18`
failed `SYNCTHING_STATUS_UNPROVEN`: the immediate post-transfer status did not
meet idle/zero-needed conditions. Exact status fields were not retained.
Another fresh root `managed-lf-proof-20261005-eb53012e1edc47b380ac65c22e97229c`
failed `ENOENT` before peer-stop/restart. Both owned profiles were cleaned;
these runs are **not PASS**. No canonical data was involved.

The runner now waits within its existing **45-second** observation bound for
the same idle/zero-needed status and additionally rejects native pull errors
and malformed counters. Synthetic-file observation treats **only ENOENT** as
pending during replacement, still requiring the exact expected contents.
Other I/O errors, unsafe entries and oversized fixtures fail. No integrity
metadata reads or digest checks receive this pending-absence behavior.
Regression tests cover these intermediate observations and explicit refusals.

## Windows corrected actual run

Fresh root:
`%LOCALAPPDATA%\Cryptex-Dev\p0-4\managed-lf-proof-20261005-f5eacd4230254665bdfe2ffc70b901a8`.
Report `managed-syncthing-report.json`, SHA-256
`4fa547a5ad7e3d8808e64c157d4ba8cff601eedb7fe671a0b252a67e89a5a2d4`.
All pairing/auth/history/conflict/restart/staged native and LF notice assertions
PASS. Global config observed hash unchanged. Four clean exits **0**, PIDs absent:
**23568 / 62 ms**, **34736 / 10062 ms**, **20680 / 823 ms**, **6780 / 138 ms**.
No forced cleanup or native database-stop failure.

Combined Windows Node **150/150** plus storage **5/5**, no failures/skips;
managed subset **21/21**. Five changed-script syntax and diff checks PASS.
Mac registers its extra POSIX mode test: updated managed expectation **22/22**.
Re-run only managed Syncthing on the Mac; passed native recovery and changed
package plans need no redundant repetition. Mac parity was pending at that point.
The subsequent `501d11d` Mac rerun passed **22/22** and the live proof; see
`macos-managed-syncthing-evidence-20261005.md`. Earlier failures remain retained.
No Actions.
