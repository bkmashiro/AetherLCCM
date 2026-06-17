use lccm_protocol::{
    api::{
        create_settlement_claim, detect_conflict, DetectConflictRequest,
    },
    causal::{validate_claim_lightcone, validate_message_route_with_policy},
    client::{ClientConfig, UniverseClient},
    settlement::mark_bilaterally_settled,
    types::{
        Event, Message, Region3D, SettlementPolicy, SpacetimeCoord, TimeInterval, TxKind, Transaction,
    },
};

fn valid_coord(frame: &str) -> SpacetimeCoord {
    SpacetimeCoord {
        frame_id: frame.to_string(),
        time_interval: TimeInterval {
            t_min: 0,
            t_max: 10,
        },
        position_region: Region3D {
            center: [0.0, 0.0, 0.0],
            radius_ly: 0.0,
        },
        uncertainty: 0.0,
        attestation: Vec::new(),
    }
}

fn phase3_fixture_client() -> (UniverseClient, SpacetimeCoord) {
    let coord = valid_coord("gcrs:earth");
    let mut client = UniverseClient::new(
        ClientConfig {
            local_domain: "earth".to_string(),
            challenge_window: 100,
            checkpoint_grace_period: 0,
            settlement_policy: SettlementPolicy::default(),
        },
        coord.clone(),
    );
    client.add_domain("earth".to_string()).unwrap();
    client.add_domain("mars".to_string()).unwrap();
    (client, coord)
}

fn append_local_tx(client: &mut UniverseClient, event_suffix: u8, dependencies: Vec<String>) -> Event {
    let tx = Transaction {
        tx_id: format!("tx-{event_suffix}"),
        domain_id: "earth".to_string(),
        kind: TxKind::Transfer,
        actor_id: "alice".to_string(),
        payload_hash: [event_suffix; 32],
        coord: valid_coord("gcrs:earth"),
        inputs: vec![],
        outputs: vec![],
        causal_dependencies: dependencies,
        signatures: vec![],
    };
    client
        .local_domains
        .get_mut("earth")
        .unwrap()
        .submit_local_tx(tx.tx_id.clone(), &tx)
        .unwrap()
}

#[test]
fn phase3_lightcone_rejects_physically_impossible_transfer() {
    let from = SpacetimeCoord {
        frame_id: "earth".to_string(),
        time_interval: TimeInterval {
            t_min: 0,
            t_max: 0,
        },
        position_region: Region3D {
            center: [0.0, 0.0, 0.0],
            radius_ly: 0.0,
        },
        uncertainty: 0.0,
        attestation: Vec::new(),
    };
    let to = SpacetimeCoord {
        frame_id: "earth".to_string(),
        time_interval: TimeInterval {
            t_min: 1,
            t_max: 1,
        },
        position_region: Region3D {
            center: [1e9, 0.0, 0.0],
            radius_ly: 0.0,
        },
        uncertainty: 0.0,
        attestation: Vec::new(),
    };
    let err = validate_claim_lightcone(&from, &to, 0.1);
    assert!(err.is_err());
}

#[test]
fn phase3_missing_dependency_lock_event_is_rejected() -> Result<(), lccm_protocol::ProtocolError> {
    let (mut client, coord) = phase3_fixture_client();
    let _event = append_local_tx(
        &mut client,
        1,
        vec!["missing-dependency".to_string()],
    );
    let invalid = create_settlement_claim(
        &mut client,
        "bad-dependency-claim".to_string(),
        "ev:earth:tx-1".to_string(),
        "earth".to_string(),
        "mars".to_string(),
        "XAU".to_string(),
        10,
    );
    assert!(invalid.is_err());

    let _event_from_submit = append_local_tx(&mut client, 2, Vec::new());
    let bad_route = Message {
        msg_id: "msg-1".to_string(),
        from_event: "ev:earth:tx-1".to_string(),
        to_event: "ev:earth:tx-2".to_string(),
        payload_hash: [0u8; 32],
        route: vec!["a".to_string()],
        send_coord: coord.clone(),
        receive_coord: coord,
        relay_signatures: vec![],
        anti_replay_nonce: "short".to_string(),
    };
    let err = validate_message_route_with_policy(&bad_route, 2, 8);
    assert!(err.is_err());
    Ok(())
}

#[test]
fn phase3_detect_conflict_and_prevent_no_skip_settlement() -> Result<(), lccm_protocol::ProtocolError> {
    let (mut client, _) = phase3_fixture_client();
    let event = append_local_tx(&mut client, 3, Vec::new());

    let claim = create_settlement_claim(
        &mut client,
        "conflict-claim-a".to_string(),
        event.event_id,
        "earth".to_string(),
        "mars".to_string(),
        "XAU".to_string(),
        20,
    )?;

    let mut conflict_claim = claim.clone();
    conflict_claim.claim_id = "conflict-claim-b".to_string();
    client.pending_claims.insert(
        conflict_claim.claim_id.clone(),
        conflict_claim.clone(),
    );
    let conflict = detect_conflict(
        &client,
        DetectConflictRequest {
            claim_a: claim.claim_id.clone(),
            claim_b: conflict_claim.claim_id.clone(),
        },
    )?;
    assert!(conflict.detected);
    assert_eq!(conflict.evidence.len(), 2);

    let result = mark_bilaterally_settled(&mut client, &claim.claim_id);
    assert!(result.is_err());
    Ok(())
}
