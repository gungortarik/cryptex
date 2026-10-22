# M8 Windows fixture native path correction

Date: 2026-10-21. Owner's repaired guest runner reached the native fixture
preparer, where `Process.Start` reported file not found. The preceding fmt,
package-tests, Clippy and build calls check their exit status before this call;
the transcript is consistent with successful completion of those steps, but
their actual logs/counts have not been imported or independently reviewed.
Neither a native PASS nor a full Windows batch PASS is claimed.

Cause: PowerShell resolves `..\native\syncthing.exe` relative to its provider
location, so Get-FileHash found the file. The preparer's later
`[IO.Path]::GetFullPath($Binary)` used the different .NET process current
directory, producing the wrong executable path for ProcessStartInfo. This was
another verification harness defect, not evidence that the pinned executable
was absent or corrupted. Fixture preparation stops before native generation.

The runner now resolves and requires an existing filesystem FileInfo before
changing location. The preparer independently resolves its Binary argument to
the same FileInfo.FullName before hashing and launching, so a direct relative
preparer call is safe too. Refreshed only those two entries in the 16-file
source manifest; historical ZIP/proof files remain sealed and unchanged.
No Rust/native engine, pairing/trust or product writer changed.

Added `scripts/m8/test-native-fixture-path.ps1`. Real Windows PowerShell 5.1
changes its provider location to the retained pinned native package while
deliberately assigning a different .NET current directory. It passes a relative
binary path to the actual preparer, successfully generates a fresh protected
synthetic native config/identity and checks the recorded absolute executable
path and certificate. It starts no transport server or discovery. PASS, proof:
`C:/Users/Tarik/AppData/Local/Cryptex-Dev/m8-verification/path-regression-811676f48b374163a93baec270d46854`.

The stderr/exit-status runner regression was rerun on Windows PowerShell 5.1:
3/3 PASS; proof:
`C:/Users/Tarik/AppData/Local/Cryptex-Dev/m8-verification/ps51-runner-regression-9147bf47510149608166edee0bcee4de`.
Diff check PASS. No source changes requiring a Rust rerun were made here.

For the already downloaded, checksum-verified and stderr-repaired guest bundle,
the equivalent owner workaround needs no file/manifest edits: resolve the
native executable with PowerShell Resolve-Path from its source folder and pass
that absolute path to the existing runner. Its original 16 checks remain in
effect. The final guest batch/native result remains OPEN until that rerun.
No Actions, commit, push, merge or M8-complete claim.
