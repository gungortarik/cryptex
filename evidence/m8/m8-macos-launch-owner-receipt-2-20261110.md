# Second personal-Mac launch receipt: 5/11; direct FD mapping candidate stopped

Status: **incomplete; not production acceptance**. The uploaded archive was
inspected as data, not executed. This supersedes the pending owner run recorded
in [the first receipt](m8-macos-launch-owner-receipt-20261110.md).

## Exact receipt

- Input: `MAC-BAGLANTI-SONUC (2).zip`.
- Archive SHA256: `39578d0abfbe59537535bb897fb248aaffa5fde42da0dafaa31c0f82696bd7f1`.
- Inventory: one regular `result.json`, 3067 bytes.
- Collected: `2026-11-10T22:32:12Z`; personal Mac 27.2 / SDK 27.2, arm64.
- Source: `888d68acaedd30e9622fe08e97ef7deeff1094c2`.
- Native source SHA256: `84c91c53b239e22c49769587ace3cbce7a5774fa185f2fee0ed53fff2c76917d`.
- Exact Git source/native hash, duplicate-key rejection, closed eleven-case
  inventory, literal boolean capability fields and incomplete status were checked
  with the source-bound parser. No imported instructions or code were run.

All five public SDK checks passed. Native compilation and isolated existing-key
signing passed. Native atomic API headers/runtime were true; live-self-derived
same-organization sealed-bundle eligibility was true. The previous metadata and
aggregate build-gating defects no longer prevent execution on this owner host.

| Case | Observed pass |
| --- | --- |
| supervisor timeout distinct from child deadline | true |
| wrong identifier refused | true |
| ad-hoc counterpart refused | true |
| raw fork excludes both pipe ends | true |
| unrelated spawn excludes both pipe ends | true |
| intended child validates live parent | false |
| short handoff refused | false |
| extra handoff refused | false |
| forged parent token refused | false |
| missing EOF reaches child deadline | false |
| fresh child repeats parent validation | false |

The row `result` strings are expected outcome labels, not observed exit statuses.
In particular, `result=accepted, pass=false` does **not** mean acceptance occurred.
The existing receipt loses the specific preparation/spawn/child error and cannot
show which stage failed. Do not infer that all six children actually ran.

`candidate_complete=false`, `runner_exit=2`, `diagnostic=native_candidate_case_failed`.
Production integration remains not implemented; production and M8 completion
flags remain false. No certificate or Mac setting change is indicated.

## Source-level explanation and its limits

Apple's published XNU implementation copies the descriptor table before applying
spawn file actions. The spawn clone uses `CLONEPROC_SPAWN`, not `CLONEPROC_EXEC`;
`fdt_fork` excludes `FP_CLOFORK` when `in_exec` is false. Only afterward does
`exec_handle_file_actions` attempt `dup2(source_fd, 198)` against the child's table.
The protected source descriptor has therefore already been omitted in that
implementation. This strongly explains why all six mapped-handoff cases fail
while unrelated fork/spawn exclusion succeeds.

This is a **source-supported inference**, not an observed errno from the owner's
Mac 27.2 kernel. The archive does not contain a spawn return code. The reviewed
published source is not asserted to be that kernel's exact build. Nevertheless,
neither the native result nor the public API contract supports activating this
candidate; repeating it unchanged cannot establish the missing proof.

Primary sources, inspected at XNU commit `f6217f891ac0bb64f3d375211650a4c1ff8ca1ea`:

- [XNU spawn ordering and file actions](https://github.com/apple-oss-distributions/xnu/blob/f6217f891ac0bb64f3d375211650a4c1ff8ca1ea/bsd/kern/kern_exec.c)
- [XNU spawn clone and descriptor table copy](https://github.com/apple-oss-distributions/xnu/blob/f6217f891ac0bb64f3d375211650a4c1ff8ca1ea/bsd/kern/kern_fork.c)
- [XNU close-on-fork exclusion](https://github.com/apple-oss-distributions/xnu/blob/f6217f891ac0bb64f3d375211650a4c1ff8ca1ea/bsd/kern/kern_descrip.c)

The tempting fileport workaround is not an approved public mapping alternative:
`fileport_makeport` is public, but the published
`posix_spawn_file_actions_add_fileportdup2_np` declaration is in
[spawn_private.h](https://github.com/apple-oss-distributions/xnu/blob/f6217f891ac0bb64f3d375211650a4c1ff8ca1ea/libsyscall/wrappers/spawn/spawn_private.h),
not [public spawn.h](https://github.com/apple-oss-distributions/xnu/blob/f6217f891ac0bb64f3d375211650a4c1ff8ca1ea/libsyscall/wrappers/spawn/spawn.h).
No newer public-SDK evidence has been collected for that action. Do not copy its
prototype, include a private header or resolve it dynamically.

## Disposition and exact continuation

Stop the direct CLOFORK descriptor + ordinary spawn-duplication candidate.
Preserve all failed observations. Do not clear the parent's protection, revert to
pipe()+fcntl, remove malformed/deadline cases or count refusal as success.
The source-only independent reviewer concurred with the ordering conflict and
identified the private-header mapping blocker; that is not native execution.

No repeat Mac/VM run is requested. Existing source/packages remain historical
evidence, not the next owner action. Actual Unix IPC, product minimum OS and Mac
owner mutation gates are unchanged. The same-organization bundle checks now have
positive bounded evidence, but child policy, live parent pinning and complete
custody still do not. Neither original security finding is declared closed.

The next decision is recorded in the
[carrier reassessment](../../docs/proposals/m8-macos-launch-carrier-reassessment.md).
Prepare/review a public sender-attributed message-bootstrap design while retaining
accepted Unix IPC, or pause that design work. The repository requires asking
before changing integration direction; no replacement carrier was implemented.
