# Signed public bootstrap: personal-Mac owner receipt

Status: **isolated signed candidate passed; product integration and M8 OPEN**.
Received2026-11-10 local; collector timestamp2026-11-11T03:35:59Z.
[Candidate, exact CI and review](m8-macos-hostile-bootstrap-signed-candidate-20261110.md).
[Verified machine-readable receipt](m8-macos-signed-bootstrap-owner-result-20261110.json).

## Provenance and verification

- Input: `C:\Users\Tarik\Downloads\MAC-MESAJ-SONUC.zip`,1574 bytes.
- ZIP SHA256: `094a2da88f61c04cac5f839d8b9cbd2d22271b635205d0b96332c866771d3d0f`.
- Sole member: result.json,5321 bytes; SHA256
  `9787b29f9343b281ec20798546c88b1835ada2e7013d5b5c357962429e78744b`.
- Source: `8e8fabbc8e50bcbb65f889a3de5b2156f0cc836b`.
- Native C SHA256: `d6f66eaa4cdeb6a6395da298ea9e0145a847aac88e4198e09bc10c988c1dab18`.
- Policy-header SHA256: `fb36977d15bbef59d4111343d052227e498909b69c872e6c453256b3933d6347`.
- Host reported macOS27.2, SDK27.2, arm64. No minimum-OS acceptance follows.

The archive was read without extracting/executing its contents. Duplicate JSON
keys were refused. Both native source hashes match exact committed Git bytes.
The repository's strict signed parser revalidated the capability and all rows;
completion was recomputed from actual observations rather than trusting the
aggregate candidate_complete flag. All rows, fault controls, process termination,
cleanup and explicit product-exclusion flags were checked. The JSON above is
preserved byte-for-byte; no secret, certificate selector, TeamID, audit-token bytes
or memory bodies appear in this closed report. This is an owner-supplied runtime
receipt, not independently attested execution or publisher acceptance.

## Native observations

Compilation and signing with one existing certificate succeeded. The certified
two-binary/self-bundle policy reported eligible=true with
`verified_self_organization_two_binary_bundle`. Desktop and Daemon have distinct
reported signed-image hashes, retained in the JSON. Both native and runner exits0.

All **19 required rows passed**:

| Observation | Actual stage / code |
| --- | --- |
| Wrong-role signed image refused | static_code /55 |
| Ad-hoc peer image refused | static_code /55 |
| Active service-name collision refused | check_in /1103 |
| Same-image imposter refused | validate /52 |
| Genuine two-way exchange after imposter | validate /0 |
| Same-sender replay refused | validate /56 |
| Wrong session refused | validate /51 |
| Short frame refused | validate /50 |
| Complex frame refused | validate /50 |
| Same-PID exec generation refused | validate /53 |
| Queued frame after sender exit refused | validate /54 |
| Fresh child/session exchange | validate /0 |
| Supervisor deadline distinct | supervisor /63 |
| Substituted endpoint refused | child_exit /52 |
| Transferred reply right refused | child_exit /52 |
| Transferred service right refused | validate /52 |
| Genuine victim succeeds after transferred-right attack | validate /0 |
| Normal service namespace independently absent | independent_lookup /1102 |
| Tracked normal owned-right cleanup succeeds | cleanup /0 |

The four additional fault controls match exact expected child exits: reaper0,
cleanup68, send-timeout67 and invalid send-once header59. Separate forced exit
observed signal9, followed by independent namespace removal and cleanup_exit0.
ci_only=false and candidate_complete=true are consistent with these observations.

## Continuation and remaining gates

This closes the requested personal-Mac signed-positive19-case experiment for this
exact source/host. Do not rerun this package, signing/API preflight, old descriptor
candidate or completed Windows pairing/transfer packages without a new changed
behavior to verify. No immediate Mac/VM action is needed.

Next work is local design/source review of the remaining bundle snapshot/change
boundary and actual product connection/session lifetime under the accepted
contracts. Preserve this public-message mechanism as an isolated candidate.
The original direct-descriptor carrier remains stopped. Product integration or
replacement of ADR-0007's accepted Unix transport needs separate explicit
acceptance; no production channel switch is implied by this receipt.

Report explicitly leaves production_integration=not_implemented,
production_changed=false, m8_complete=false, publisher_or_notarization_acceptance=false,
product_minimum_os=unchanged_unconfirmed, owner_session_restart=not_tested,
concurrent_bundle_replacement=not_tested and unix_frame_sender_attribution=not_proven.
Broader forwarding/flooding/deadline behavior, supported Mac transport/publication
parity, actual offline/restriction propagation, final security/product and human
review also remain open. This isolated startup experiment does not close M8.
