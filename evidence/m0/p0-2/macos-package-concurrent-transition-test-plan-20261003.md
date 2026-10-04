# Mac task — package-candidate concurrent transition parity

**Scope:** focused P0-2 regression check; no production updater integration.  
**Workflow:** do not run GitHub Actions.  
**UI:** no GUI, installer, login, or window interaction.

Mac Codex: pull the current `m0/p0-2-cursor` branch and read
`docs/34_CURRENT_IMPLEMENTATION_HANDOFF.md` plus
`docs/adr/ADR-0012-update-orchestration-candidate.md`. Report the exact source
commit tested. Do not change files or commit; return the command output to the
Windows implementer so evidence and handoff remain single-writer.

Run from the repository root with the pinned existing Node runtime:

```sh
node --version
node --test tools/m0-lifecycle-compare/scripts/package-transaction.test.mjs
node --check tools/m0-lifecycle-compare/scripts/package-transaction.mjs
node --check tools/m0-lifecycle-compare/scripts/package-transaction.test.mjs
git diff --check
```

Expected: **31 tests**, zero failures and skips. Confirm the two focused tests
pass:

- `stale concurrent transition fails closed after another candidate commits`
- `concurrent commit between selection check and intent is detected`

The five existing SIGKILL boundary tests must also pass. Report the exact Node
version, OS/architecture, source SHA, test totals and exit status. Do not repeat
the Mac real-health, installer, GUI, or lifecycle suites for this request.

This validates the test-only filesystem candidate on Mac. It does not establish
cross-process stress safety, a native process lock, installed updater handover,
publisher authenticity, canonical-data acceptance, or ADR-0012 acceptance.
