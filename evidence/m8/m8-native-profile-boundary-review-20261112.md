# M8 native profile boundary review

The implementing agent reviewed the profile/replay changes through
`5b6f8635bca726cc6637fc47000e9972928f3826`, including the owner receipt
`MAC-KALICI-SONUC-be33e9c4.zip`. Corrections are at `b7b6f9b`.
This is a focused source review. Final independent security review and human
diff acceptance remain completion gates.

## Scope

Reviewed the child startup/profile gate, synthetic native domain setup,
same-thread replay probes, protected asset installation, product image-policy
constructor and owner result collector. The accepted boundary is ADR-0020 and
the protected-content proposal. The native C transport was examined for its
sender/image checks. A new full audit of all native port/resource paths was
outside this focused review.

## Corrected findings

- The profile gate read its marker through filesystem links. A symbolic or
  shared hard link containing the expected marker bytes could satisfy the
  fresh-root check. The gate now requires a regular, caller-owned, single-link
  marker without group/other write permissions. Regression coverage creates
  both symbolic and shared hard links in private synthetic TEMP directories.
- The resumed profile's fixed synthetic Brain and original signed probe were
  checked in the native domain driver after the shared lifecycle had already
  begun recovery. The read-only identity/probe check now runs in the child
  profile gate before lifecycle work and repeats under the domain's instance
  guard. It binds the descriptor to the local DeviceId, fixed Brain and fixed
  synthetic seed event. Tests reject absent identity, a foreign Brain and a
  mismatched descriptor without rewriting the supplied metadata.

## Retained boundaries

Replay probes have private bytes, no Debug/Serialize implementation and a
non-Send thread marker. Replays require the exact request bytes and an explicit
missing-session response over the fresh native connection. Tests compare
canonical/identity and journal bytes and exercise fresh approvals.

The protected asset provider uses the verified in-memory inventory. The
candidate renderer receives local handles. The product publisher constructor
remains unconfigured, and ordinary Mac owner routes continue to refuse access.
Current acceptance covers disposable local-certificate fixtures on the tested
Mac and Windows paths.

## Verification and continuation

Local formatting and diff checks passed. The Python suite ran 72 tests with
five platform-specific skips.
Native verification is tracked by
[run 43284483019](https://github.com/gungortarik/cryptex/actions/runs/43284483019).
Both Mac and Windows jobs passed. The Mac feature-enabled checks include the
marker/foreign-profile regressions, actual signed startup, native same-profile
Keychain restart, raw confirmation replay refusal and missing-key refusal.

The next bounded integration step is adding pairing exchange and approval
methods to the disabled native Desktop client using the existing daemon domain
services. The current candidate UI registers the owner-security commands.
Normal product activation still requires the publisher and supported-platform
decisions, reviewed integration, exact CI and human acceptance described in
ADR-0020. The existing local-certificate development path remains available.
