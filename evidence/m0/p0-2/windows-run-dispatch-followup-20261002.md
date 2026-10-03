# Windows benign HKCU Run dispatch follow-up

Date: 2026-10-02 EDT. Session root:
`%LOCALAPPDATA%\Cryptex-Dev\p0-2\wd-1003a`.

## Preparation

The isolated marker probe was prepared at 7:56 PM EDT. Its manifest records:

- Run value name: `CryptexDevP02RunDispatch`
- command length: 186 UTF-16 characters
- marker-script SHA-256: `D167EFEF046E1EFD82EA4713B22D25ABBCFEA34C157435891D355DB9F1614B11`
- prior `StartupApproved\Run` value: absent
- marker self-check: passed

Before the user's real sign-out/sign-in, the status command reported
`run_registered=true`, `marker_present=false`.

## After sign-out/sign-in

The user ran the status command from the absolute repository path. It initially
reported:

```text
run_registered=false
marker_present=false
marker=null
```

The manifest and marker script remain in the session root. The old
`run_registered` field measures equality with the manifest command; false does
not distinguish a missing value from a changed value. No absence or removal
was established by that output. Read-only inspection of `Microsoft-Windows-Shell-Core/Operational`
found a Run-key enumeration beginning at 8:32:11 PM EDT and finishing at
8:32:28 PM EDT. No event in that window names `write-marker.ps1`,
`CryptexDevP02RunDispatch`, or `wd-1003a`; no matching 9707/9708 command
execution event was found. The Security log could not be read without elevated
rights. The Shell-Core enumeration events do not disclose which value names
were present during enumeration.

## Classification

**INCONCLUSIVE — marker dispatch not observed.**

The marker was not written, and its command has no Shell-Core execution event.
However, a later read-only inspection found the Run value present, with
the exact manifest command and a matching 186-character length. Re-running the
same `Status` action then reported `run_registered=true`. A new `Prepare` was
correctly refused because the test value is occupied. The current
`StartupApproved\Run` value remains absent. This conflicts with the user's
earlier post-login status output; the reason for the differing equality result
is unknown. It does not establish a removal or re-addition. A fresh `wd-1003b` experiment was not armed because
the preparation script correctly refused to replace this occupied test value.
The existing registration and evidence were left intact.

The status script now reports value presence and manifest-match separately.
Its read-only follow-up returned `run_value_present=true`,
`run_value_matches_manifest=true`, `run_command_length=186`, and
`marker_present=false`. PowerShell syntax parsing passed.

The subsequent Status enhancement adds observation UTC, PID, session ID, user
SID and process bitness. The physical read at `2026-10-03T01:14:28.5872041Z`
returned PID `21528`, session `3`, user SID
`S-1-5-21-958957750-515913097-3701153746-1001`, 64-bit process, matching
registration and no marker. This is a read-only observation, not another login
trial. The existing registration remains armed.

## Combined collector follow-up

`collect-windows-run-dispatch.ps1` ran under Windows PowerShell 5.1 at
`2026-10-03T01:30:13.4484145Z`. Retained observation:
`dispatch-observations/20261003T0130134484145Z-f14e5d14d2b44a3795ff4ba9cd3ac3ea.json`
under the existing session root.

- Registry32 and Registry64: present, matching command, length 186.
- This value's StartupApproved bytes: absent in both views.
- Inspected HKCU/HKLM Run-disabling policy values: absent.
- Marker script SHA-256: matches the prepared manifest.
- Explorer PID 5208: same session 3, created at `2026-10-03T00:32:04.5700840Z`.
- Eight Shell-Core Run/RunOnce enumeration events were retained; no experiment
  launch event was found. Explicit UTC XPath avoids inconsistent date-boundary
  interpretation across Windows PowerShell and PowerShell 7.
- Marker absent; collection errors: none.

The collector's five synthetic controls passed under Windows PowerShell 5.1:
absent marker, present marker requiring login correlation, changed script hash,
mismatched-manifest refusal, and out-of-root refusal. Unique observations
preserve earlier records; the existing startup registration was unchanged.
Synthetic fixture root:
`%LOCALAPPDATA%\Cryptex-Dev\p0-2\dispatch-collector-test-0e0a96a93ca34dd6a089f7379087de22`.
PowerShell syntax parsing and `git diff --check` passed.

