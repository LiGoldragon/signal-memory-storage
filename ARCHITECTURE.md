# Architecture

## Current implementation: wired legacy topology

This pure contract crate names typed storage, lookup, version, snapshot, push-subscription, fixture-scoped identifier-allocation, and identity-authority messages for a standalone central Memory service. It owns no actors, sockets, or durable state itself.

That is the wired legacy topology, not the approved architecture. The approved target has every component daemon stateful and owning its own embedded memory database. A separate small translator daemon — `memory-translator`, with this contract's approved counterpart `signal-memory-translator` — is authoritative only for shared naming and encodedID allocation. It does not become a central store for Ethos, Nomos, or Logos documents. `memory-translator` and `signal-memory-translator` are distinct repositories; `signal-memory-storage` carries this storage contract.

The current Logos archive train is one deliberate exception to that freeze. The live `DocumentPayload::Logos` arm carries the canonical validated archive of one complete `WholeLogos` encoded form plus the canonical validated archive of its production nested vocabulary table. It does not expose the superseded per-item `EncodedItem` carrier or a flat name table. Both archives cross their owners' checked reconstruction boundaries before the payload is accepted or hashed; only the `WholeLogos` bytes participate in document content identity, because names are projection data. This replacement changes the closed payload enum and therefore advances `DocumentPayloadDomain` to layout 3. It updates the donor's archive truth without reviving the central topology or choosing any later identity-migration scheme.

The older `TypeSchema` arm remains byte-for-byte representable as an opaque archive wrapper for forensic compatibility. This crate no longer depends on a superseded schema owner and deliberately offers no constructor or decoder that would pretend those bytes are a current validated schema.

## Existing identity-authority operation: mechanism donor

The central service this crate names is the one logical allocation authority per deployment under the now-dead 2026-07-17 "seat it centrally in memory" claim; that claim was later overruled. Its mandate was the keystone's two laws — never re-mint an identity for the same declared thing, never rebind an existing identity to a different thing. This crate names the bind-or-mint operation that enforced them:

- `Request::BindIdentities { whole: SchemaWholeHandle, declarations: Vec<DeclaredIdentity> }` — mint or bind the schema-whole's universe, then per declaration mint a fresh identity (new thing) or return the existing one (same thing).
- `Reply::IdentitiesBound(BoundIdentities)` — the whole's `MintedUniverse` and one `IdentityAssignment` per declaration (its `TypeIdentity` plus a `BindOutcome` of `Minted`/`Bound`).
- Each `DeclaredIdentity` carries a `DeclaredKey` ("the same declared thing"), a `DeclaredShape` fingerprint, and an `IdentityIntent`: `MintOrBind` (idempotent by key; a shape change under the same key is a §3 version-advance keeping the id) or `Continue(TypeIdentity)` (a rename/move asserting an existing identity — the §4 declared bind-existing marker).
- Law-2 rejection `Rejection::IdentityRebindRejected { identity, bound_shape, attempted_shape }`; plus `EmptyDeclarationSet` and `IdentityNeverMinted`.

All additions are append-only on the `Request`, `Reply`, and `Rejection` enums (new variants appended after the existing ones), so the rkyv wire encoding stays backward-compatible; `tests/round_trip.rs` covers the new request, reply, and rejection.

This flat identity ontology — one `SchemaWholeHandle` universe with a flat `DeclaredKey`-to-`TypeIdentity` map beneath it — is superseded and must not guide the `signal-memory-translator` contract. The approved naming model is variant-fronted, nested module-owned nametables: each module owns the table of its own members' encodedIDs, and the module itself is an entry in its containing module's table; a thing's durable identity is the full chain of module-local encodedIDs, not a flat handle-plus-key pair.

## Revisable leans

- The prototype stores all accepted document roots in one closed `DocumentPayload`; revise when the document-kind design review settles richer roots.
- No durable central-store deployment was found in the present CriomOS/CriomOS-home configuration audit. That is migration evidence for this archive-layout crossing, not a claim that no external archive can exist; any discovered archive must be handled by an explicit migration rather than decoded under layout 3 by guesswork.
- One fixture scope is explicit on every key and allocation request. No split/merge lineage is inferred.
- **Identity-authority contract leans.** `SchemaWholeHandle` is an opaque author-supplied byte handle (`whole-handle-opaque`, v2 L1): the whole carries no independently minted-and-returned constitutive id. `DeclaredKey` keys "the same declared thing" on the declared name spelling within its whole (`declared-key-is-name`, v2 L4). Revise both if an authoring surface carries explicit whole/declaration identity markers, or the psyche rules whole-identity is independently minted. The `AllocateIdentifiers`/`IdentifierBlock` fixture-scope allocation is retained as a marked compatibility path alongside the authoritative `BindIdentities` mint.
- `signal-memory` remains the universal operation-class vocabulary; this separate crate is the component contract because `signal-memory` explicitly owns no component payloads.
- **Short-header observability (legacy signal-frame body).** This donor carries the current shared handshake, exchange, request, and reply body types, but preserves its historical u32-length-prefixed frame with an all-zero eight-byte short header. Current `signal-frame` correctly refuses to mint an unbound `ShortHeader`, so this crate encodes and checks that legacy prefix locally rather than inventing a contract allocation. The lean holds while no tap consumer reads these central-store frames. If the donor ever re-enters production routing, allocate its binding through the canonical registry before using the bound frame API.
