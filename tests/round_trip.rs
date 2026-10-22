use core_logos::{
    WholeLogos, WholeLogosEnumeration, WholeLogosItem, WholeLogosTupleFields,
    WholeLogosTypeReference, WholeLogosVariant, WholeLogosVariantPayload, WholeLogosVisibility,
};
use encoded_name_table::{
    Declaration, LocalEncodedId, Name, OperationKey, RootDeclaration, SealRequest, TableMutability,
};
use signal_frame::{HandshakeReply, ProtocolVersion, SIGNAL_FRAME_PROTOCOL_VERSION};
use signal_memory_storage::{
    BindOutcome, BoundIdentities, ContentHash, DeclaredIdentity, DeclaredKey, DeclaredShape,
    DocumentKey, DocumentKind, DocumentPayload, FixtureScope, FrameMessage, IdentityAssignment,
    IdentityIntent, MintedUniverse, Rejection, Reply, Request, SchemaWholeHandle, SlotIdentifier,
    TypeIdentity, VocabularyNameTableArchive, WholeLogosArchive, Wire,
};
use signal_sema_translator::{VocabularyEncodedId, VocabularyNameTable, VocabularyRoot};

fn encoded_id(root: VocabularyRoot, chain: &[u16]) -> VocabularyEncodedId {
    VocabularyEncodedId::new(
        root,
        chain.iter().copied().map(LocalEncodedId::new).collect(),
    )
    .expect("nonempty fixture encoded ID")
}

fn whole_logos_fixture() -> WholeLogos {
    let integer = WholeLogosTypeReference::Identity(encoded_id(VocabularyRoot::Rust, &[3]));
    WholeLogos::new(vec![WholeLogosItem::Enumeration(
        WholeLogosEnumeration::new(
            WholeLogosVisibility::Public,
            encoded_id(VocabularyRoot::Universal, &[8, 6]),
            vec![
                WholeLogosVariant::new(
                    encoded_id(VocabularyRoot::Universal, &[8, 6, 1]),
                    WholeLogosVariantPayload::Unit,
                ),
                WholeLogosVariant::new(
                    encoded_id(VocabularyRoot::Universal, &[8, 6, 2]),
                    WholeLogosVariantPayload::Tuple(
                        WholeLogosTupleFields::new(vec![integer.clone()])
                            .expect("unary tuple variant"),
                    ),
                ),
                WholeLogosVariant::new(
                    encoded_id(VocabularyRoot::Universal, &[8, 6, 3]),
                    WholeLogosVariantPayload::Tuple(
                        WholeLogosTupleFields::new(vec![integer.clone(), integer])
                            .expect("product tuple variant"),
                    ),
                ),
            ],
        ),
    )])
}

fn populated_vocabulary_names() -> VocabularyNameTable {
    let mut names = VocabularyNameTable::new();
    names
        .seal(SealRequest::new(
            OperationKey::new([1; 32]),
            vec![RootDeclaration::new(
                VocabularyRoot::Rust,
                TableMutability::Immutable,
                vec![Declaration::Member(Name::new("u64"))],
            )],
            vec![],
        ))
        .expect("seal vocabulary fixture");
    names
}
#[test]
fn request_has_stable_typed_binary_encoding() {
    let request = Request::HashFetch {
        hash: ContentHash([7; 32]),
    };
    let bytes = Wire::encode_request(&request).expect("encode");
    let decoded = rkyv::from_bytes::<Request, rkyv::rancor::Error>(&bytes).expect("decode");
    assert_eq!(decoded, request);
}

#[test]
fn bind_identities_request_round_trips() {
    let request = Request::BindIdentities {
        whole: SchemaWholeHandle(b"payments/v1".to_vec()),
        declarations: vec![
            DeclaredIdentity {
                key: DeclaredKey(b"Alpha".to_vec()),
                shape: DeclaredShape([1; 32]),
                intent: IdentityIntent::MintOrBind,
            },
            DeclaredIdentity {
                key: DeclaredKey(b"Beta".to_vec()),
                shape: DeclaredShape([2; 32]),
                intent: IdentityIntent::Continue(TypeIdentity(4)),
            },
        ],
    };
    let bytes = Wire::encode_request(&request).expect("encode");
    let decoded = rkyv::from_bytes::<Request, rkyv::rancor::Error>(&bytes).expect("decode");
    assert_eq!(decoded, request);
}

#[test]
fn bound_identities_reply_round_trips() {
    let reply = Reply::IdentitiesBound(BoundIdentities {
        universe: MintedUniverse(9),
        assignments: vec![
            IdentityAssignment {
                key: DeclaredKey(b"Alpha".to_vec()),
                identity: TypeIdentity(0),
                outcome: BindOutcome::Minted,
            },
            IdentityAssignment {
                key: DeclaredKey(b"Beta".to_vec()),
                identity: TypeIdentity(1),
                outcome: BindOutcome::Bound,
            },
        ],
    });
    let bytes = Wire::encode_reply(&reply).expect("encode");
    let decoded = rkyv::from_bytes::<Reply, rkyv::rancor::Error>(&bytes).expect("decode");
    assert_eq!(decoded, reply);
}

