# Cryptex: Intellectual Property and Contribution Governance

**Status:** Project governance guidance (2026-10-31). This page is not a replacement license, contributor assignment agreement, or legal advice.

## Maintainer and decision authority
- The repository is administered by @gungortarik. The project maintainer decides what is merged, released, or included in official Cryptex distributions.
- External contributors are welcome to submit issues and pull requests from forks. A pull request does not convey repository administration or ownership.
- `.github/CODEOWNERS` requests maintainer review. CODEOWNERS alone is **not** an enforcement mechanism: branch protection/rulesets must separately be enabled in GitHub repository settings.
- Never grant write, maintain, or admin access solely to enable an external contribution.

## Existing open-source rights
- The repository currently declares `MIT OR Apache-2.0` in `Cargo.toml` and contains `LICENSE-MIT` and `LICENSE-APACHE`.
- Those licenses permit certain copying, modification, redistribution and commercial use. Their conditions, including applicable notices, must be respected.
- Making the repository private later, or modifying a license notice, does **not** retroactively revoke licenses already granted to recipients of past licensed releases.
- The current open-source licenses do not grant exclusive rights to a business model or prevent independent competing products.
- Do not remove third-party notices, relicense someone else's contributions, or assume Cryptex owns contributions exclusively without a valid separate agreement.

## Contributor rights and verification
- Contributors must submit only material they have authority to license. They must preserve upstream/third-party copyright, license, and attribution notices.
- Contributions submitted for inclusion in currently open-source code should be compatible with the applicable project licenses.
- A separate contributor license agreement (CLA) **has not been adopted**. Any draft is nonbinding until approved and signed through a documented process.
- Before adopting a CLA or dual-licensing/assignment scheme, obtain qualified Canadian IP legal review and decide how signatures and records will be managed. This protects contributors as well as the maintainer.
- Maintainers may decline, pause or request revisions to any contribution.

## Security and review
- Require maintainer review and passing CI before merging, via repository branch protection/rulesets.
- Do not merge new PRs that touch license texts, branding, identity/trust, key management, encryption, data deletion, or personal-memory permissions without focused technical and licensing review.
- Do not commit secrets, tokens, personal data, runtime databases, or private memory.
- External contributors should work in forks; the maintainer retains merge and release decisions.

## Future commercial products
- An official commercial distribution, hosted service, enterprise support offering, or separate proprietary module may be offered in the future, subject to applicable third-party licenses and contributor permissions.
- Keep future **unpublished** proprietary features and trade secrets in separate private repositories; never copy code without satisfying its licenses.
- Do not promise an exclusive commercial license over contributions unless their rights are properly secured.
- For major proprietary commercialization or relicensing, have counsel review the provenance and licensing of all included code.

## Cryptex name and visual identity
- Software copyright licensing and trademark rights are different. The source-code licenses do not by themselves confer any trademark registration or exclusive legal right to the project name.
- Do not imply that an unofficial fork or unrelated product is endorsed by the official Cryptex maintainer.
- Trademark clearance/registration for the name and logos is **not confirmed**. Conduct CIPO and other relevant marketplace searches before registration or claims of registered status.
- Do not use the registered-mark symbol (®) without an applicable registration.

## Checklist for the maintainer
1. Enable a GitHub ruleset/branch protection for `m0/foundation`: require PRs, code-owner review, prevent deletion and force pushes; configure a safe maintainer bypass for solo-maintainer work.
2. Require CI checks **after they have run successfully at least once** on target PRs.
3. Review and sign off on an appropriate contributor-rights process with legal advice before relying on CLA/assignment.
4. Check third-party licenses and provenance before any commercial redistribution.
5. Consider separate private modules and brand clearance when commercial plans become concrete.
