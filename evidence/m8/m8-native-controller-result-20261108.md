# Verified actual native controller pilot result

Date: 2026-11-08. Requested explicit host-to-VM transfer/restart/publication round
passed. This is a bounded synthetic pilot result, not M8/release completion.

## Exact receipt bindings

- Result ZIP: `C:\Users\Tarik\Downloads\DENETLEYICI-SONUC-45278d8e-a4a65ab5fda6.zip`.
- SHA-256: `3e46f449f9548720d7a11759cf45e41fbc3540d6f87f5415121a8291535dd989`.
- Exact six members: runner-result.json, result.json, build.log, native-test.log,
  checkpoint.txt, relay.json. No duplicate/unexpected member accepted.
- Source: `c669a9a4f653fdfd759ef4c9250c834d664d26d0`.
- Source ZIP SHA-256: `eee2a0cfd46aa28f8969baeea6d9301f11c95188c35c7b43354edb9a8dd915b8`.
- Session: `45278d8e00394a62b8d3607208858878`.
- Receiver: `ADJ25H6-ITNXBUJ-FCYCXVA-2TKOTEE-EQXHY5S-VJRGRS7-L6ICJ7Q-EQA2PQU`.
- Relay: `L522WBJ-BF7DO2L-MBUCWLS-JXPUBRO-BKB6ZFD-IUC2MVY-QLLUQ6T-43FUNQK`.
- OS: Windows 10.0.26200.0, NTFS.
- Build, test and runner exits all zero. Build log contains successful test build.
- Exact native ignored opt-in test explicitly selected/executed: 1 passed,
  0 failed, 0 ignored, 592 filtered; elapsed 108.15 seconds. Not a full-suite run.
- Host relay-result.json SHA-256:
  `463e32c4ce606413f5c40af515603d55f1cf101b7e534674ce97796496730569`.
  Independently reports peer_completion_observed, clean_shutdown=true, expected
  receiver/session, 6557 bytes. Host no longer running.

## Assertions observed and verified

Log checkpoints appear in exact order:

1. owned_rest_ready
2. waiting_for_host_delivery
3. actual_delivery_verified_before_publication
4. owned_job_crash_restarted_identity_unchanged
5. worker_publication_and_real_ipc_search_passed
6. restart_replay_no_duplicate_passed

Final checkpoint agrees. Native result flags all true: passed, real_owned_rest,
native_job_crash_before_publication, restart_identity_unchanged,
real_ipc_busy_and_search, immutable_segment_hashes, clean_shutdown,
synthetic_api_credential_deleted. Published one segment with three events;
reopened replay reports one identical segment and no new publication, with
identical complete canonical event hashes. Existing canonical bytes preserved.

Runner/native/host sessions and identities match the delivered package sidecar
and request. Runner outcome is native_controller_pilot_passed; source/archive
digests match the exact corrected long-path candidate. No failed-test or cleanup
warning occurs in the returned native log. Source and all bounded native flags,
summary cardinality and exit codes were checked programmatically in receipt review.

## Scope and next work

Actual network crosses Windows host and VMware guest using Syncthing 2.1.5.
Canonical source/receiver keys and signed reciprocal approvals are synthetic;
the host is an untrusted byte relay. Peer/folder exception and fault trigger are
test-only; ordinary product configuration is still strict. Native named-pipe
requests exercise actual admission/publication/search, while peer authentication
uses the existing test seam rather than native Desktop attestation.

No production profile accessed, no security setting changed, no physical power
cut tested. Interrupted native job is after completed delivery and before
publication, not during network write. No normal application transport activation
or automatic startup feeder is established. Remaining milestone gates include
Mac production publication/endpoint parity, physical durability, cross-device
offline/restriction integration, required CI and human review.

This pilot round is complete: owner need not rerun packages, pairing or keep VM
and test windows open. Preserve the retained private session and ZIP bindings;
continue from explicit unfinished gates rather than repeating this test.