This is diagnostic preparation. No new physical login was performed and no
startup root cause is established. Next: real sign-out/sign-in, then the combined
collector against `wd-1003a`, preserving the currently armed registration.

## Second real login: observer-dependent registry visibility

The user's collected observation at `2026-10-03T01:33:28.6382498Z` is retained
as `dispatch-observations/20261003T0133286382498Z-35977ce6db784a9b95df2da6986600c0.json`.
Session changed from 3 to 4 with the same user SID. Explorer PID 28252 was
created at `2026-10-03T01:33:05.6242650Z`; `quser` confirms the new 9:33 PM EDT
console login. The user's Windows PowerShell collector recorded:

- Run value absent in both registry views; no marker.
- Matching script hash, no collection errors, no inspected Run-disabling policies.
- New Shell-Core Run enumeration, but no experiment launch event.

A subsequent read-only Codex-hosted query at `2026-10-03T01:34:02.0820868Z`
found the Run value present; explicit HKCU and HKEY_USERS/current-SID reads
both found it. No repository script changed registration between these reads.
Another Codex-hosted collector at `2026-10-03T01:36:54.2103218Z` recorded both
registry views matching, no marker, and no collection errors.

**This establishes observer-dependent registry visibility; it does not prove
registry deletion/recreation, dispatch skipping, or a daemon termination cause.**
The user's external absence and the Codex-context presence must not be treated
as equivalent Explorer-visible startup registration evidence.

