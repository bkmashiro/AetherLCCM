use crate::checkpoint::{extract_checkpoint_hash, CheckpointVerification};
use crate::crypto::{can_grant_new_credit, evaluate_crypto_risk, verify_checkpoint_claim};
use crate::errors::{ProtocolError, ProtocolResult};
use crate::causal::{validate_claim_lightcone, validate_event_dependency_chain};
use crate::types::{
    hash_chunks,
    hash_to_hex,
    ClaimId, CryptoStatus, DomainId, EventId, FinalityStage, Hash, LightconeStatus, RiskLabel,
    SettlementClaim, SettlementPolicy,
};
use crate::client::UniverseClient;
use crate::types::{CheckpointId, Observation, SpacetimeCoord};
use crate::causal::CausalCertificate;
use crate::types::hash_hex_to_bytes;

#[derive(Debug, Clone)]
pub struct SettlementDecision {
    pub claim_id: ClaimId,
    pub approved: bool,
    pub finality: FinalityStage,
    pub reason: String,
}

pub fn lock_for_export(
    client: &mut UniverseClient,
    lock_event_id: EventId,
    origin_domain: DomainId,
    remote_domain: DomainId,
    asset_id: String,
    amount: u64,
    coord: SpacetimeCoord,
    settlement_horizon_years: u64,
) -> ProtocolResult<SettlementClaim> {
    if amount == 0 {
        return Err(ProtocolError::Validation {
            reason: "amount must be positive".to_string(),
        });
    }
    if origin_domain == remote_domain {
        return Err(ProtocolError::Validation {
            reason: "origin and remote domains must be distinct".to_string(),
        });
    }

    let policy = &client.config.settlement_policy;
    if settlement_horizon_years > policy.max_settlement_horizon_years {
        return Err(ProtocolError::Validation {
            reason: format!(
                "settlement horizon {settlement_horizon_years} exceeds policy max {}",
                policy.max_settlement_horizon_years
            ),
        });
    }

    let domain = client.local_domains.get(&origin_domain).ok_or_else(|| ProtocolError::NotFound {
        what: "origin domain not found".to_string(),
    })?;
    let lock_event = domain
        .get_event(&lock_event_id)
        .ok_or_else(|| ProtocolError::NotFound {
            what: "lock event not found in origin domain".to_string(),
        })?;
    let known_event_ids = domain
        .event_log
        .iter()
        .map(|evt| evt.event_id.clone())
        .collect::<Vec<_>>();
    validate_event_dependency_chain(&lock_event.event_id, &lock_event.causal_dependencies, &known_event_ids)?;
    let lightcone_status = match validate_claim_lightcone(
        &lock_event.coord,
        &coord,
        policy.lightcone_speed_ly_per_year,
    ) {
        Ok(_) => LightconeStatus::Valid,
        Err(_) => LightconeStatus::Unknown,
    };

    if client
        .pending_claims
        .values()
        .any(|c| c.lock_event_id == lock_event_id && c.origin_domain == origin_domain && c.finality != FinalityStage::BilaterallySettled)
    {
        return Err(ProtocolError::Conflict {
            reason: "existing non-settled claim already uses this lock event".to_string(),
        });
    }

    let lock_checkpoint = domain
        .final_checkpoint
        .as_ref()
        .map_or_else(|| synthetic_lock_checkpoint(&lock_event), |cp| cp.hash.clone());
    let claim_id = format!(
        "claim:{origin_domain}:{remote_domain}:{asset_id}:{amount}:{lock_event_id}"
    );

    if client.pending_claims.contains_key(&claim_id) {
        return Err(ProtocolError::Conflict {
            reason: "claim already exists".to_string(),
        });
    }

    let claim = SettlementClaim {
        claim_id: claim_id.clone(),
        lock_event_id,
        origin_domain: origin_domain.clone(),
        remote_domain: remote_domain.clone(),
        asset_id,
        amount,
        lock_checkpoint,
        lock_coord: coord,
        dependencies: lock_event.causal_dependencies.clone(),
        observations: Vec::new(),
        dependencies_causal: Vec::new(),
        signature_bundle: Vec::new(),
        finality: FinalityStage::RemoteObserved,
        crypto_status: CryptoStatus::CryptoCurrent,
        lightcone_status,
        risk_label: RiskLabel::Medium,
        settlement_horizon_years,
    };
    client.pending_claims.insert(claim_id.clone(), claim.clone());
    client.bootstrap_claim_status(
        claim_id.clone(),
        claim.finality.clone(),
        "created claim from lock".to_string(),
        claim.risk_label.clone(),
    );
    Ok(claim)
}

