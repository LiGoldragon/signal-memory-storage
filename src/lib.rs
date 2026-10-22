//! Typed binary contract for the prototype's central, standalone Memory service.
use std::collections::BTreeSet;

use content_identity::{DomainSeparation, HashDomain, IdentityHasher, LayoutVersion};
use core_logos::WholeLogos;
use legacy_name_table::Identifier;
use rkyv::{Archive, Deserialize, Serialize};
use signal_frame::{
    ExchangeFrameBody, ExchangeIdentifier, ExchangeLane, HandshakeRejectionReason, HandshakeReply,
    HandshakeRequest, LaneSequence, NonEmpty, ProtocolVersion, Reply as FrameReply, SessionEpoch,
    SubReply,
};

pub struct DocumentPayloadDomain;
impl HashDomain for DocumentPayloadDomain {
    fn separation() -> DomainSeparation {
        DomainSeparation::Contextual {
            context: "language-engine/document-payload",
            // Layout 3 replaces the legacy per-item Logos arm with one validated
            // WholeLogos archive and its matching nested vocabulary-name archive.
            // The closed payload enum and its canonical Logos identity preimage both
            // change, so this is an explicit identity-layout crossing.
            layout: LayoutVersion::new(3),
        }
    }
}

#[derive(
    Archive, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub struct FixtureScope(pub u64);
#[derive(
    Archive, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub struct SlotIdentifier(pub u64);
#[derive(
    Archive, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub struct Version(pub u64);
#[derive(
    Archive, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub struct Snapshot(pub u64);
#[derive(
    Archive, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub struct ContentHash(pub [u8; 32]);
#[derive(
    Archive, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub struct SubscriptionIdentifier(pub u64);
#[derive(Archive, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct IdentifierBlock {
    pub first: u32,
    pub length: u32,
}

// ---- Identity authority (design v2, primary-56d1.11) ----
//
// The central Memory service is the one logical allocation authority per deployment
// (settled: "seat it centrally in memory", 2026-07-17). Its whole mandate is the two
// laws of the keystone: never re-mint an identity for the same declared thing, and
// never rebind an existing identity to a different thing. These types name the
// bind-or-mint operation that enforces them.

/// A schema-whole's stable declared handle: the author-controlled identity the
/// authority binds a minted universe to. Two ingestions that present the SAME handle
/// bind the SAME universe (the keystone's anti-remint at the whole level); what the
/// handle keys on is the caller's concern — the authority treats it as opaque bytes.
///
/// LEAN `whole-handle-opaque` (realizes v2 L1): schema-whole identity is an
/// author-supplied opaque byte handle rather than an independently minted-and-returned
/// constitutive id. Revision trigger: the psyche ruling the whole carries an
/// independently minted identity, or that it derives from a designated root type.
#[derive(Archive, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SchemaWholeHandle(pub Vec<u8>);

/// A minted universe identity: a globally-unique, never-reused compact u32 the
/// authority allocates per schema-whole, the authoritative universe-id path replacing
/// the `FIXTURE_UNIVERSE(0)` placeholder (v2 L5). Maps directly onto core-schema's
/// `EncodedUniverseId`.
#[derive(
    Archive, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub struct MintedUniverse(pub u32);

/// The declared key of one thing within a schema-whole — "the same declared thing."
/// At this layer it is the declared name spelling: two ingestions of one schema name
/// their types identically, so keying identity on the name binds parse-order-
/// independent identities (the keystone anti-remint made operational).
///
/// LEAN `declared-key-is-name` (realizes v2 L4 at the authority layer): "the same
/// declared thing" keys on the declared name spelling within its whole. Revision
/// trigger: an authoring surface carrying an explicit per-declaration bind/mint marker
/// distinct from the name.
#[derive(Archive, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeclaredKey(pub Vec<u8>);

/// A fingerprint of a declared thing's structure, recorded alongside its identity so
/// the authority can tell a version-advance of the same thing (§3, same id) from a
/// genuinely different thing claiming an existing id (law 2).
#[derive(
    Archive, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub struct DeclaredShape(pub [u8; 32]);

