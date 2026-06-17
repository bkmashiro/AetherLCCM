use lccm_protocol::{
    api::{
        accept_remote_claim, create_settlement_claim, lock_for_export, provisionally_credit,
        verify_settlement_claim, AcceptRemoteClaimRequest, LockForExportRequest,
        ProvisionallyCreditRequest, VerifySettlementClaimRequest,
    },
    client::{ClientConfig, UniverseClient},
    types::{
        CreditLine, RiskLabel, Region3D, SettlementPolicy, SpacetimeCoord, TimeInterval,
        TxKind, Transaction,
    },
};

fn fixture_client() -> (UniverseClient, SpacetimeCoord) {
    let coord = SpacetimeCoord {
        frame_id: "gcrs:earth".to_string(),
        time_interval: TimeInterval { t_min: 0, t_max: 10 },
        position_region: Region3D {
            center: [0.0, 0.0, 0.0],
            radius_ly: 0.0,
        },
        uncertainty: 0.0,
        attestation: Vec::new(),
    };
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
    client.credit_lines.insert(
        "line-1".to_string(),
        CreditLine {
            credit_line_id: "line-1".to_string(),
            from_domain: "earth".to_string(),
            to_domain: "mars".to_string(),
            limit: 1000,
            used: 0,
            haircut: 0.0,
            risk_label: RiskLabel::Medium,
        },
    );
    client.sync_state.get_mut("earth").unwrap().frontier.push("checkpoint-1".to_string());
    (client, coord)
}

fn append_local_tx(client: &mut UniverseClient, event_suffix: u8) -> lccm_protocol::types::Event {
    let tx = Transaction {
        tx_id: format!("tx-{event_suffix}"),
        domain_id: "earth".to_string(),
        kind: TxKind::Transfer,
        actor_id: "alice".to_string(),
        payload_hash: [event_suffix; 32],
        coord: SpacetimeCoord {
            frame_id: "gcrs:earth".to_string(),
            time_interval: TimeInterval { t_min: 0, t_max: 10 },
            position_region: Region3D {
                center: [0.0, 0.0, 0.0],
                radius_ly: 0.0,
            },
            uncertainty: 0.0,
            attestation: Vec::new(),
        },
        inputs: vec![],
        outputs: vec![],
        causal_dependencies: vec![],
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
fn phase2_lock_and_verify_and_credit() {
    let (mut client, coord) = fixture_client();
    let event = append_local_tx(&mut client, 11);
    let claim = lock_for_export(
        &mut client,
        LockForExportRequest {
            lock_event_id: event.event_id.clone(),
            origin_domain: "earth".to_string(),
            remote_domain: "mars".to_string(),
            asset_id: "XAU".to_string(),
            amount: 10,
            coord: coord.clone(),
            settlement_horizon_years: 2,
        },
    )
    .unwrap();

    let verify_decision = verify_settlement_claim(
        &mut client,
        VerifySettlementClaimRequest {
            claim_id: claim.claim_id.clone(),
            remote_observer_id: "relay-earth-mars".to_string(),
        },
    )
    .unwrap();
    assert!(verify_decision.approved);

    let credit_decision = provisionally_credit(
        &mut client,
        ProvisionallyCreditRequest {
            claim_id: claim.claim_id.clone(),
            credit_line_id: "line-1".to_string(),
        },
    )
    .unwrap();
    assert!(credit_decision.approved);
    assert_eq!(credit_decision.finality, lccm_protocol::FinalityStage::ProvisionallyCredited);
}

#[test]
fn phase2_remote_accept_requires_reconciled_checkpoint_and_no_rollback() {
    let (mut client, coord) = fixture_client();
    let event = append_local_tx(&mut client, 12);
    let claim = lock_for_export(
        &mut client,
        LockForExportRequest {
            lock_event_id: event.event_id.clone(),
            origin_domain: "earth".to_string(),
            remote_domain: "mars".to_string(),
            asset_id: "XAU".to_string(),
            amount: 20,
            coord: coord.clone(),
            settlement_horizon_years: 2,
        },
    )
    .unwrap();

    let missing = accept_remote_claim(
        &mut client,
        AcceptRemoteClaimRequest {
            claim_id: claim.claim_id.clone(),
            remote_checkpoint_id: "missing-remote-cp".to_string(),
        },
    );
    assert!(missing.is_err());

    client
        .sync_state
        .get_mut("earth")
        .unwrap()
        .frontier
        .push("missing-remote-cp".to_string());
    let ok = accept_remote_claim(
        &mut client,
        AcceptRemoteClaimRequest {
            claim_id: claim.claim_id.clone(),
            remote_checkpoint_id: "missing-remote-cp".to_string(),
        },
    )
    .unwrap();
    assert!(ok.approved);

    let rollback = client.update_claim_status(
        &claim.claim_id,
        lccm_protocol::FinalityStage::RemoteObserved,
        "rollback".to_string(),
        RiskLabel::Medium,
    );
    assert!(rollback.is_err());
}

#[test]
fn phase2_create_claim_refuses_missing_lock_event() {
    let (mut client, _) = fixture_client();
    let result = create_settlement_claim(
        &mut client,
        "phase2-missing-event".to_string(),
        "does-not-exist".to_string(),
        "earth".to_string(),
        "mars".to_string(),
        "XAU".to_string(),
        20,
    );
    assert!(result.is_err());
}
