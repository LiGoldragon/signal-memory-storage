use legacy_name_table::Identifier;
use signal_sema_storage::{
    BelongsRelation, DocumentPayload, FamilyDeclaration, NameTableBytes, NexusActorDeclaration,
    NexusRoute, NexusRuntimeRoot, OpensRelation, RootViolation, SemaStorageRoot,
    SignalContractRoot, StreamDeclaration,
};

#[test]
fn signal_contract_owns_stream_opens_and_belongs_laws() {
    let contract = Identifier::Fixture(0);
    let stream = Identifier::Fixture(1);
    let operation = Identifier::Fixture(2);
    let root = SignalContractRoot {
        contract,
        streams: vec![StreamDeclaration { stream }],
        opens: vec![OpensRelation { operation, stream }],
        belongs: vec![BelongsRelation { stream, contract }],
        names: NameTableBytes(Vec::new()),
    };
    assert_eq!(root.validate(), Ok(()));
}

#[test]
fn nexus_routes_only_between_declared_actors() {
    let actor = Identifier::Fixture(1);
    let missing = Identifier::Fixture(2);
    let root = NexusRuntimeRoot {
        actors: vec![NexusActorDeclaration { actor }],
        routes: vec![NexusRoute {
            sender: actor,
            receiver: missing,
        }],
        names: NameTableBytes(Vec::new()),
    };
    assert_eq!(root.validate(), Err(RootViolation::UnknownRouteActor));
}

#[test]
fn sema_alone_owns_versioned_families() {
    let root = SemaStorageRoot {
        families: vec![FamilyDeclaration {
            family: Identifier::Fixture(1),
            layout_version: 0,
        }],
        names: NameTableBytes(Vec::new()),
    };
    assert_eq!(root.validate(), Err(RootViolation::ZeroLayoutVersion));
}

#[test]
fn names_are_excluded_from_root_identity() {
    let payload = DocumentPayload::SignalContract(SignalContractRoot {
        contract: Identifier::Fixture(0),
        streams: vec![StreamDeclaration {
            stream: Identifier::Fixture(1),
        }],
        opens: Vec::new(),
        belongs: Vec::new(),
        names: NameTableBytes(vec![1, 2, 3]),
    });
    let mut renamed = payload.clone();
    let DocumentPayload::SignalContract(root) = &mut renamed else {
        unreachable!()
    };
    root.names = NameTableBytes(vec![9, 8, 7, 6]);
    assert_eq!(
        payload.content_hash().expect("hash original"),
        renamed.content_hash().expect("hash renamed"),
        "NameTable bytes are projection data, not encoded-form identity"
    );
}

#[test]
fn structural_edits_move_root_identity() {
    let payload = DocumentPayload::SemaStorage(SemaStorageRoot {
        families: vec![FamilyDeclaration {
            family: Identifier::Fixture(1),
            layout_version: 1,
        }],
        names: NameTableBytes(Vec::new()),
    });
    let mut changed = payload.clone();
    let DocumentPayload::SemaStorage(root) = &mut changed else {
        unreachable!()
    };
    root.families[0].layout_version = 2;
    assert_ne!(
        payload.content_hash().expect("hash original"),
        changed.content_hash().expect("hash changed")
    );
}