pub fn create_settlement_claim(
    client: &mut UniverseClient,
    claim_id: ClaimId,
    lock_event_id: EventId,
    origin_domain: DomainId,
    remote_domain: DomainId,
    asset_id: String,
    amount: u64,
) -> ProtocolResult<SettlementClaim> {
    if client.pending_claims.contains_key(&claim_id) {
        return Err(ProtocolError::Conflict {
            reason: "claim id already exists".to_string(),
        });
    }
    if origin_domain == remote_domain {
        return Err(ProtocolError::Validation {
            reason: "origin and remote domains must be distinct".to_string(),
        });
    }
    if amount == 0 {
        return Err(ProtocolError::Validation {
            reason: "amount must be positive".to_string(),
        });
    }
    let domain = client.local_domains.get(&origin_domain).ok_or_else(|| ProtocolError::NotFound {
        what: "origin domain not found".to_string(),
    })?;
    let lock_event = domain
        .get_event(&lock_event_id)
        .ok_or_else(|| ProtocolError::Validation {
            reason: "lock event does not exist in origin domain".to_string(),
        })?;
    let known_event_ids = domain
        .event_log
        .iter()
        .map(|evt| evt.event_id.clone())
        .collect::<Vec<_>>();
    validate_event_dependency_chain(&lock_event.event_id, &lock_event.causal_dependencies, &known_event_ids)?;
    if client
        .pending_claims
        .values()
        .any(|c| c.lock_event_id == lock_event_id && c.origin_domain == origin_domain && c.finality != FinalityStage::BilaterallySettled)
    {
        return Err(ProtocolError::Conflict {
            reason: "existing non-settled claim already uses this lock event".to_string(),
        });
    }

    let lock_checkpoint = domain
        .final_checkpoint
        .as_ref()
        .map_or_else(|| synthetic_lock_checkpoint(&lock_event), |cp| cp.hash.clone());

    if claim_id.trim().is_empty() {
        return Err(ProtocolError::Validation {
            reason: "claim id cannot be empty".to_string(),
        });
    }

    let claim = SettlementClaim {
        claim_id: claim_id.clone(),
        lock_event_id,
        origin_domain,
        remote_domain,
        asset_id,
        amount,
        lock_checkpoint,
        lock_coord: lock_event.coord.clone(),
        dependencies: lock_event.causal_dependencies.clone(),
        observations: Vec::new(),
        dependencies_causal: Vec::new(),
        signature_bundle: Vec::new(),
        finality: FinalityStage::RemoteObserved,
        crypto_status: CryptoStatus::CryptoCurrent,
        lightcone_status: LightconeStatus::Unknown,
        risk_label: RiskLabel::Medium,
        settlement_horizon_years: 5,
    };
    client
        .pending_claims
        .insert(claim_id.clone(), claim.clone());
    client.bootstrap_claim_status(
        claim_id.clone(),
        FinalityStage::RemoteObserved,
        "created explicit settlement claim".to_string(),
        RiskLabel::Medium,
    );
    Ok(claim)
}

