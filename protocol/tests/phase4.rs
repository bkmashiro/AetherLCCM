use lccm_protocol::{
    api::{
        lock_for_export,
        resync_after_offline,
        verify_settlement_claim,
        ResyncAfterOfflineRequest,
        VerifySettlementClaimRequest,
    },
    client::{ClientConfig, UniverseClient},
    crypto::{assert_era_renewal_chain, CryptoEraRegistry},
    types::{
        Region3D, SettlementPolicy, SpacetimeCoord, TimeInterval, TxKind, Transaction,
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

fn phase4_fixture_client() -> (UniverseClient, SpacetimeCoord) {
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

fn append_local_tx(client: &mut UniverseClient, event_suffix: u8, coord: SpacetimeCoord) -> lccm_protocol::types::Event {
    let tx = Transaction {
        tx_id: format!("tx-{event_suffix}"),
        domain_id: "earth".to_string(),
        kind: TxKind::Transfer,
        actor_id: "alice".to_string(),
        payload_hash: [event_suffix; 32],
        coord,
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
fn phase4_stale_checkpoint_cannot_support_provisional_credit() -> lccm_protocol::ProtocolResult<()> {
    let (mut client, coord) = phase4_fixture_client();
    append_local_tx(&mut client, 1, coord.clone());
    let cp = client
        .local_domains
        .get_mut("earth")
        .unwrap()
        .finalize_checkpoint_auto([0u8; 32], coord.clone())?;

    let claim = lock_for_export(
        &mut client,
        lccm_protocol::api::LockForExportRequest {
            lock_event_id: "ev:earth:tx-1".to_string(),
            origin_domain: "earth".to_string(),
            remote_domain: "mars".to_string(),
            asset_id: "XAU".to_string(),
            amount: 5,
            coord,
            settlement_horizon_years: 2,
        },
    )?;
    assert_eq!(claim.lock_checkpoint, cp.hash);

    let mut stale = claim.clone();
    stale.lock_checkpoint = "0000000000000000000000000000000000000000000000000000000000000000".to_string();
    client.pending_claims.insert(stale.claim_id.clone(), stale.clone());

    let decision = verify_settlement_claim(
        &mut client,
        VerifySettlementClaimRequest {
            claim_id: stale.claim_id.clone(),
            remote_observer_id: "relay-earth-mars".to_string(),
        },
    )?;
    assert!(!decision.approved);
    assert!(decision.reason.contains("crypto verification failed"));
    Ok(())
}

#[test]
fn phase4_crypto_era_renewal_chain_checks() {
    let registry = CryptoEraRegistry::default();
    let stable_root = [4u8; 32];
    let expired_parent = [0u8; 32];

    assert!(
        assert_era_renewal_chain(&stable_root, Some(&expired_parent), 10, &registry).is_err()
    );
    assert!(
        assert_era_renewal_chain(&stable_root, Some(&stable_root), 10, &registry).is_ok()
    );
}

#[test]
fn phase4_resync_offline_deterministic_frontier_rebuild() -> lccm_protocol::ProtocolResult<()> {
    let (mut client, _) = phase4_fixture_client();
    let result = resync_after_offline(
        &mut client,
        ResyncAfterOfflineRequest {
            trusted_anchors: vec![
                "earth:12:earth-offline".to_string(),
                "mars:20:mars-offline".to_string(),
                "shared-offline-anchor".to_string(),
                "earth:12:earth-offline".to_string(),
            ],
        },
    )?;
    assert_eq!(result.synced_domains, 2);

    let earth_sync = client.sync_state.get("earth").expect("earth sync state");
    let mars_sync = client.sync_state.get("mars").expect("mars sync state");
    assert!(earth_sync.frontier.contains(&"earth-offline".to_string()));
    assert!(mars_sync.frontier.contains(&"mars-offline".to_string()));
    assert!(earth_sync.frontier.contains(&"shared-offline-anchor".to_string()));
    assert!(mars_sync.frontier.contains(&"shared-offline-anchor".to_string()));
    assert_eq!(earth_sync.sync_watermark, 12);
    assert_eq!(mars_sync.sync_watermark, 20);
    Ok(())
}

#[test]
fn phase4_unknown_domain_anchor_errors() {
    let (mut client, _) = phase4_fixture_client();
    let err = resync_after_offline(
        &mut client,
        ResyncAfterOfflineRequest {
            trusted_anchors: vec!["unknown:12:anchor".to_string()],
        },
    );
    assert!(err.is_err());
}
