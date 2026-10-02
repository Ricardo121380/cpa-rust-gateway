"""Authenticate private credential envelopes. Never emit plaintext, IDs or key digests.

HChaCha derivation follows draft-irtf-cfrg-xchacha-03 sections 2.2/2.3; its published
block vector is checked before use. AEAD authentication uses installed cryptography/OpenSSL.
This operator probe does not replace gateway startup or alter candidate source.
"""
import json
import pathlib
import sqlite3
import struct
from cryptography.hazmat.primitives.ciphers.aead import ChaCha20Poly1305

def hchacha(key, nonce):
    words = list(struct.unpack("<16I", b"expand 32-byte k" + key + nonce))
    def rotate(n, bits):
        return ((n << bits) | (n >> (32-bits))) & 0xffffffff
    def quarter(a, b, c, d):
        words[a] = (words[a]+words[b]) & 0xffffffff
        words[d] = rotate(words[d] ^ words[a], 16)
        words[c] = (words[c]+words[d]) & 0xffffffff
        words[b] = rotate(words[b] ^ words[c], 12)
        words[a] = (words[a]+words[b]) & 0xffffffff
        words[d] = rotate(words[d] ^ words[a], 8)
        words[c] = (words[c]+words[d]) & 0xffffffff
        words[b] = rotate(words[b] ^ words[c], 7)
    for _ in range(10):
        for indices in [(0,4,8,12),(1,5,9,13),(2,6,10,14),(3,7,11,15),(0,5,10,15),(1,6,11,12),(2,7,8,13),(3,4,9,14)]:
            quarter(*indices)
    return struct.pack("<8I", *(words[:4] + words[12:]))

assert hchacha(bytes(range(32)), bytes.fromhex("000000090000004a0000000031415927")).hex() == "82413b4227b27bfed30e42508a877d73a0f9e4d58a74a853c12ec41326d3ecdc"

def validate_native(provider, plain):
    if provider != "build":
        assert plain.decode("utf-8")
        return
    # oauth.rs persisted_bytes uses a version/source header, signed BE i64 expiry,
    # four length-prefixed UTF-8 fields and optional version-2 identity JSON.
    if plain[:1] not in [b"\x01", b"\x02"]:
        assert isinstance(json.loads(plain), dict)
        return
    assert len(plain)>=10 and plain[1]<=5 and int.from_bytes(plain[2:10],"big",signed=True)>=0
    position=10; fields=[]
    for _ in range(5 if plain[0]==2 else 4):
        assert position+4<=len(plain)
        size=int.from_bytes(plain[position:position+4],"big"); position+=4
        assert 0<size<=65536 and position+size<=len(plain)
        field=plain[position:position+size].decode("utf-8"); position+=size; fields.append(field)
    assert position==len(plain)
    if plain[0]==2: assert isinstance(json.loads(fields[-1]),dict)

def authenticate(db_path, keys_path):
    key = bytearray((pathlib.Path(keys_path)/"master-key").read_bytes())
    assert len(key) == 32
    counts = {"upstream_credentials": 0, "native_grok_accounts": 0}
    failures = {"upstream_credentials": 0, "native_grok_accounts": 0}
    classifications = {}
    versions = set()
    def open_envelope(envelope, version, aad):
        versions.add(version)
        assert version == 1 and len(envelope) >= 41 and envelope[0] == 1
        nonce = envelope[1:25]
        plain = bytearray(ChaCha20Poly1305(hchacha(bytes(key), nonce[:16])).decrypt(b"\0"*4+nonce[16:], envelope[25:], aad))
        assert plain
        return plain
    try:
        with sqlite3.connect(f"file:{db_path}?mode=ro", uri=True) as db:
            db.execute("BEGIN")
            for config, credential, upstream, envelope, version, lifecycle, status in db.execute("SELECT c.config_version_id,c.id,c.upstream_id,c.ciphertext,c.key_version,v.status,c.status FROM upstream_credentials c JOIN config_versions v ON v.id=c.config_version_id"):
                aad = bytearray(b"cpa-rust-gateway/control-plane/credential-aad/v1")
                for field in [config, credential, upstream]:
                    raw = field.encode(); aad.extend(len(raw).to_bytes(4, "big")); aad.extend(raw)
                category = "upstream:"+lifecycle+":"+status
                classifications.setdefault(category,{"authenticated":0,"failed":0})
                try:
                    plain = open_envelope(envelope, version, bytes(aad))
                except Exception:
                    failures["upstream_credentials"] += 1; classifications[category]["failed"] += 1
                    continue
                try:
                    counts["upstream_credentials"] += 1
                    classifications[category]["authenticated"] += 1
                finally:
                    plain[:] = b"\0"*len(plain)
            for provider, identity, envelope, version, enabled, auth_status in db.execute("SELECT provider,identity_digest,credential_ciphertext,credential_key_version,enabled,auth_status FROM grok_accounts"):
                aad = b"cpa-rust-gateway/grok/account-pool/v1\0" + provider.encode() + b"\0" + identity
                category = "native:"+provider+":"+str(bool(enabled))+":"+auth_status
                classifications.setdefault(category,{"authenticated":0,"failed":0})
                try:
                    plain = open_envelope(envelope, version, aad)
                except Exception:
                    failures["native_grok_accounts"] += 1; classifications[category]["failed"] += 1
                    continue
                try:
                    validate_native(provider,plain)
                    counts["native_grok_accounts"] += 1
                    classifications[category]["authenticated"] += 1
                finally:
                    plain[:] = b"\0"*len(plain)
        return {"status": "PASS" if not any(failures.values()) else "FAIL", "method": "actual XChaCha20-Poly1305 AEAD authentication with exact source AAD and retained key", "hchacha_published_vector": "PASS", "authenticated_envelopes": counts, "failed_envelopes":failures,"classifications":classifications,"key_versions": sorted(versions), "plaintext_exported": False, "network_used": False}
    finally:
        key[:] = b"\0"*len(key)
