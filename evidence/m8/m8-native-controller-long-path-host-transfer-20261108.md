# Corrected Windows long-path pilot: actual host relay transfer

Date: 2026-11-08. Receiver result remains required for native publication pass.

- Request `C:\Users\Tarik\Downloads\DENETLEYICI-ISTEK-45278d8e-a4a65ab5fda6.zip`.
- Request SHA-256 `4e3fb28b8af8c1491f007a068c234a8d6c3389c8ce75137a52e9eb9dde118ffa`.
- Session `45278d8e00394a62b8d3607208858878`.
- Receiver `ADJ25H6-ITNXBUJ-FCYCXVA-2TKOTEE-EQXHY5S-VJRGRS7-L6ICJ7Q-EQA2PQU`.
- Relay `L522WBJ-BF7DO2L-MBUCWLS-JXPUBRO-BKB6ZFD-IUC2MVY-QLLUQ6T-43FUNQK`.
- One signed synthetic public segment, 6557 bytes, SHA-256
  `C3174BB08FB97E5BE2D6F190C36245F1B70263842A377A3F9B2E68A25510CFD2`.
- Relative segment path
  `65616430396537362d643237622d343839382d623833612d313961626436646562353534/00000000000000000000.seg`.
- Expected receiver candidate `c669a9a4f653fdfd759ef4c9250c834d664d26d0`, source ZIP
  `eee2a0cfd46aa28f8969baeea6d9301f11c95188c35c7b43354edb9a8dd915b8`.
  Confirm these in the returned runner receipt, not from the request alone.

Closed member/path/size/hash/session validation passed before sender launch.
Fresh isolated send-only host relay used private endpoint 192.168.177.1:14687
and loopback-only API. Real VM connection and reconnections observed, completion
100%, globalBytes 6557, needBytes 0, needItems 0. Reconnection alone does not prove
the test's crash/restart or publication assertions.

Host script exited 0 after completion keepalive. Final relay-result.json reports
peer_completion_observed, clean_shutdown=true, exact receiver above and 6557 bytes.
Receiver result still pending.
Private retained session: `%LOCALAPPDATA%\Cryptex-Dev\native-controller-relay\45278d8e00394a62b8d3607208858878`.

Next: collect and inspect
`DENETLEYICI-SONUC-45278d8e-a4a65ab5fda6.zip` for exact source binding, one passed/
zero failed/zero ignored, exit statuses, all checkpoints, immutable hashes,
replay counts, real IPC flags and credential cleanup. No M8 completion claim.
