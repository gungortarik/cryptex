"""Independent wire fixtures. ALL seeds/identities are public synthetic test data.

No Cryptex executable/library is invoked. RFC8785 supplies JCS, the official
BLAKE3 Python binding supplies hashing, Node's built-in OpenSSL Ed25519 signs.
This is an offline developer tool, never a runtime writer or key generator.
"""

import argparse
import copy
import hashlib
import json
from pathlib import Path
import subprocess

import blake3
import rfc8785


ROOT = Path(__file__).resolve().parents[2]
OUTPUT = ROOT / "crates/events/tests/fixtures/m8-protocol-v1.json"
ZERO = "00" * 32
BRAIN = "22222222-2222-4222-8222-222222222222"
TX = "ab" * 32
TIME = "2026-01-01T00:00:00Z"
NODE = r"""
const crypto = require('node:crypto');
let input = '';
process.stdin.setEncoding('utf8');
process.stdin.on('data', x => input += x);
process.stdin.on('end', () => {
  const {seed, preimage} = JSON.parse(input);
  const key = crypto.createPrivateKey({format:'der', type:'pkcs8',
    key:Buffer.from('302e020100300506032b657004220420' + seed, 'hex')});
  const publicKey = crypto.createPublicKey(key);
  const bytes = Buffer.from(preimage, 'hex');
  const signature = crypto.sign(null, bytes, key);
  if (!crypto.verify(null, bytes, publicKey, signature)) process.exit(2);
  process.stdout.write(JSON.stringify({
    public_key: publicKey.export({format:'der', type:'spki'}).subarray(-32).toString('hex'),
    signature: signature.toString('hex')}));
});
"""


def jcs(value):
    return rfc8785.dumps(value)


def preimage(domain, value):
    return domain.encode("ascii") + b"\0" + jcs(value)


def digest(domain, value):
    return blake3.blake3(preimage(domain, value)).hexdigest()


def sign(seed, data):
    result = subprocess.run(
        ["node", "-e", NODE], input=json.dumps({"seed": seed, "preimage": data.hex()}),
        capture_output=True, text=True, check=True,
    )
    return json.loads(result.stdout)


def identity(seed_byte, device, transport_letter):
    seed = seed_byte * 32
    public = sign(seed, b"")["public_key"]
    fingerprint = hashlib.sha256(bytes.fromhex(public)).hexdigest()
    return {"seed": seed, "device_id": device, "public_key": public,
            "signing_key_id": fingerprint[:32], "fingerprint": fingerprint,
            "transport_id": "-".join([transport_letter * 7] * 8)}


def bootstrap_message(kind, sender, peer, previous, payload):
    value = {"protocol_version": 1, "message_type": kind, "transaction_id": TX,
             "brain_id": BRAIN, "previous_hash": previous, "payload": payload}
    for prefix, who in [("sender", sender), ("peer", peer)]:
        for field in ["device_id", "signing_key_id", "public_key"]:
            value[f"{prefix}_{field}"] = who[field] if who else None
        value[f"{prefix}_transport_device_id"] = who["transport_id"] if who else None
    data = preimage("cryptex-peer-bootstrap-sig-v1", value)
    value["signature"] = sign(sender["seed"], data)["signature"]
    return {"wire": jcs(value).decode(), "signature_preimage_hex": data.hex(),
            "message_hash": digest("cryptex-peer-bootstrap-message-v1", value)}


def event(schema, number, seq, event_type, payload, previous, author):
    value = {"schema_version": schema, "event_id": f"44444444-4444-4444-8444-{number:012d}",
             "brain_id": BRAIN, "device_id": author["device_id"], "device_seq": str(seq),
             "created_at": TIME, "event_type": event_type, "classification": "internal",
             "signing_key_id": author["signing_key_id"], "payload": payload,
             "prev_content_hash": previous}
    if schema == 2:
        value["payload_schema_version"] = 1
    content_domain = ("cryptex-security-content-v1" if schema == 3
                      else f"cryptex-event-content-v{schema}")
    sig_domain = ("cryptex-security-sign-v1" if schema == 3
                  else f"cryptex-event-sig-v{schema}")
    content_preimage = preimage(content_domain, value)
    content_hash = blake3.blake3(content_preimage).hexdigest()
    signature_preimage = sig_domain.encode() + b"\0" + bytes.fromhex(content_hash)
    value["content_hash"] = content_hash
    value["signature"] = sign(author["seed"], signature_preimage)["signature"]
    out = {"wire": jcs(value).decode(), "content_preimage_hex": content_preimage.hex(),
           "signature_preimage_hex": signature_preimage.hex(), "content_hash": content_hash}
    if schema == 3:
        operation = {key: value[key] for key in ["schema_version", "brain_id", "device_id",
                                                "event_type", "classification", "signing_key_id", "payload"]}
        out["operation_digest"] = digest("cryptex-security-operation-v1", operation)
    return out


def approval_identity(who):
    return {"device_id": who["device_id"], "signing_key_id": who["signing_key_id"],
            "public_key_fingerprint_sha256": who["fingerprint"]}