/// The assigned local identity of one declared thing within its universe — the
/// `local` half of core-schema's `ScopedEncodedTypeId`. Compact and never reused within a
/// universe.
#[derive(
    Archive, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub struct TypeIdentity(pub u32);

/// Whether a declared thing mints a fresh identity / binds its existing one by key, or
/// asserts it continues a specific already-issued identity (a rename or a "move" into
/// this whole — the §4 declared bind-existing marker). Declared, never content-inferred;
/// the authority enforces the two laws against it. A closed typed record, never a flag.
#[derive(Archive, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdentityIntent {
    /// Mint a fresh identity for a newly declared thing, or return the identity already
    /// bound to this key (idempotent by key — law 1's anti-remint). A re-presented key
    /// whose shape has advanced is a version edit within the one identity (§3): the id
    /// is unchanged and the recorded shape advances.
    MintOrBind,
    /// Assert this declared thing IS an already-issued identity (a continuation — a
    /// rename or move into this home). The authority enforces law 2: the claimed
    /// identity must already name a thing whose recorded shape equals this declaration's
    /// shape; pointing it at a structurally different thing is rejected.
    Continue(TypeIdentity),
}

/// One declared thing in a bind-or-mint request: its declared key, its structural
/// fingerprint, and its declared intent.
#[derive(Archive, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct DeclaredIdentity {
    pub key: DeclaredKey,
    pub shape: DeclaredShape,
    pub intent: IdentityIntent,
}

/// Whether the authority minted a fresh identity or bound an existing one — carried
/// back for observability; the identity is authoritative either way.
#[derive(Archive, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum BindOutcome {
    Minted,
    Bound,
}

/// The authority's assignment for one declared thing: its key, the identity it now
/// holds, and whether that identity was freshly minted or bound to an existing one.
#[derive(Archive, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct IdentityAssignment {
    pub key: DeclaredKey,
    pub identity: TypeIdentity,
    pub outcome: BindOutcome,
}

/// The authority's reply to a bind-or-mint: the schema-whole's minted universe and one
/// assignment per requested declared thing, in request order.
#[derive(Archive, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct BoundIdentities {
    pub universe: MintedUniverse,
    pub assignments: Vec<IdentityAssignment>,
}

#[derive(
    Archive, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub enum DocumentKind {
    TypeSchema,
    SignalContract,
    NexusRuntime,
    MemoryStorage,
    Nomos,
    Logos,
}

#[derive(Archive, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct DocumentKey {
    pub scope: FixtureScope,
    pub kind: DocumentKind,
    pub slot: SlotIdentifier,
}

#[derive(Archive, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct NameTableBytes(pub Vec<u8>);

/// Opaque bytes from the superseded core-schema archive train.
///
/// The storage contract preserves this closed payload arm for forensic reads, but
/// no longer claims to construct or interpret it through a current schema owner.
#[derive(Archive, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct EncodedSchemaArchive(Vec<u8>);

impl EncodedSchemaArchive {
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// Canonical, validated archive of one complete Logos encoded form.
#[derive(Archive, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct WholeLogosArchive(Vec<u8>);

impl WholeLogosArchive {
    pub fn from_logos(logos: &WholeLogos) -> Result<Self, CodecError> {
        let bytes = logos
            .to_archive_bytes()
            .map_err(CodecError::WholeLogosArchive)?;
        Self::try_from_archive_bytes(bytes)
    }

    pub fn try_from_archive_bytes(bytes: Vec<u8>) -> Result<Self, CodecError> {
        WholeLogos::from_archive_bytes(&bytes).map_err(CodecError::WholeLogosArchive)?;
        Ok(Self(bytes))
    }

