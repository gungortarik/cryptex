"""Independent structural schema checks and re-signed public-synthetic reader cases.

JSON Schema is deliberately not a signature, canonical-byte or trust verifier.
All seeds used here are already public reference-fixture data. No Cryptex Rust
writer/verifier is called to compute this test oracle.
"""
import argparse
import copy
from collections import Counter
import json
from pathlib import Path

from jsonschema import Draft7Validator, FormatChecker
from generate import ROOT, ZERO, digest, jcs, preimage, sign

OUTPUT = ROOT / "crates/events/tests/fixtures/m8-schema-conformance.json"
OLD_READER = ROOT / "crates/events/tests/fixtures/m8-old-reader-v1-v3.json"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--write-cases", action="store_true")
    args = parser.parse_args()
    data = json.loads((ROOT / "crates/events/tests/fixtures/m8-protocol-v1.json").read_text(encoding="utf-8"))
    checker = FormatChecker()
    assert "date-time" in checker.checkers, "install pinned format dependencies"
    validators = {}
    for kind, name in [("message", "cryptex-bootstrap-v1"),
                       ("transcript", "cryptex-bootstrap-transcript-v1"), ("event", "cryptex-event-v3")]:
        schema = json.loads((ROOT / f"packages/schemas/{name}.schema.json").read_text(encoding="utf-8"))
        Draft7Validator.check_schema(schema)
        validators[kind] = Draft7Validator(schema, format_checker=checker)
    identities = data["identities"]
    cases = []

    def resign(kind, value, who):
        value = copy.deepcopy(value)
        if kind == "transcript":
            return value
        value.pop("signature", None)
        if kind == "message":
            value["signature"] = sign(who["seed"], preimage("cryptex-peer-bootstrap-sig-v1", value))["signature"]
        else:
            value.pop("content_hash", None)
            content = digest("cryptex-security-content-v1", value)
            value["content_hash"] = content
            value["signature"] = sign(who["seed"], b"cryptex-security-sign-v1\0" + bytes.fromhex(content))["signature"]
        return value

    def add(name, kind, value, who, structural=True, runtime=True, sign_again=False):
        if sign_again:
            value = resign(kind, value, who)
        accepted = validators[kind].is_valid(value)
        assert accepted == structural, f"schema decision mismatch: {name}, expected {structural}"
        cases.append({"name": name, "kind": kind, "wire": jcs(value).decode(),
                      "public_key": who["public_key"], "schema_accepts": structural, "runtime_accepts": runtime})

    messages = [json.loads(item["wire"]) for item in data["bootstrap"]["messages"]]
    bases = []
    for index, message in enumerate(messages):
        who = next(identity for identity in identities if identity["public_key"] == message["sender_public_key"])
        bases.append((f"message-{index}", "message", message, who))
    for index, item in enumerate(data["security_events"]):
        value = json.loads(item["wire"])
        who = next(identity for identity in identities if identity["signing_key_id"] == value["signing_key_id"])
        bases.append((f"event-{index}", "event", value, who))

    for name, kind, base, who in bases:
        add(name, kind, base, who)
        # Every root/payload field is required. Auth-field removal must remain
        # absent, so it is not repaired by the independent signing helper.
        for container in [(), ("payload",)]:
            original = base if not container else base["payload"]
            for field in original:
                for mutation in ["missing", "null"]:
                    if mutation == "null" and original[field] is None:
                        continue  # Required offer peer fields are already null.
                    value = copy.deepcopy(base)
                    target = value if not container else value["payload"]
                    if mutation == "missing":
                        del target[field]
                    else:
                        target[field] = None
                    can_resign = bool(container) or field not in ("signature", "content_hash")
                    add(f"{name}-{'payload' if container else 'root'}-{field}-{mutation}",
                        kind, value, who, False, False, can_resign)
            value = copy.deepcopy(base)
            target = value if not container else value["payload"]
            target["unexpected"] = True
            add(f"{name}-{'payload' if container else 'root'}-unknown", kind, value, who, False, False, True)
        value = copy.deepcopy(base)
        value["payload"] = list(value["payload"].values())
        add(f"{name}-positional-payload", kind, value, who, False, False, True)
        value = copy.deepcopy(base)
        value["signature"] = "00" * 64
        add(f"{name}-shape-alone-does-not-authorize", kind, value, who, True, False)
        for field in ["device_id"] if kind == "event" else ["sender_device_id"]:
            for invalid in ["CON", "com9", "Lpt1", "has space", "../escape", "a" * 65, "valid\n"]:
                value = copy.deepcopy(base)
                value[field] = invalid
                add(f"{name}-{field}-{invalid!r}", kind, value, who, False, False, True)
        if kind == "message" and base["message_type"] == "approval":
            for invalid in [{"initiator": None}, {"responder": None}, True, 1, ["initiator"]]:
                value = copy.deepcopy(base)
                value["payload"]["role"] = invalid
                add(f"{name}-role-{invalid!r}", kind, value, who, False, False, True)
        if kind == "event":
            if base["event_type"] == "security.peer_approved":
                for label, field, replacement, shape in [
                    ("wrong-brain-binding", "brain_id", "99999999-9999-4999-8999-999999999999", True),
                    ("wrong-transaction-binding", "transaction_id", "cd" * 32, True),
                    ("empty-inner-objects", "transcript", [{}, {}, {}, {}], False),
                    ("reversed-inner-messages", "transcript", list(reversed(messages)), False),
                ]:
                    value = copy.deepcopy(base)
                    target = value if field == "brain_id" else value["payload"]
                    target[field] = replacement
                    add(f"{name}-{label}", kind, value, who, shape, False, True)
            for invalid in ["0", "01", "-1", "1e2", "1\n", str(2**64)]:
                value = copy.deepcopy(base)
                value["device_seq"] = invalid
                add(f"{name}-sequence-{invalid!r}", kind, value, who, False, False, True)
            for seq_name, hash_name in [("cutoff_seq", "cutoff_content_hash"),
                                        ("last_authorized_seq", "last_authorized_content_hash")]:
                if seq_name not in base["payload"]:
                    continue
                for invalid in ["00", "-1", "1.0", "1\n", str(2**64)]:
                    value = copy.deepcopy(base)
                    value["payload"][seq_name] = invalid
                    add(f"{name}-{seq_name}-{invalid!r}", kind, value, who, False, False, True)
                value = copy.deepcopy(base)
                value["payload"][hash_name] = "11" * 32 if base["payload"][seq_name] == "0" else ZERO
                add(f"{name}-cutoff-hash-pair", kind, value, who, False, False, True)

    add("transcript-ordered", "transcript", messages, identities[0])
    for label, value in [("reversed", list(reversed(messages))), ("short", messages[:3]),
                         ("long", messages + [messages[3]])]:
        add(f"transcript-{label}", "transcript", value, identities[0], False, False)

    # Equal-length, exact upper-bound lexical probes. These are structural
    # checks only; a valid history prefix and allocator are separate authority.
    restricted = next(json.loads(item["wire"]) for item in data["security_events"]
                      if json.loads(item["wire"])["event_type"] == "security.device_restricted")
    for cutoff in [0, 1, 2**64 - 1]:
        value = copy.deepcopy(restricted)
        value["payload"]["cutoff_seq"] = str(cutoff)
        value["payload"]["cutoff_content_hash"] = ZERO if cutoff == 0 else "cc" * 32
        add(f"cutoff-bound-{cutoff}", "event", value, identities[0], True, True, True)
    assert Counter(case["kind"] for case in cases) == {"message": 188, "event": 303, "transcript": 4}, "conformance inventory changed"
    document = {"format_version": 1, "scope": "PUBLIC SYNTHETIC structural/cryptographic conformance; not canonical target-history or trust acceptance", "cases": cases}
    encoded = json.dumps(document, ensure_ascii=False, indent=2) + "\n"
    baseline = json.loads(data["mixed_chain"][0]["wire"])
    next_record = json.loads(data["security_events"][0]["wire"])
    next_record["device_seq"] = "3"
    next_record["prev_content_hash"] = baseline["content_hash"]
    next_record = resign("event", next_record, identities[0])
    assert validators["event"].is_valid(next_record)
    old_reader = json.dumps({"format_version": 1,
        "scope": "PUBLIC SYNTHETIC v1 positive old-product baseline followed by independently signed valid v3; not an installed Desktop test",
        "brain_id": baseline["brain_id"], "device_id": baseline["device_id"],
        "event_id": baseline["event_id"], "public_key": identities[0]["public_key"],
        "baseline_wire": jcs(baseline).decode(), "v3_wire": jcs(next_record).decode()}, indent=2) + "\n"
    if args.write_cases:
        OUTPUT.write_text(encoded, encoding="utf-8", newline="\n")
        OLD_READER.write_text(old_reader, encoding="utf-8", newline="\n")
    else:
        assert OUTPUT.read_text(encoding="utf-8") == encoded, "schema conformance fixture drift"
        assert OLD_READER.read_text(encoding="utf-8") == old_reader, "old-product fixture drift"
    print(f"m8_schema_structural_and_resigned_cases={len(cases)}")


if __name__ == "__main__":
    main()
