# Package fixture temporary-path regression fix — Windows PASS

Base `12f76ac`; Mac evidence review confirmed 28 passing tests only with a
real-path TMPDIR override, and default `/var` alias initialization failures.
The original Mac failure and its conditional PASS remain historical evidence.

The fixture allocator now resolves only the OS temporary directory before
creating a unique owned test root. Package root/source paths are not rewritten;
the transaction engine is unchanged and still rejects links/junctions.
Fixture teardown is registered immediately after allocation, before copying
payloads or initializing an experiment, so initialization failures do not leave
unregistered test roots.

Added one regression: allocate through a temporary directory alias, verify the
returned root uses its real parent, initialize through the real path, and assert
the same engine still rejects access through the alias with `UNSAFE_PATH`.
Windows uses an ordinary directory junction without changing security settings.

Checks on Windows Node `v24.19.0`:

- Unprefixed package transaction suite: **29/29 PASS**, no skipped tests.
- New allocator/test syntax checks: PASS.
- `git diff --check`: PASS.

No production/app source, package engine, retained package, product runtime,
startup registration, P0-1 corpus, GUI/focus or GitHub workflow changed.
The prior eight retained-binary-copy scenarios were not repeated because this
change affects only unit-test fixture allocation and teardown.

Mac must rerun the ordinary unprefixed command with its original default TMPDIR
and expect 29 passing tests. The Windows alias regression is not a substitute
for that actual Mac verification. ADR-0012 and native integration remain open.
