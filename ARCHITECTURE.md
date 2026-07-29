# Architecture

## Current implementation: wired legacy topology

This pure contract crate names typed storage, lookup, version, snapshot, push-subscription, fixture-scoped identifier-allocation, and identity-authority messages for a standalone central Sema daemon. It owns no actors, sockets, or durable state itself.

That is the wired legacy topology, not the approved architecture. The approved target has every component daemon stateful and owning its own embedded sema database. A separate small translator daemon — `sema-translator`, with this contract's approved counterpart `signal-sema-translator` — is authoritative only for shared naming and encodedID allocation. It does not become a central store for Ethos, Nomos, or Logos documents. `sema-translator` and `signal-sema-translator` are new repositories, not renames of `sema-storage` or this crate; this crate remains a frozen donor until its separately designed dissolution.

## Existing identity-authority operation: mechanism donor

The central daemon this crate names is the one logical allocation authority per deployment under the now-dead 2026-07-17 "seat it centrally in sema" claim; that claim was later overruled. Its mandate was the keystone's two laws — never re-mint an identity for the same declared thing, never rebind an existing identity to a different thing. This crate names the bind-or-mint operation that enforced them:

- `Request::BindIdentities { whole: SchemaWholeHandle, declarations: Vec<DeclaredIdentity> }` — mint or bind the schema-whole's universe, then per declaration mint a fresh identity (new thing) or return the existing one (same thing).
- `Reply::IdentitiesBound(BoundIdentities)` — the whole's `MintedUniverse` and one `IdentityAssignment` per declaration (its `TypeIdentity` plus a `BindOutcome` of `Minted`/`Bound`).
- Each `DeclaredIdentity` carries a `DeclaredKey` ("the same declared thing"), a `DeclaredShape` fingerprint, and an `IdentityIntent`: `MintOrBind` (idempotent by key; a shape change under the same key is a §3 version-advance keeping the id) or `Continue(TypeIdentity)` (a rename/move asserting an existing identity — the §4 declared bind-existing marker).
- Law-2 rejection `Rejection::IdentityRebindRejected { identity, bound_shape, attempted_shape }`; plus `EmptyDeclarationSet` and `IdentityNeverMinted`.

All additions are append-only on the `Request`, `Reply`, and `Rejection` enums (new variants appended after the existing ones), so the rkyv wire encoding stays backward-compatible; `tests/round_trip.rs` covers the new request, reply, and rejection.

This flat identity ontology — one `SchemaWholeHandle` universe with a flat `DeclaredKey`-to-`TypeIdentity` map beneath it — is superseded and must not guide the `signal-sema-translator` contract. The approved naming model is variant-fronted, nested module-owned nametables: each module owns the table of its own members' encodedIDs, and the module itself is an entry in its containing module's table; a thing's durable identity is the full chain of module-local encodedIDs, not a flat handle-plus-key pair.

## Revisable leans

- The prototype stores all accepted document roots in one closed `DocumentPayload`; revise when the document-kind design review settles richer roots.
- One fixture scope is explicit on every key and allocation request. No split/merge lineage is inferred.
- **Identity-authority contract leans.** `SchemaWholeHandle` is an opaque author-supplied byte handle (`whole-handle-opaque`, v2 L1): the whole carries no independently minted-and-returned constitutive id. `DeclaredKey` keys "the same declared thing" on the declared name spelling within its whole (`declared-key-is-name`, v2 L4). Revise both if an authoring surface carries explicit whole/declaration identity markers, or the psyche rules whole-identity is independently minted. The `AllocateIdentifiers`/`IdentifierBlock` fixture-scope allocation is retained as a marked compatibility path alongside the authoritative `BindIdentities` mint.
- `signal-sema` remains the universal operation-class vocabulary; this separate crate is the component contract because `signal-sema` explicitly owns no component payloads.
- **Short-header observability (signal-frame partial adoption).** Unlike the sibling engine contracts, which hand-roll a u32-length + rkyv frame and bypass `signal-frame` entirely, this contract already carries requests over `signal-frame`'s `ExchangeFrame` with the shared handshake and version negotiation. It frames them with `ShortHeader::empty()`, however, so it never populates the short-header tap-anywhere observability fields. The lean holds while no tap consumer reads them. Revise it when cross-hop tracing or routing needs the short header — populate it at the frame boundary.