    pub fn to_logos(&self) -> Result<WholeLogos, CodecError> {
        WholeLogos::from_archive_bytes(&self.0).map_err(CodecError::WholeLogosArchive)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// Canonical, validated archive of the production nested vocabulary table.
#[derive(Archive, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct VocabularyNameTableArchive(Vec<u8>);

impl VocabularyNameTableArchive {
    pub fn from_name_table(
        table: &signal_sema_translator::VocabularyNameTable,
    ) -> Result<Self, CodecError> {
        let bytes = table
            .to_archive_bytes()
            .map_err(|error| CodecError::VocabularyNameTableArchive(error.to_string()))?;
        Self::try_from_archive_bytes(bytes.as_ref().to_vec())
    }

    pub fn try_from_archive_bytes(bytes: Vec<u8>) -> Result<Self, CodecError> {
        signal_sema_translator::VocabularyNameTable::from_archive_bytes(&bytes)
            .map_err(|error| CodecError::VocabularyNameTableArchive(error.to_string()))?;
        Ok(Self(bytes))
    }

    pub fn to_name_table(&self) -> Result<signal_sema_translator::VocabularyNameTable, CodecError> {
        signal_sema_translator::VocabularyNameTable::from_archive_bytes(&self.0)
            .map_err(|error| CodecError::VocabularyNameTableArchive(error.to_string()))
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

#[derive(Archive, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct StreamDeclaration {
    pub stream: Identifier,
}
#[derive(Archive, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct OpensRelation {
    pub operation: Identifier,
    pub stream: Identifier,
}
#[derive(Archive, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct BelongsRelation {
    pub stream: Identifier,
    pub contract: Identifier,
}
#[derive(Archive, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SignalContractRoot {
    pub contract: Identifier,
    pub streams: Vec<StreamDeclaration>,
    pub opens: Vec<OpensRelation>,
    pub belongs: Vec<BelongsRelation>,
    pub names: NameTableBytes,
}

impl SignalContractRoot {
    pub fn validate(&self) -> Result<(), RootViolation> {
        let streams: BTreeSet<_> = self.streams.iter().map(|entry| entry.stream).collect();
        if streams.len() != self.streams.len() {
            return Err(RootViolation::DuplicateStream);
        }
        if self
            .opens
            .iter()
            .any(|relation| !streams.contains(&relation.stream))
        {
            return Err(RootViolation::UnknownOpenedStream);
        }
        if self.belongs.iter().any(|relation| {
            !streams.contains(&relation.stream) || relation.contract != self.contract
        }) {
            return Err(RootViolation::InvalidBelongsRelation);
        }
        Ok(())
    }
}

#[derive(Archive, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct NexusActorDeclaration {
    pub actor: Identifier,
}
#[derive(Archive, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct NexusRoute {
    pub sender: Identifier,
    pub receiver: Identifier,
}
#[derive(Archive, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct NexusRuntimeRoot {
    pub actors: Vec<NexusActorDeclaration>,
    pub routes: Vec<NexusRoute>,
    pub names: NameTableBytes,
}
impl NexusRuntimeRoot {
    pub fn validate(&self) -> Result<(), RootViolation> {
        let actors: BTreeSet<_> = self.actors.iter().map(|entry| entry.actor).collect();
        if actors.len() != self.actors.len() {
            return Err(RootViolation::DuplicateActor);
        }
        if self
            .routes
            .iter()
            .any(|route| !actors.contains(&route.sender) || !actors.contains(&route.receiver))
        {
            return Err(RootViolation::UnknownRouteActor);
        }
        Ok(())
    }
}

#[derive(Archive, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct FamilyDeclaration {
    pub family: Identifier,
    pub layout_version: u32,
}
#[derive(Archive, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct MemoryStorageRoot {
    pub families: Vec<FamilyDeclaration>,
    pub names: NameTableBytes,
}
impl MemoryStorageRoot {
    pub fn validate(&self) -> Result<(), RootViolation> {
        let families: BTreeSet<_> = self.families.iter().map(|entry| entry.family).collect();
        if families.len() != self.families.len() {
            return Err(RootViolation::DuplicateFamily);
        }
        if self
            .families
            .iter()
            .any(|family| family.layout_version == 0)
        {
            return Err(RootViolation::ZeroLayoutVersion);
        }
        Ok(())
    }
}

