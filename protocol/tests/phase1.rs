use lccm_protocol::{
    checkpoint::{CheckpointBundle, CheckpointVerification, QuorumCertificate},
    causal::prove_lightcone,
    client::{ClientConfig, UniverseClient},
    ledger::LocalLedgerDomain,
    types::{
        encode_u64, hash_chunks, hash_hex_to_bytes, hash_to_hex, LightconeStatus, Region3D, SpacetimeCoord,
        SettlementPolicy, TimeInterval, TxKind, Transaction, RiskLabel,
    },
    api,
    errors::ProtocolResult,
};
use lccm_protocol::FinalityStage;

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

#[test]
fn phase1_hash_helpers_roundtrip() -> ProtocolResult<()> {
    let chunk_a = b"event-root".as_slice();
    let chunk_b = b"checkpoint".as_slice();
    let chunk_c = encode_u64(42);
    let hash = hash_chunks(&[chunk_a, chunk_b, chunk_c.as_slice()]);
    let encoded = hash_to_hex(&hash);
    let round = hash_hex_to_bytes(&encoded).expect("hash from hex");
    assert_eq!(hash, round);
    Ok(())
}

#[test]
fn phase1_ledger_checkpoint_monotonicity_and_root() -> ProtocolResult<()> {
    let mut domain = LocalLedgerDomain::new("sol".to_string());
    let coord = valid_coord("gcrs:sol");
    let tx = Transaction {
        tx_id: "tx-1".to_string(),
        domain_id: "sol".to_string(),
        kind: TxKind::Transfer,
        actor_id: "validator-1".to_string(),
        payload_hash: [9u8; 32],
        coord: coord.clone(),
        inputs: vec![],
        outputs: vec![],
        causal_dependencies: vec![],
        signatures: vec![],
    };
    let _evt = domain.submit_local_tx(tx.tx_id.clone(), &tx)?;
    let first = domain.finalize_checkpoint_auto([0u8; 32], coord.clone())?;

    assert_eq!(first.height, 1);
    assert_eq!(domain.chain_height, 1);
    assert_eq!(domain.final_checkpoint.as_ref().unwrap().height, 1);
    assert!(domain.final_checkpoint.as_ref().map(|cp| cp.state_root).is_some());

    assert!(domain
        .finalize_checkpoint_auto([0u8; 32], coord.clone())
        .is_err());

    let tx2 = Transaction {
        tx_id: "tx-2".to_string(),
        domain_id: "sol".to_string(),
        kind: TxKind::Transfer,
        actor_id: "validator-1".to_string(),
        payload_hash: [11u8; 32],
        coord: coord.clone(),
        inputs: vec![],
        outputs: vec![],
        causal_dependencies: vec![],
        signatures: vec![],
    };
    domain.submit_local_tx(tx2.tx_id.clone(), &tx2)?;
    let second = domain.finalize_checkpoint_auto([0u8; 32], coord)?;

    assert!(second.height > first.height);
    assert_eq!(domain.final_checkpoint.as_ref().unwrap().hash, second.hash);
    Ok(())
}

#[test]
fn phase1_lightcone_violation_is_reported() {
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
            center: [1e9, 0.0, 0.0], // impossible for subrelativistic speed in one year
            radius_ly: 0.0,
        },
        uncertainty: 0.0,
        attestation: Vec::new(),
    };
    let proof = prove_lightcone("source", "sink", &[from, to], 0.1)
        .expect("lightcone proof for invalid path");
    assert!(matches!(proof.status, LightconeStatus::Invalid));
    assert!(proof.reason.is_some());
}

#[test]
fn phase1_claim_status_monotonic_guard() -> ProtocolResult<()> {
    let mut client = UniverseClient::new(
        ClientConfig {
            local_domain: "sol".to_string(),
            challenge_window: 100,
            checkpoint_grace_period: 10,
            settlement_policy: SettlementPolicy::default(),
        },
        valid_coord("gcrs:sol"),
    );
    client.add_domain("sol".to_string())?;
    let event = {
        let tx = Transaction {
            tx_id: "ev-lock".to_string(),
            domain_id: "sol".to_string(),
            kind: TxKind::LockForExport,
            actor_id: "alice".to_string(),
            payload_hash: [2u8; 32],
            coord: valid_coord("gcrs:sol"),
            inputs: vec![],
            outputs: vec![],
            causal_dependencies: vec![],
            signatures: vec![],
        };
        client
            .local_domains
            .get_mut("sol")
            .unwrap()
            .submit_local_tx(tx.tx_id.clone(), &tx)?
    };
    let claim = api::create_settlement_claim(
        &mut client,
        "claim-phase1".to_string(),
        event.event_id,
        "sol".to_string(),
        "x1".to_string(),
        "XAU".to_string(),
        1000,
    )?;

    assert_eq!(claim.finality, FinalityStage::RemoteObserved);
    client.update_claim_status(
        &claim.claim_id,
        FinalityStage::ProvisionallyCredited,
        "upgrade".to_string(),
        RiskLabel::Medium,
    )?;
    assert_eq!(client.get_claim_status(&claim.claim_id), Some(&FinalityStage::ProvisionallyCredited));

    let rollback = client.update_claim_status(
        &claim.claim_id,
        FinalityStage::RemoteObserved,
        "rollback attempt".to_string(),
        RiskLabel::Medium,
    );
    assert!(rollback.is_err());

    Ok(())
}

#[test]
fn phase1_remote_checkpoint_missing_causal_fails_verification() -> ProtocolResult<()> {
    let mut domain = LocalLedgerDomain::new("mercury".to_string());
    let coord = valid_coord("gcrs:mercury");
    let tx = Transaction {
        tx_id: "tx-causal".to_string(),
        domain_id: "mercury".to_string(),
        kind: TxKind::Transfer,
        actor_id: "validator-1".to_string(),
        payload_hash: [3u8; 32],
        coord: coord.clone(),
        inputs: vec![],
        outputs: vec![],
        causal_dependencies: vec![],
        signatures: vec![],
    };
    domain.submit_local_tx(tx.tx_id.clone(), &tx)?;
    let cp = domain.finalize_checkpoint_auto([0u8; 32], coord.clone())?;
    let qc = QuorumCertificate {
        signers: vec!["a".into(), "b".into()],
        threshold: 2,
        root: cp.state_commitment,
        signature_bundle: vec![],
    };
    let bundle = CheckpointBundle {
        checkpoint: cp.clone(),
        quorum_certificate: qc,
        causal_certificate: None,
        observation: vec![],
    };

    let mut client = UniverseClient::new(
        ClientConfig {
            local_domain: "mercury".to_string(),
            challenge_window: 0,
            checkpoint_grace_period: 0,
            settlement_policy: SettlementPolicy::default(),
        },
        coord,
    );
    client.add_domain("mercury".to_string())?;
    client.local_roots.insert("mercury".to_string(), cp.validator_set_root);

    let verification: CheckpointVerification = lccm_protocol::settlement::verify_remote_checkpoint(&client, &bundle)?;
    assert!(!verification.valid);
    assert!(verification
        .reasons
        .iter()
        .any(|s| s.contains("missing causal certificate")));
    Ok(())
}
