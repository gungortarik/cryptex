# Native controller pilot: host relay transfer

Date: 2026-11-08. Receiver result ZIP remains required before claiming the
native controller publication/restart/replay test passed.

## Request and source binding

- Request: `C:\Users\Tarik\Downloads\DENETLEYICI-ISTEK-d1e5c535-5b92b54889a6.zip`.
- Request SHA-256: `7a501ff89fd87cc12c1c4a30d68767c53bf30dbd32e4264cacb2eb2b7a0c2f8b`.
- Session: `d1e5c535ae0e459fb699be8f7324c149`.
- Receiver: `7ZXBOHM-RJINYKJ-PFJLMM7-FOCEAST-QVHSCQS-RP3PHD3-OWGDSK4-VPNBSQN`.
- Relay: `LJPWM5V-AENEGPO-ZVZKRVQ-Y7ZKZWN-ROEYYK2-ZMLREP3-4PSTWPD-3VPY4QL`.
- One synthetic public segment, 6557 bytes, SHA-256
  `3B605CB84596D5908769BE3485BB70A651ADD7633320EC4825CBCBA8E56ADC12`.
- Relative segment path:
  `63323936383531302d663362622d346137392d626330662d663762636663306564643032/00000000000000000000.seg`.
- Expected VM candidate `393d0c4dcdfc36ddd72c0d6386d067ada6634d0c`, source archive
  `c039876748e35f947661d9a754f6465b3464d1c39b536f3ce906192cd9505e92`.
  The public request itself does not attest that candidate; verify the receiver
  runner receipt and logs against these values when returned.

## Host observations

Closed ZIP/member/path/size/digest/session validation passed before native launch.
Started only the retained isolated send-only host relay on private VMware address
`192.168.177.1:4147`; API remained loopback. No owner profile accessed.
Native authenticated device connection observed from VM `192.168.177.131`.
Connection closed and was re-established; this alone does not prove the intended
controller crash/restart assertions. Peer completion reports 100%, globalBytes
6557, needBytes 0, needItems 0. No canonical publication is inferred from this.

Host relay script exited 0 with `peer_completion_observed`; final receipt reports
`clean_shutdown=true`, the expected receiver and 6557 bytes. Native log confirms
listener shutdown and exit. The 120-second completion keepalive finished.
VM result remains pending.
Private host session: `%LOCALAPPDATA%\Cryptex-Dev\native-controller-relay\d1e5c535ae0e459fb699be8f7324c149`.

Next: collect `DENETLEYICI-SONUC-d1e5c535-5b92b54889a6.zip`. Verify exact source,
exit 0 and one passed/no failed/no ignored, native receipt identity/segment counts,
all required checkpoint flags and credential cleanup. M8 remains open.