#[test]
fn whole_logos_document_payload_round_trips_through_both_archive_boundaries() {
    let logos = whole_logos_fixture();
    let logos_archive = WholeLogosArchive::from_logos(&logos).expect("archive WholeLogos");
    let names = VocabularyNameTable::new();
    let names_archive =
        VocabularyNameTableArchive::from_name_table(&names).expect("archive vocabulary names");
    let payload = DocumentPayload::Logos {
        logos: logos_archive,
        names: names_archive,
    };
    assert_eq!(payload.validate(), Ok(()));

    let request = Request::Store {
        key: DocumentKey {
            scope: FixtureScope(1),
            kind: DocumentKind::Logos,
            slot: SlotIdentifier(0),
        },
        payload,
    };
    let bytes = Wire::encode_request(&request).expect("encode");
    let decoded = rkyv::from_bytes::<Request, rkyv::rancor::Error>(&bytes).expect("decode");
    assert_eq!(decoded, request);

    let Request::Store {
        payload: DocumentPayload::Logos { logos, names },
        ..
    } = decoded
    else {
        panic!("expected stored WholeLogos")
    };
    assert_eq!(
        logos.to_logos().expect("restore WholeLogos"),
        whole_logos_fixture()
    );
    assert_eq!(
        names.to_name_table().expect("restore vocabulary names"),
        VocabularyNameTable::new()
    );
}

#[test]
fn corrupt_whole_logos_and_name_archives_are_refused() {
    assert!(WholeLogosArchive::try_from_archive_bytes(b"not WholeLogos".to_vec()).is_err());
    assert!(
        VocabularyNameTableArchive::try_from_archive_bytes(b"not a name table".to_vec()).is_err()
    );
}

#[test]
fn logos_structure_moves_identity_while_vocabulary_names_do_not() {
    let logos = whole_logos_fixture();
    let empty_names = VocabularyNameTable::new();
    let populated_names = populated_vocabulary_names();
    let original = DocumentPayload::Logos {
        logos: WholeLogosArchive::from_logos(&logos).expect("archive WholeLogos"),
        names: VocabularyNameTableArchive::from_name_table(&empty_names)
            .expect("archive empty vocabulary names"),
    };
    let renamed = DocumentPayload::Logos {
        logos: WholeLogosArchive::from_logos(&logos).expect("archive WholeLogos"),
        names: VocabularyNameTableArchive::from_name_table(&populated_names)
            .expect("archive populated vocabulary names"),
    };
    let structurally_changed = DocumentPayload::Logos {
        logos: WholeLogosArchive::from_logos(&WholeLogos::new(Vec::new()))
            .expect("archive changed WholeLogos"),
        names: VocabularyNameTableArchive::from_name_table(&empty_names)
            .expect("archive empty vocabulary names"),
    };

    assert_eq!(
        original.content_hash().expect("hash original"),
        renamed.content_hash().expect("hash renamed"),
        "vocabulary names are projection data, not encoded-form identity"
    );
    assert_ne!(
        original.content_hash().expect("hash original"),
        structurally_changed.content_hash().expect("hash changed")
    );
}

#[test]
fn identity_rebind_rejection_round_trips() {
    let reply = Reply::Rejected(Rejection::IdentityRebindRejected {
        identity: TypeIdentity(3),
        bound_shape: DeclaredShape([7; 32]),
        attempted_shape: DeclaredShape([8; 32]),
    });
    let bytes = Wire::encode_reply(&reply).expect("encode");
    let decoded = rkyv::from_bytes::<Reply, rkyv::rancor::Error>(&bytes).expect("decode");
    assert_eq!(decoded, reply);
}
#[test]
fn fixture_scope_is_explicit() {
    assert_eq!(FixtureScope(1).0, 1);
}

#[test]
fn shared_frame_rejects_unsupported_protocol_versions() {
    let unsupported = ProtocolVersion::new(99, 0, 0);
    assert!(matches!(
        Wire::handshake_reply(unsupported),
        HandshakeReply::Rejected(_)
    ));
    assert!(matches!(
        Wire::handshake_reply(SIGNAL_FRAME_PROTOCOL_VERSION),
        HandshakeReply::Accepted(version) if version == SIGNAL_FRAME_PROTOCOL_VERSION
    ));
}

#[test]
fn request_uses_shared_length_prefixed_exchange_frame() {
    let payload = Wire::encode_request(&Request::HashFetch {
        hash: ContentHash([3; 32]),
    })
    .expect("encode request");
    let frame = Wire::frame_request(payload.clone(), 7).expect("frame request");
    let FrameMessage::Request {
        exchange,
        payload: decoded,
    } = Wire::decode_frame(&frame).expect("decode shared frame")
    else {
        panic!("expected request frame")
    };
    assert_eq!(exchange.sequence().value(), 7);
    assert_eq!(decoded, payload);
}
