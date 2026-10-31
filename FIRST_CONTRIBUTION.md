# Your first Cryptex contribution

**Welcome!** Cryptex is an open-source, local-first AI memory project. You can make a useful first contribution without learning the complete event-sourcing or cryptography design.

## Choose one small task

1. Browse [good first issues](https://github.com/gungortarik/cryptex/issues?q=is%3Aissue+is%3Aopen+label%3A%22good+first+issue%22). Each should have a small scope and clear acceptance criteria.
2. Read the issue and **comment before starting**. Maintainers can answer questions and help avoid duplicate work.
3. For help picking a task, use [Discussions](https://github.com/gungortarik/cryptex/discussions).

**No repository write access is required.** Contributions come through forks and pull requests. Documentation, reproducible bug reports, accessibility checks, and platform testing are valuable.

## Small-PR workflow

1. [Fork Cryptex](https://github.com/gungortarik/cryptex/fork) to your account.
2. Clone **your** fork and create a branch from the latest designated integration base (currently `m0/foundation`; confirm on the issue before starting).
3. Change only what the issue requests. Do not include unrelated refactors or generated files.
4. Run the tests relevant to your change. For docs-only changes, confirm Markdown links resolve and instructions match the repository.
5. Push your branch and open a pull request against the **base branch agreed on the issue**.
6. In the PR description, link the issue, explain your changes, give brief testing evidence and mention anything you could not test.

See [CONTRIBUTING.md](CONTRIBUTING.md) for local build instructions and [AGENTS.md](AGENTS.md) for project invariants. The [implementation handoff](docs/34_CURRENT_IMPLEMENTATION_HANDOFF.md) is a dated operational snapshot; it is not proof of a stable release.

## Need a development environment?

- **Documentation-only:** a GitHub account and Markdown editor are enough.
- **Frontend / React:** Node version in [`.node-version`](.node-version) and pnpm in [CONTRIBUTING.md](CONTRIBUTING.md). The desktop frontend lives in `apps/desktop/src/`.
- **Rust / MCP:** Rust toolchain in [`rust-toolchain.toml`](rust-toolchain.toml); use the commands in [CONTRIBUTING.md](CONTRIBUTING.md).
- **Native desktop:** Cryptex targets macOS and Windows 11. Native packaging requires additional OS tooling; a first PR does not need to package the app unless the issue says so.

## What to include in your PR

```markdown
Closes #<issue-number>

### Change
- What changed and why

### Checks
- [ ] Relevant checks passed (list commands and platform)
- [ ] No personal runtime data, keys or secrets were committed
- [ ] Links and screenshots are accurate (where applicable)

### Limitations
- Anything I could not test
```

## Safety and project boundaries

- Cryptex is **pre-release**. Distinguish implemented behavior from planned milestones in docs and demos.
- `cryptexd` is the only canonical writer; SQLite and Obsidian notes are derived outputs.
- Use synthetic memory in tests and screenshots. Never commit private vaults, personal notes, credentials or recovery material.
- Preserve macOS and Windows portability; do not introduce one-platform assumptions.
- Do not turn on GitHub Actions, alter branch policies or modify security architecture as part of an unrelated first contribution.
- If an issue needs wider architecture changes, discuss them before implementing.

Thanks for helping make long-term AI memory more reliable and user-controlled.