The Codex process ancestry leads to the packaged desktop app
`OpenAI.Codex_26.930.3748.0_x64__2p2nqsd0c76g0`. The installed manifest includes
both legacy virtualization-disabled properties and newer exclusions (registry:
Chrome NativeMessagingHosts). Microsoft documents that newer exclusions take
precedence on Windows 11 and that private HKCU writes may be visible only in an
app context: [flexible virtualization](https://learn.microsoft.com/en-us/windows/msix/desktop/flexible-virtualization).
This is a plausible explanation, not confirmed attribution: the child
PowerShell's `GetCurrentPackageFullName` returned no package identity.

The collector now records process package identity and executable ancestry,
and exposes `run_matches_both_views` in its short output. Its five regression
controls still pass. No application manifest, Windows policy or startup value
was modified during this investigation.

Next: from the user's ordinary Windows PowerShell, prepare a fresh `wd-1003b`
marker experiment and collect its pre-login snapshot. Require matching
registration in that external console before another sign-out/sign-in. Retain
`wd-1003a`; do not clean it up or re-arm it from the Codex context.

## External preparation of wd-1003b

The user's ordinary Windows PowerShell prepared `wd-1003b` at
`2026-10-03T01:38:11.1241028Z`, with a successful marker self-check and
186-character Run command. The external snapshot at `01:38:13.4700102Z`
confirmed matching registration in both views, no marker and no collection
errors. This is valid external pre-login registration evidence.

The user's follow-up at `01:39:33.0279611Z` also found matching registration,
no marker and no collection errors. Its observer remains in session 4, with
the same Explorer PID 28252 created at `01:33:05.6242650Z`, before preparation.
`quser` still reports the 9:33 PM EDT login. Thus this snapshot does not establish
a new login after preparation and must not be classified as another failed
login launch. The experiment is armed pending a real sign-out/sign-in.

The user only needs to perform the interactive sign-out/sign-in and return to
the chat. Codex can read the retained runtime evidence and perform subsequent
diagnostic collection; external commands are needed only if comparing observer
contexts again becomes necessary.

## wd-1003b real login dispatch: VERIFIED

After the user's next real sign-out/sign-in, session changed to 5. Explorer
PID 2288 was created at `2026-10-03T01:41:41.729304Z` (9:41 PM EDT).
The benign marker records PID 15656, session 5 and launch time
`2026-10-03T01:42:14.7693778Z`, approximately 33 seconds after shell creation.
Shell-Core event 9708, record 3298, at `01:42:15.0945097Z` records PID 15656
and the truncated command `write-marker.ps1"`.

The collector now correlates a truncated script name with the marker PID and
launch time within ten seconds, instead of treating a name alone as attribution.
The physical collection at `01:44:44.9403733Z` retained that correlated event
with no collection errors. The five synthetic collector controls passed again.
The Codex registry view still shows the old `wd-1003a` command, so its mismatch
with the `wd-1003b` manifest is not evidence of external value removal.

**Verified scope:** the ordinary-PowerShell-prepared benign HKCU Run command
was dispatched during a real login. Event 9708 is launch-operation completion,
not process-exit evidence. No daemon survival or supervision decision follows.

## Next instrumented daemon experiment

Local build of `cryptexd` and `m0-lifecycle-compare` passed. Five observer
controls passed under
`%LOCALAPPDATA%\Cryptex-Dev\p0-2\observer-controls-20261003T014403138Z`.
No GitHub workflow was dispatched.

An attempt was dispatched with `Shell.Application` from the Codex process,
without elevation. Although Explorer appears in the ancestry, this did not
establish an independent ordinary-shell registry context. Attempts
`wl-1003c/d/e/f` did not produce a manifest and remain retained.
`wl-1003f/preparation.jsonl` records failure after daemon registration because
`Get-FileHash` was unavailable. Preparation's existing rollback removed its
matching registrations; subsequent fresh preparation passed its occupied-value
check. No OS policy or application manifest was changed.

Using standard framework SHA-256 avoids that module-autoload dependency.
`wl-1003g` was prepared at `2026-10-03T01:49:21.3976179Z`. Its retained manifest
records source `b24253b` with `source_dirty=true`, reflecting the diagnostic
changes in progress. Binary SHA-256:
`EDA29D5001CAFCA9091893696E8C46FC4436702152810A9CF764EDD9D0E18DB2`.
Observer SHA-256:
`49B77FCB4570D8013044D74A5C4B39F06F0AFDBA92A842C48A5EB591780AA10E`.
Both independently matched `Get-FileHash` in the working tool environment.
`preparation.jsonl` reached `manifest_written`. However,
`external-preflight.json` was written by a shell launch descended from the
packaged Codex context and does NOT prove Explorer's unvirtualized Run values.
The latest user login entered session 6; no daemon or observer runtime files
appeared under `wl-1003g`. Classify it as **NOT ARMED / NOT TESTED**. No daemon
survival claim follows. Syntax parsing, five collector controls and five
observer controls passed; no CI was dispatched.

Next: prepare a fresh instrumented root in the user's ordinary Windows
PowerShell and require the printed manifest before sign-out/sign-in. Codex can
inspect retained evidence afterward. Preserve every earlier runtime root.

## External occupied registration identified

The user's ordinary PowerShell revealed the actual occupied daemon Run value:
`D:\github_repos\cryptex\target\debug\cryptexd.exe lifecycle hold`, with
runtime/ready/stop paths under `compare\mode-a-hkcu`. Its observer value was
absent. Thus cleanup against `wl-1003g` correctly refused ownership mismatch.
Codex's separate view still matched `wl-1003g`; that view did not identify the
external owner. No cleanup or new preparation succeeded in those user attempts.

The retained legacy root contains a stop file last written at 18:06 EDT.
At the latest 21:52 login its ready/lock/status metadata was updated; status
records PID 18100 released with `clean shutdown released instance lock`.
No live `cryptexd` was found. The hold loop's stop-file check matches this
clean-shutdown result. This explains the current legacy comparison run; it
does not establish the cause of the earlier 13:00 UTC disappearance.

`replace-legacy-windows-login.ps1` was added for a single external invocation.
It validates the exact confirmed command and empty observer slot, refuses a
live legacy ready PID, backs up the registry command, and prepares a fresh
`wl-1003h` root after removing that matching value. Failure restores the saved
command only if neither slot has a new owner. Legacy runtime/control files stay
retained. Syntax parsing and Git diff checks passed; a local read-only guard
check correctly refused Codex's nonmatching `wl-1003g` view without mutations.
Actual external replacement is pending. No GitHub workflow ran.