def corpus():
    a = identity("11", "11111111-1111-4111-8111-111111111111", "A")
    b = identity("22", "33333333-3333-4333-8333-333333333333", "B")
    offer = bootstrap_message("offer", a, None, ZERO, {"initiator_nonce": "11" * 32})
    proof = bootstrap_message("proof", b, a, offer["message_hash"],
                              {"offer_hash": offer["message_hash"], "responder_nonce": "22" * 32})
    agreement = {"offer_hash": offer["message_hash"], "proof_hash": proof["message_hash"],
                 "brain_id": BRAIN, "initiator_identity": {k: a[k] for k in ["device_id", "signing_key_id", "public_key"]},
                 "responder_identity": {k: b[k] for k in ["device_id", "signing_key_id", "public_key"]},
                 "transport_association": {"initiator_transport_device_id": a["transport_id"],
                                           "responder_transport_device_id": b["transport_id"]}}
    agreement_hash = digest("cryptex-peer-bootstrap-agreement-v1", agreement)
    first = bootstrap_message("approval", a, b, proof["message_hash"],
                              {"agreement_hash": agreement_hash, "role": "initiator"})
    second = bootstrap_message("approval", b, a, first["message_hash"],
                               {"agreement_hash": agreement_hash, "role": "responder"})
    messages = [offer, proof, first, second]
    transcript = [json.loads(message["wire"]) for message in messages]
    peer = event(3, 1, 1, "security.peer_approved", {"transaction_id": TX, "transcript": transcript}, ZERO, a)
    restricted = []
    for number, cutoff in enumerate([0, 1, 2**64 - 1], 2):
        restricted.append(event(3, number, 2, "security.device_restricted", {
            "transaction_id": TX, "target_device_id": b["device_id"],
            "target_signing_key_id": b["signing_key_id"], "target_public_key": b["public_key"],
            "reason": "compromised", "cutoff_seq": str(cutoff),
            "cutoff_content_hash": ZERO if cutoff == 0 else "cc" * 32}, peer["content_hash"], a))
    retirement = event(3, 5, 3, "security.device_self_retired", {
        "transaction_id": TX, "last_authorized_seq": "2",
        "last_authorized_content_hash": restricted[1]["content_hash"]}, restricted[1]["content_hash"], a)
    chain = [event(1, 6, 1, "memory.note", {"text": "Synthetic café / 🧠\nline two"}, ZERO, a)]
    chain.append(event(2, 7, 3, "knowledge.assertion", {"kind": "entity.declare",
        "entity_id": "55555555-5555-4555-8555-555555555555", "entity_kind": "concept",
        "label": "Synthetic café", "publication_authorization": {"basis": "direct_user",
        "receipt_id": "66666666-6666-4666-8666-666666666666"}}, chain[-1]["content_hash"], a))
    chain.append(event(3, 8, 4, "security.peer_approved", {"transaction_id": TX, "transcript": transcript}, chain[-1]["content_hash"], a))
    chain.append(event(1, 9, 6, "memory.note", {"text": "Synthetic after v3"}, chain[-1]["content_hash"], a))
    snapshots = []
    for role, local, remote, previous in [("initiator", a, b, proof["message_hash"]),
                                         ("responder", b, a, first["message_hash"])]:
        snapshot = {"brain_id": BRAIN, "transaction_id": TX, "role": role,
            "operation": "peer_admission", "agreement_hash": agreement_hash, "previous_message_hash": previous,
            "local_identity": approval_identity(local), "peer_identity": approval_identity(remote),
            "transport_association": {"local_device_id": local["transport_id"], "peer_device_id": remote["transport_id"]},
            "restriction_target": None, "restriction_reason": None, "cutoff_seq": None,
            "cutoff_content_hash": None, "last_authorized_seq": None, "last_authorized_content_hash": None}
        snapshots.append(snapshot)
    restriction = copy.deepcopy(snapshots[0])
    restriction.update(role="issuer", operation="device_restriction", agreement_hash=None,
                       previous_message_hash=None, restriction_target=b["device_id"],
                       restriction_reason="compromised", cutoff_seq="1", cutoff_content_hash="cc" * 32)
    snapshots.append(restriction)
    retirement_snapshot = copy.deepcopy(restriction)
    retirement_snapshot.update(operation="self_retirement", peer_identity=None, transport_association=None,
        restriction_target=None, restriction_reason=None, cutoff_seq=None, cutoff_content_hash=None,
        last_authorized_seq="2", last_authorized_content_hash=restricted[1]["content_hash"])
    snapshots.append(retirement_snapshot)
    return {"format_version": 1, "warning": "PUBLIC SYNTHETIC TEST SEEDS. NEVER USE FOR REAL IDENTITIES.",
        "identities": [a, b], "bootstrap": {"messages": messages, "agreement_hash": agreement_hash,
        "transcript_hash": digest("cryptex-peer-bootstrap-transcript-v1", transcript)},
        "security_events": [peer, *restricted, retirement], "mixed_chain": chain,
        "approval_snapshots": [{"snapshot": snapshot, "digest": digest("cryptex-owner-approval-request-v1", snapshot)}
                               for snapshot in snapshots]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    data = json.dumps(corpus(), ensure_ascii=False, indent=2) + "\n"
    if args.check:
        if OUTPUT.read_bytes() != data.encode("utf-8"):
            raise SystemExit("Independent M8 fixture drift")
        print("Independent M8 fixtures match")
    else:
        OUTPUT.parent.mkdir(parents=True, exist_ok=True)
        OUTPUT.write_bytes(data.encode("utf-8"))
        print(f"Wrote {OUTPUT.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