#[derive(Archive, Serialize)]
struct SignalContractIdentity {
    contract: Identifier,
    streams: Vec<StreamDeclaration>,
    opens: Vec<OpensRelation>,
    belongs: Vec<BelongsRelation>,
}
impl From<&SignalContractRoot> for SignalContractIdentity {
    fn from(root: &SignalContractRoot) -> Self {
        Self {
            contract: root.contract,
            streams: root.streams.clone(),
            opens: root.opens.clone(),
            belongs: root.belongs.clone(),
        }
    }
}

#[derive(Archive, Serialize)]
struct NexusRuntimeIdentity {
    actors: Vec<NexusActorDeclaration>,
    routes: Vec<NexusRoute>,
}
impl From<&NexusRuntimeRoot> for NexusRuntimeIdentity {
    fn from(root: &NexusRuntimeRoot) -> Self {
        Self {
            actors: root.actors.clone(),
            routes: root.routes.clone(),
        }
    }
}

#[derive(Archive, Serialize)]
struct MemoryStorageIdentity {
    families: Vec<FamilyDeclaration>,
}
impl From<&MemoryStorageRoot> for MemoryStorageIdentity {
    fn from(root: &MemoryStorageRoot) -> Self {
        Self {
            families: root.families.clone(),
        }
    }
}

#[derive(Archive, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum RootViolation {
    DuplicateStream,
    UnknownOpenedStream,
    InvalidBelongsRelation,
    DuplicateActor,
    UnknownRouteActor,
    DuplicateFamily,
    ZeroLayoutVersion,
    InvalidWholeLogosArchive,
    InvalidVocabularyNameTableArchive,
}

/// The prototype has one real, fixture-scoped macro package. This stable typed
/// identity reconstructs `MacroPackage::wire_fixture()` in the Nomos daemon.
#[derive(Archive, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum NomosPackage {
    WireFixture,
}

#[derive(Archive, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum DocumentPayload {
    TypeSchema {
        schema: EncodedSchemaArchive,
        names: NameTableBytes,
    },
    SignalContract(SignalContractRoot),
    NexusRuntime(NexusRuntimeRoot),
    MemoryStorage(MemoryStorageRoot),
    Nomos(NomosPackage),
    Logos {
        logos: WholeLogosArchive,
        names: VocabularyNameTableArchive,
    },
}

impl DocumentPayload {
    pub fn kind(&self) -> DocumentKind {
        match self {
            Self::TypeSchema { .. } => DocumentKind::TypeSchema,
            Self::SignalContract(_) => DocumentKind::SignalContract,
            Self::NexusRuntime(_) => DocumentKind::NexusRuntime,
            Self::MemoryStorage(_) => DocumentKind::MemoryStorage,
            Self::Nomos(_) => DocumentKind::Nomos,
            Self::Logos { .. } => DocumentKind::Logos,
        }
    }

    pub fn validate(&self) -> Result<(), RootViolation> {
        match self {
            Self::SignalContract(root) => root.validate(),
            Self::NexusRuntime(root) => root.validate(),
            Self::MemoryStorage(root) => root.validate(),
            Self::Logos { logos, names } => {
                logos
                    .to_logos()
                    .map_err(|_| RootViolation::InvalidWholeLogosArchive)?;
                names
                    .to_name_table()
                    .map_err(|_| RootViolation::InvalidVocabularyNameTableArchive)?;
                Ok(())
            }
            Self::TypeSchema { .. } | Self::Nomos(_) => Ok(()),
        }
    }

