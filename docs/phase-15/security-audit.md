# Aether OS 0.2.0 — Security Audit

Last reviewed 2026-08-30. This document is the production security
checklist for Aether OS 0.2.0. Every line is a property Aether either
holds today, or names the phase that closes it.

## 1. Cryptographic primitives

| Primitive       | Use                                            | Status   | Source                                  |
| --------------- | ---------------------------------------------- | -------- | --------------------------------------- |
| SHA-256         | audit chain links, device fingerprint          | OK       | `sha2 0.10`                             |
| AES-256-GCM     | sealed credential store                        | OK       | `aes-gcm 0.10`                          |
| Ed25519         | manifest signing, signed updates               | OK       | `ed25519-dalek 2 / 3`                   |
| Hybrid AEAD     | cross-device envelopes (future)                | Planned  | Phase 15.x (cross-device runtime)       |
| Argon2id        | passphrase → key (future)                      | Planned  | Phase 15.x (recovery)                   |

## 2. Key handling

- `SealedStore` keeps the sealing key in a `KeyProvider`. The default
  `StaticKeyProvider` holds the key in a `[u8; 32]` buffer; the
  `RandomKeyProvider` generates a fresh key at startup.
- Plaintext returned by `unseal` is wrapped in `Secret<String>` which
  zeroizes on drop.
- The local copy of the key inside `seal` is zeroized before
  constructing the cipher; the key never escapes the closure in a
  logged form.
- The fingerprint is derived from a public key with SHA-256; the
  private key is never present in the device core.

## 3. Capability / permission policy

- Every IPC command is evaluated by `DefaultPermissionPolicy` against
  the request's `actor_trust` and the service's declared
  `PermissionProfile`. Verdicts:
  - `Allow` → request proceeds.
  - `Deny` → `POLICY_DENIED`.
  - `RequireConsent` → `REQUIRES_CONFIRMATION` (caller re-issues
    through the consent flow).
- The decision is recorded in the audit chain with the same
  `prev_hash` / `content_hash` discipline as every other entry.

## 4. Audit chain integrity

- `prev_hash` is the SHA-256 of the previous entry's canonical
  serialization, or `GENESIS_PREV_HASH` for `index == 0`.
- `content_hash` is the SHA-256 of the entry's own canonical
  serialization including `prev_hash`.
- `verify_chain` recomputes both for every entry in order and rejects
  any tampering. The log can be exported and re-verified off-host
  because the canonicalization is field-order-independent from the
  caller's perspective.
- Retention is bounded by `RetentionPolicy::last_n` /
  `RetentionPolicy::bounded` so a runaway process cannot exhaust
  memory.

## 5. IPC transport security

- Loopback-only by default (`AETHER_BIND` defaults to `127.0.0.1`).
- Aether exposes no public surface in 0.2.0; the only network
  service in the initramfs is `udhcpc` for DHCP lease.
- TLS is *not* layered over the control plane in 0.2.0; the binding
  to loopback is the only access control. Cross-host access is
  deferred to the future device runtime.

## 6. Cross-device security (Phase 14.x)

- Pairing is a typed handshake:
  `PairingRequest` ↔ `PairingAcceptance` validated by
  `validate_acceptance`. The user's confirmation of the 6-digit code
  is the trust anchor; code mismatch, fingerprint mismatch, identity
  mismatch, and request expiry are all explicit error variants.
- `accept_remote_delivery` validates the *delivery* in addition to
  the *pairing*: the source's `fingerprint` must match the registry's
  entry, the `seq` must be strictly greater than the last seen
  `seq` from that peer, the timestamp must be inside the configured
  skew window, and the per-peer grant must cover the operation
  (`receive_observations` or `receive_proposals`).
- `execute_remote_tasks` is off by default in `PairingGrant::default`;
  the local user has to explicitly opt in.
- Revocation is immediate: `device.revoke` flips the state to
  `Revoked` and the future runtime refuses further deliveries.

## 7. Supply chain

- All dependencies are tracked in `Cargo.lock`, committed to the
  repository.
- `unsafe_code = "forbid"` is enforced at the workspace level.
- `clippy::unwrap_used` and `clippy::expect_used` are denied in
  production code; tests opt in with module-level `#[allow]`
  attributes.
- `clippy::all = "deny"` is enforced at the workspace level.
- CI runs clippy with `-D warnings` on every push.

## 8. Update mechanism

- Updates carry a signed `SignedManifest` envelope (Ed25519). The
  update core refuses to apply a manifest whose signature does not
  match an entry in the local `TrustStore`.
- `VersionPolicy` rejects silent downgrades by default.
- Recovery from a failed transition is supported via
  `aether-update-core::recovery` (the "rollback" entry point).

## 9. Known limitations

- No public-key pinning on the IPC loopback port (the binding is the
  only access control). Production deployments should firewall
  `AETHER_CONTROL_PORT` at the network boundary.
- No rate limiting on the control plane. A misbehaving local client
  can saturate the dispatcher; the audit chain's `RetentionPolicy`
  is the only back-pressure.
- No confidential-computing integration (TEE / SGX / SEV) in 0.2.0.
  The hooks are present in `aether-security` but the runtime is not
  yet built.

## 10. Audit sign-off

The 0.2.0 audit was performed by the development team in advance of
the release tag. Every item above is either `OK`, `Planned`, or
documented as a follow-on. The next scheduled review is at the 0.3.0
milestone; the same template will be used.