pub fn verify_remote_checkpoint(
    client: &UniverseClient,
    bundle: &crate::checkpoint::CheckpointBundle,
) -> ProtocolResult<CheckpointVerification> {
    let local_root = client
        .local_roots
        .get(&bundle.checkpoint.domain_id)
        .ok_or_else(|| ProtocolError::NotFound {
            what: "domain root not tracked".to_string(),
        })?;
    crate::checkpoint::verify_remote_checkpoint(bundle, local_root)
}

pub fn verify_settlement_claim(
    client: &mut UniverseClient,
    claim_id: &ClaimId,
) -> ProtocolResult<SettlementDecision> {
    let claim = client
        .pending_claims
        .get(claim_id)
        .ok_or_else(|| ProtocolError::NotFound {
            what: "claim not found".to_string(),
        })?
        .clone();

    if claim.lock_checkpoint.is_empty() || claim.lock_checkpoint.len() != 64 {
        return Ok(SettlementDecision {
            claim_id: claim.claim_id.clone(),
            approved: false,
            finality: claim.finality,
            reason: "lock checkpoint missing or malformed for claim verification".to_string(),
        });
    }

    let policy = &client.config.settlement_policy;
    if settlement_horizon_exceeds_policy(&policy, claim.settlement_horizon_years) {
        return Ok(SettlementDecision {
            claim_id: claim.claim_id.clone(),
            approved: false,
            finality: claim.finality,
            reason: "claim horizon exceeds local policy".to_string(),
        });
    }
    if claim.lightcone_status == LightconeStatus::Invalid {
        return Ok(SettlementDecision {
            claim_id: claim.claim_id.clone(),
            approved: false,
            finality: claim.finality,
            reason: "claim has invalid lightcone proof".to_string(),
        });
    }

    let root: Hash = hash_hex_to_bytes(&claim.lock_checkpoint).map_err(|_| ProtocolError::Validation {
        reason: "lock checkpoint hash is not well formed".to_string(),
    })?;
    let crypto_verification = verify_checkpoint_claim(
        &root,
        claim.settlement_horizon_years as i64,
        &client.crypto_registry,
    )?;
    if !crypto_verification.accepted {
        return Ok(SettlementDecision {
            claim_id: claim.claim_id.clone(),
            approved: false,
            finality: claim.finality,
            reason: format!(
                "crypto verification failed: {}",
                crypto_verification
                    .reason
                    .unwrap_or_else(|| "not trusted".to_string())
            ),
        });
    }
    let risk = evaluate_crypto_risk(
        &root,
        claim.settlement_horizon_years as i64,
        &client.crypto_registry,
        None,
    )?;
    if !can_grant_new_credit(&crypto_verification.status, &risk, claim.settlement_horizon_years)? {
        return Ok(SettlementDecision {
            claim_id: claim.claim_id.clone(),
            approved: false,
            finality: claim.finality,
            reason: "crypto policy blocks additional credit".to_string(),
        });
    }

    if claim.finality == FinalityStage::BilaterallySettled {
        return Ok(SettlementDecision {
            claim_id: claim.claim_id.clone(),
            approved: false,
            finality: claim.finality,
            reason: "claim already final".to_string(),
        });
    }

    if let Some(staged) = client.pending_claims.get_mut(claim_id) {
        staged.crypto_status = crypto_verification.status;
        staged.risk_label = risk.risk_label.clone();
    }

    Ok(SettlementDecision {
        claim_id: claim.claim_id.clone(),
        approved: true,
        finality: claim.finality,
        reason: "claim passed staged settlement checks".to_string(),
    })
}

