"""Verify the notice references and embedded bytes in a delivered Rust inventory."""
import base64
import hashlib


def verify_documents(notices):
    packages = notices["packages"]
    documents = notices["documents"]
    identities = {(item["name"], item["version"]) for item in packages}
    if len(identities) != len(packages) or not identities:
        raise ValueError("Empty or duplicate Rust notice package identity")
    if not all(item["documents"] for item in packages):
        raise ValueError("Delivered package lacks its notice")
    referenced = {doc["sha256"] for item in packages for doc in item["documents"]}
    if referenced != documents.keys():
        raise ValueError("Delivered package references differ from the embedded notice documents")
    for expected, item in documents.items():
        if item.keys() == {"text"}:
            content = item["text"].encode("utf-8")
        elif item.keys() == {"base64"}:
            content = base64.b64decode(item["base64"], validate=True)
        else:
            raise ValueError("Unrecognized embedded notice format")
        if not content or hashlib.sha256(content).hexdigest() != expected:
            raise ValueError("Delivered notice bytes differ from their digest")
    return identities
