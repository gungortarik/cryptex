# Syncthing 2.1.5 notice inventory

The `LICENSE.txt` and `AUTHORS.txt` notice text came from the retained official
Windows 2.1.5 package installed through WinGet. Its wording is retained; the
repository representation uses **LF** and explicit Git attributes to preserve
identical checkout bytes on both platforms. No binary is vendored
in Git. This inventory is for the M0 managed integration spike; it does not
claim that Cryptex installers currently contain Syncthing or its notices.

| File | Repository LF SHA-256 | Retained upstream Windows CRLF SHA-256 |
| --- | --- | --- |
| LICENSE.txt | `3f3d9e0024b1921b067d6f7f88deb4a60cbe7a78e76c64e3f1d7fc3b779b9d04` | `9221c2f936159b8446d329249fb4c0f25be510f447383a0f13336ac7985668a3` |
| AUTHORS.txt | `5a0044d13ddf6f013bdd5c2bc419bf45d6123c356567510237e82f304d113d48` | `2c197afd6113ec13ae134a20ce8f101d9812b078c3ef32c42c3bb4cdaf2b96a2` |

`f17bcf7` fixes the initial error of pinning Windows checkout CRLF hashes while
Git blobs/Mac checkouts were LF. Packager checks the exact LF digests and copies
those bytes unchanged; it does not normalize arbitrary input at runtime or
accept alternate digests. Retained earlier CRLF proof bundles remain historical
artifacts; do not rewrite them or adopt them with the new verifier.

Upstream release/source: <https://github.com/syncthing/syncthing/releases/tag/v2.1.5>
and <https://github.com/syncthing/syncthing/tree/v2.1.5>.
The supplied license is Mozilla Public License 2.0. Preserve the upstream
license, attribution and available source link with any distributed binary;
verify dependency notices and actual installer inclusion before distribution.
Any changed upstream source requires its own provenance/source review.
Release checksums identify supplied bytes; they do not replace publisher
authentication or platform code-signing verification.