pub fn accept_remote_claim(
    client: &mut UniverseClient,
    claim_id: &ClaimId,
    remote_checkpoint_id: &crate::types::CheckpointId,
) -> ProtocolResult<SettlementDecision> {
    let claim = client.pending_claims.get(claim_id).ok_or_else(|| ProtocolError::NotFound {
        what: "claim not found".to_string(),
    })?;
    if claim.finality == FinalityStage::BilaterallySettled {
        return Err(ProtocolError::Conflict {
            reason: "claim already bilaterally settled".to_string(),
        });
    }
    if claim.finality == FinalityStage::AcceptedByRemoteLedger {
        return Ok(SettlementDecision {
            claim_id: claim_id.clone(),
            approved: true,
            finality: claim.finality.clone(),
            reason: format!("claim already accepted by remote checkpoint {remote_checkpoint_id}"),
        });
    }
    if remote_checkpoint_id.is_empty() {
        return Err(ProtocolError::Validation {
            reason: "remote checkpoint id cannot be empty".to_string(),
        });
    }
    let sync_state = client
        .sync_state
        .get(&claim.origin_domain)
        .ok_or_else(|| ProtocolError::NotFound {
            what: "sync state missing".to_string(),
        })?;
    if !sync_state.frontier.contains(remote_checkpoint_id)
        && sync_state.last_trusted_checkpoint.as_ref() != Some(remote_checkpoint_id)
    {
        return Err(ProtocolError::Validation {
            reason: "remote checkpoint has not been reconciled locally".to_string(),
        });
    }

    client.update_claim_status(
        claim_id,
        FinalityStage::AcceptedByRemoteLedger,
        format!("accepted via remote checkpoint {remote_checkpoint_id}"),
        claim.risk_label.clone(),
    )?;
    Ok(SettlementDecision {
        claim_id: claim_id.clone(),
        approved: true,
        finality: FinalityStage::AcceptedByRemoteLedger,
        reason: format!("accepted via remote checkpoint {remote_checkpoint_id}"),
    })
}

pub fn provisionally_credit(
    client: &mut UniverseClient,
    claim_id: &ClaimId,
    credit_line_id: &str,
) -> ProtocolResult<SettlementDecision> {
    let mut claim = client
        .pending_claims
        .get(claim_id)
        .cloned()
        .ok_or_else(|| ProtocolError::NotFound {
            what: "claim not found".to_string(),
        })?;
    if claim.lightcone_status == LightconeStatus::Invalid {
        return Err(ProtocolError::Validation {
            reason: "cannot credit claim with invalid lightcone".to_string(),
        });
    }
    if claim.lock_checkpoint.is_empty() || claim.lock_checkpoint.len() != 64 {
        return Err(ProtocolError::Validation {
            reason: "lock checkpoint must be present and hash-formatted for credit".to_string(),
        });
    }
    if claim.finality != FinalityStage::RemoteObserved {
        return Err(ProtocolError::Conflict {
            reason: "claim status not eligible for provisional credit".to_string(),
        });
    }
    let root = derive_claim_root(&claim);
    let crypto_verification = verify_checkpoint_claim(
        &root,
        claim.settlement_horizon_years as i64,
        &client.crypto_registry,
    )?;
    let risk = evaluate_crypto_risk(
        &root,
        claim.settlement_horizon_years as i64,
        &client.crypto_registry,
        None,
    )?;
    if !crypto_verification.accepted {
        return Err(ProtocolError::Validation {
            reason: "crypto policy blocks provisional credit".to_string(),
        });
    }
    if !can_grant_new_credit(&crypto_verification.status, &risk, claim.settlement_horizon_years)? {
        return Err(ProtocolError::Validation {
            reason: "crypto policy blocks provisional credit".to_string(),
        });
    }

    let policy = &client.config.settlement_policy;
    if claim.settlement_horizon_years > policy.max_settlement_horizon_years {
        return Err(ProtocolError::Validation {
            reason: "claim horizon exceeds policy limit".to_string(),
        });
    }

    let line = client
        .credit_lines
        .get_mut(credit_line_id)
        .ok_or_else(|| ProtocolError::NotFound {
            what: "credit line missing".to_string(),
        })?;
    if line.used.saturating_add(claim.amount) > line.limit {
        return Err(ProtocolError::Validation {
            reason: "credit line exhausted".to_string(),
        });
    }
    line.used = line.used.saturating_add(claim.amount);
    claim.crypto_status = crypto_verification.status;
    claim.risk_label = risk_label_or_default(&risk.risk_label, &policy.auto_settlement_risk_floor);
    client
        .pending_claims
        .insert(claim_id.to_string(), claim.clone());
    client.update_claim_status(
        claim_id,
        FinalityStage::ProvisionallyCredited,
        format!("provisional credit through {credit_line_id}"),
        claim.risk_label.clone(),
    )?;
    Ok(SettlementDecision {
        claim_id: claim_id.clone(),
        approved: true,
        finality: FinalityStage::ProvisionallyCredited,
        reason: format!("provisional credit through {credit_line_id}"),
    })
}