    pub fn content_hash(&self) -> Result<ContentHash, CodecError> {
        let mut hasher = DocumentPayloadDomain::separation().begin();
        match self {
            Self::TypeSchema { schema, .. } => {
                hasher.update_raw(&[0]);
                Self::hash_archived(&mut hasher, schema)?;
            }
            Self::SignalContract(root) => {
                hasher.update_raw(&[1]);
                Self::hash_archived(&mut hasher, &SignalContractIdentity::from(root))?;
            }
            Self::NexusRuntime(root) => {
                hasher.update_raw(&[2]);
                Self::hash_archived(&mut hasher, &NexusRuntimeIdentity::from(root))?;
            }
            Self::MemoryStorage(root) => {
                hasher.update_raw(&[3]);
                Self::hash_archived(&mut hasher, &MemoryStorageIdentity::from(root))?;
            }
            Self::Nomos(package) => {
                hasher.update_raw(&[4]);
                Self::hash_archived(&mut hasher, package)?;
            }
            Self::Logos { logos, names } => {
                hasher.update_raw(&[5]);
                logos.to_logos()?;
                names.to_name_table()?;
                hasher.update_length_prefixed(logos.as_bytes());
            }
        }
        Ok(ContentHash(hasher.finalize_bytes()))
    }

    fn hash_archived<Value>(hasher: &mut IdentityHasher, value: &Value) -> Result<(), CodecError>
    where
        Value: for<'archive> Serialize<
            rkyv::api::high::HighSerializer<
                rkyv::util::AlignedVec,
                rkyv::ser::allocator::ArenaHandle<'archive>,
                rkyv::rancor::Error,
            >,
        >,
    {
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(value)
            .map_err(|error| CodecError::Encode(error.to_string()))?;
        hasher.update_length_prefixed(&bytes);
        Ok(())
    }
}

#[derive(Archive, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct StoredDocument {
    pub key: DocumentKey,
    pub version: Version,
    pub hash: ContentHash,
    pub payload: DocumentPayload,
}
#[derive(Archive, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SlotSummary {
    pub key: DocumentKey,
    pub version: Version,
    pub hash: ContentHash,
}
#[derive(Archive, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ChangeEvent {
    pub subscription: SubscriptionIdentifier,
    pub snapshot: Snapshot,
    pub document: SlotSummary,
}

