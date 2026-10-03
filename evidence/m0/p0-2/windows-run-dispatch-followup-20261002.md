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

The manifest and marker script remain in the session root. The test Run value
is absent after sign-in. Read-only inspection of `Microsoft-Windows-Shell-Core/Operational`
found a Run-key enumeration beginning at 8:32:11 PM EDT and finishing at
8:32:28 PM EDT. No event in that window names `write-marker.ps1`,
`CryptexDevP02RunDispatch`, or `wd-1003a`; no matching 9707/9708 command
execution event was found. The Security log could not be read without elevated
rights. The Shell-Core enumeration events do not disclose which value names
were present during enumeration.

## Classification

**INCONCLUSIVE — Run registration disappeared by the post-login status check.**

The marker was not written, and its command has no Shell-Core execution event.
However, a later read-only inspection found the Run value present again, with
the exact manifest command and a matching 186-character length. Re-running the
same `Status` action then reported `run_registered=true`. A new `Prepare` was
correctly refused because the test value is occupied. The current
`StartupApproved\Run` value remains absent. This conflicts with the user's
earlier post-login status output; the point at which the observed registry
state changed is unknown.

**Final classification: INCONCLUSIVE — marker dispatch not observed; registry
observation inconsistent.** Shell-Core confirms that the Run key was enumerated
but does not expose its value list. No test-command launch event or process
creation proof is available. Do not infer whether Explorer skipped the entry,
whether its state changed during login, or why the earlier status differed.
Preserve this session directory and use the currently matching registration
for a further real sign-out/sign-in observation before cleanup.
