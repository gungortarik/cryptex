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

**Final classification: INCONCLUSIVE — marker dispatch not observed; registry
observation inconsistent.** Shell-Core confirms that the Run key was enumerated
but does not expose its value list. No test-command launch event or process
creation proof is available. Do not infer whether Explorer skipped the entry,
whether its state changed during login, or why the earlier status differed.
Preserve this session directory and use the currently matching registration
for a further real sign-out/sign-in observation before cleanup.