#[derive(Archive, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum Request {
    Store {
        key: DocumentKey,
        payload: DocumentPayload,
    },
    Fetch {
        key: DocumentKey,
        version: Option<Version>,
    },
    List {
        scope: FixtureScope,
        kind: Option<DocumentKind>,
    },
    HashFetch {
        hash: ContentHash,
    },
    Snapshot {
        scope: FixtureScope,
    },
    Subscribe {
        scope: FixtureScope,
        kind: Option<DocumentKind>,
    },
    AllocateIdentifiers {
        scope: FixtureScope,
        count: u32,
    },
    /// Bind-or-mint identities for a declared schema-whole against the central
    /// authority: mint (or bind) the whole's universe, then for each declaration return
    /// its existing identity (same thing) or a fresh one (new thing), enforcing the two
    /// laws. Appended after `AllocateIdentifiers` so the wire encoding stays append-only.
    BindIdentities {
        whole: SchemaWholeHandle,
        declarations: Vec<DeclaredIdentity>,
    },
}
#[derive(Archive, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum Reply {
    Stored(SlotSummary),
    Document(Option<StoredDocument>),
    Listed(Vec<SlotSummary>),
    Snapshotted(Snapshot),
    Subscribed {
        identifier: SubscriptionIdentifier,
        initial: Vec<SlotSummary>,
    },
    IdentifiersAllocated(IdentifierBlock),
    /// The authority's bind-or-mint result: the whole's universe and per-declaration
    /// identity assignments. Appended after `IdentifiersAllocated` (append-only wire).
    IdentitiesBound(BoundIdentities),
    Rejected(Rejection),
    Event(ChangeEvent),
}
#[derive(Archive, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum Rejection {
    InvalidKind,
    InvalidDocument(RootViolation),
    NotFound,
    CountZero,
    IncompatibleWireVersion,
    Internal,
    /// A bind-or-mint carried no declarations — the whole-level analogue of
    /// `CountZero`; there is nothing to bind.
    EmptyDeclarationSet,
    /// A `Continue` intent named an identity the authority never issued. An identity
    /// that does not exist cannot be continued.
    IdentityNeverMinted(TypeIdentity),
    /// Law 2 — an existing identity would be rebound to a structurally different
    /// declared thing. A `Continue` intent claimed `identity`, but it already names a
    /// thing of a different shape; rebinding it is a bug, so it is rejected.
    IdentityRebindRejected {
        identity: TypeIdentity,
        bound_shape: DeclaredShape,
        attempted_shape: DeclaredShape,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum CodecError {
    #[error("whole Logos archive: {0}")]
    WholeLogosArchive(#[source] core_logos::WholeLogosArchiveError),
    #[error("vocabulary name-table archive: {0}")]
    VocabularyNameTableArchive(String),
    #[error("encode: {0}")]
    Encode(String),
    #[error("decode: {0}")]
    Decode(String),
    #[error("frame too large")]
    FrameTooLarge,
}

pub struct Wire;
impl Wire {
    pub fn encode_request(value: &Request) -> Result<Vec<u8>, CodecError> {
        rkyv::to_bytes::<rkyv::rancor::Error>(value)
            .map(|bytes| bytes.to_vec())
            .map_err(|error| CodecError::Encode(error.to_string()))
    }
    pub fn encode_reply(value: &Reply) -> Result<Vec<u8>, CodecError> {
        rkyv::to_bytes::<rkyv::rancor::Error>(value)
            .map(|bytes| bytes.to_vec())
            .map_err(|error| CodecError::Encode(error.to_string()))
    }

    pub fn frame_current_handshake_request() -> Result<Vec<u8>, CodecError> {
        Self::frame_handshake_request(signal_frame::SIGNAL_FRAME_PROTOCOL_VERSION)
    }

    pub fn frame_handshake_request(version: ProtocolVersion) -> Result<Vec<u8>, CodecError> {
        Self::encode_legacy_frame_body(&ExchangeFrameBody::<Vec<u8>, Vec<u8>>::HandshakeRequest(
            HandshakeRequest::new(version),
        ))
    }

    pub fn frame_handshake_reply(reply: HandshakeReply) -> Result<Vec<u8>, CodecError> {
        Self::encode_legacy_frame_body(&ExchangeFrameBody::<Vec<u8>, Vec<u8>>::HandshakeReply(
            reply,
        ))
    }

    pub fn frame_request(payload: Vec<u8>, sequence: u64) -> Result<Vec<u8>, CodecError> {
        let exchange = ExchangeIdentifier::new(
            SessionEpoch::new(0),
            ExchangeLane::Connector,
            LaneSequence::new(sequence),
        );
        Self::encode_legacy_frame_body(&ExchangeFrameBody::Request {
            exchange,
            request: signal_frame::Request::from_payload(payload),
        })
    }

    pub fn frame_reply(
        exchange: ExchangeIdentifier,
        payload: Vec<u8>,
    ) -> Result<Vec<u8>, CodecError> {
        Self::encode_legacy_frame_body(&ExchangeFrameBody::Reply {
            exchange,
            reply: FrameReply::committed(NonEmpty::single(SubReply::Ok(payload))),
        })
    }

    pub fn decode_frame(bytes: &[u8]) -> Result<FrameMessage, CodecError> {
        let body = Self::decode_legacy_frame_body(bytes)?;
        match body {
            ExchangeFrameBody::HandshakeRequest(request) => {
                Ok(FrameMessage::HandshakeRequest(request.version()))
            }
            ExchangeFrameBody::HandshakeReply(reply) => Ok(FrameMessage::HandshakeReply(reply)),
            ExchangeFrameBody::Request { exchange, request } => Ok(FrameMessage::Request {
                exchange,
                payload: request.payloads().head().clone(),
            }),
            ExchangeFrameBody::Reply { exchange, reply } => match reply {
                FrameReply::Accepted { per_operation, .. } => match per_operation.head() {
                    SubReply::Ok(payload) => Ok(FrameMessage::Reply {
                        exchange,
                        payload: payload.clone(),
                    }),
                    _ => Err(CodecError::Decode(
                        "frame operation was not committed".into(),
                    )),
                },
                FrameReply::Rejected { .. } => {
                    Err(CodecError::Decode("frame request was rejected".into()))
                }
            },
        }
    }

    fn encode_legacy_frame_body(
        body: &ExchangeFrameBody<Vec<u8>, Vec<u8>>,
    ) -> Result<Vec<u8>, CodecError> {
        let archive = rkyv::to_bytes::<rkyv::rancor::Error>(body)
            .map_err(|error| CodecError::Encode(error.to_string()))?;
        let payload_length = 8_usize
            .checked_add(archive.len())
            .and_then(|length| u32::try_from(length).ok())
            .ok_or(CodecError::FrameTooLarge)?;
        let mut framed = Vec::with_capacity(4 + payload_length as usize);
        framed.extend_from_slice(&payload_length.to_be_bytes());
        framed.extend_from_slice(&[0; 8]);
        framed.extend_from_slice(&archive);
        Ok(framed)
    }

    fn decode_legacy_frame_body(
        bytes: &[u8],
    ) -> Result<ExchangeFrameBody<Vec<u8>, Vec<u8>>, CodecError> {
        let length_bytes: [u8; 4] = bytes
            .get(..4)
            .and_then(|prefix| prefix.try_into().ok())
            .ok_or_else(|| CodecError::Decode("frame has no complete length prefix".into()))?;
        let expected = u32::from_be_bytes(length_bytes) as usize;
        let payload = &bytes[4..];
        if payload.len() != expected {
            return Err(CodecError::Decode(format!(
                "frame length is {}, expected {expected}",
                payload.len()
            )));
        }
        let (short_header, archive) = payload
            .split_at_checked(8)
            .ok_or_else(|| CodecError::Decode("frame has no complete short header".into()))?;
        if short_header != [0; 8] {
            return Err(CodecError::Decode(
                "legacy storage frame has a nonzero short header".into(),
            ));
        }
        rkyv::from_bytes::<ExchangeFrameBody<Vec<u8>, Vec<u8>>, rkyv::rancor::Error>(archive)
            .map_err(|error| CodecError::Decode(error.to_string()))
    }

    pub fn handshake_reply(peer: ProtocolVersion) -> HandshakeReply {
        let local = signal_frame::SIGNAL_FRAME_PROTOCOL_VERSION;
        if local.accepts(peer) {
            HandshakeReply::Accepted(local)
        } else {
            HandshakeReply::Rejected(HandshakeRejectionReason::IncompatibleVersion { local, peer })
        }
    }
}

#[derive(Debug)]
pub enum FrameMessage {
    HandshakeRequest(ProtocolVersion),
    HandshakeReply(HandshakeReply),
    Request {
        exchange: ExchangeIdentifier,
        payload: Vec<u8>,
    },
    Reply {
        exchange: ExchangeIdentifier,
        payload: Vec<u8>,
    },
}
impl FrameMessage {
    pub fn is_accepted_handshake(&self) -> bool {
        matches!(self, Self::HandshakeReply(HandshakeReply::Accepted(_)))
    }
}

#[cfg(test)]
mod tests {
    use super::{DocumentPayload, RootViolation, VocabularyNameTableArchive, WholeLogosArchive};

    #[test]
    fn logos_payload_defensively_refuses_bypassed_invalid_archives() {
        let valid_names = VocabularyNameTableArchive::from_name_table(
            &signal_sema_translator::VocabularyNameTable::new(),
        )
        .expect("archive names");
        let invalid_logos = DocumentPayload::Logos {
            logos: WholeLogosArchive(b"not WholeLogos".to_vec()),
            names: valid_names,
        };
        assert_eq!(
            invalid_logos.validate(),
            Err(RootViolation::InvalidWholeLogosArchive)
        );
        assert!(invalid_logos.content_hash().is_err());

        let valid_logos = WholeLogosArchive::from_logos(&core_logos::WholeLogos::new(Vec::new()))
            .expect("archive WholeLogos");
        let invalid_names = DocumentPayload::Logos {
            logos: valid_logos,
            names: VocabularyNameTableArchive(b"not a name table".to_vec()),
        };
        assert_eq!(
            invalid_names.validate(),
            Err(RootViolation::InvalidVocabularyNameTableArchive)
        );
        assert!(invalid_names.content_hash().is_err());
    }
}