pub fn add_observation(claim: &mut SettlementClaim, observation: Observation) {
    claim.observations.push(observation);
}

pub fn update_causal_status(
    claim: &mut SettlementClaim,
    status: LightconeStatus,
) {
    claim.lightcone_status = status;
}

pub fn reconcile_causal(
    _claim: &mut SettlementClaim,
    _cert: &CausalCertificate,
) -> bool {
    true
}

pub fn apply_checkpoint_to_claim(
    client: &mut UniverseClient,
    claim_id: &ClaimId,
    checkpoint_id: &CheckpointId,
) -> ProtocolResult<()> {
    let claim = client
        .pending_claims
        .get_mut(claim_id)
        .ok_or_else(|| ProtocolError::NotFound {
            what: "claim not found".to_string(),
        })?;
    claim.lock_checkpoint = checkpoint_id.clone();
    claim.lightcone_status = LightconeStatus::Unknown;
    Ok(())
}

pub fn mark_bilaterally_settled(
    client: &mut UniverseClient,
    claim_id: &ClaimId,
) -> ProtocolResult<()> {
    let claim = client.pending_claims.get(claim_id).ok_or_else(|| ProtocolError::NotFound {
        what: "claim not found".to_string(),
    })?;
    if claim.finality != FinalityStage::OriginAcknowledged {
        return Err(ProtocolError::Conflict {
            reason: "claim must be origin acknowledged before bilateral settlement".to_string(),
        });
    }
    let _ = claim;
    client.update_claim_status(
        claim_id,
        FinalityStage::BilaterallySettled,
        "both domains acknowledged settlement".to_string(),
        RiskLabel::Low,
    )?;
    Ok(())
}

pub fn _unused_causal_dep_reference(_checkpoint: &CheckpointId, _cert: &CausalCertificate) {
    let _ = extract_checkpoint_hash;
}

fn settlement_horizon_exceeds_policy(policy: &SettlementPolicy, horizon: u64) -> bool {
    horizon > policy.max_settlement_horizon_years
}

fn derive_claim_root(claim: &SettlementClaim) -> Hash {
    if claim.lock_checkpoint.is_empty() {
        return [0u8; 32];
    }
    hash_hex_to_bytes(&claim.lock_checkpoint).unwrap_or([0u8; 32])
}

fn risk_label_or_default(source: &RiskLabel, floor: &RiskLabel) -> RiskLabel {
    if risk_worse_than_floor(source, floor) {
        floor.clone()
    } else {
        source.clone()
    }
}

fn risk_worse_than_floor(candidate: &RiskLabel, floor: &RiskLabel) -> bool {
    risk_rank(candidate) < risk_rank(floor)
}

fn synthetic_lock_checkpoint(event: &crate::types::Event) -> String {
    let bytes = event.canonical_bytes();
    hash_to_hex(&hash_chunks(&[bytes.as_slice()]))
}

fn risk_rank(label: &RiskLabel) -> u8 {
    match label {
        RiskLabel::Low => 0,
        RiskLabel::Medium => 1,
        RiskLabel::High => 2,
        RiskLabel::DoNotAcceptForCredit => 3,
        RiskLabel::DoNotAcceptAtAll => 4,
    }
}
