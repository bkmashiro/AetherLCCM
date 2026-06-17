use std::collections::{HashSet, BTreeSet};

use crate::checkpoint::{extract_checkpoint_hash, CheckpointBundle};
use crate::errors::{ProtocolError, ProtocolResult};
use crate::types::{ClaimId, DisputeRecord, SettlementClaim};
use crate::client::UniverseClient;

#[derive(Debug, Clone)]
pub struct ReconcileResult {
    pub reconciled_checkpoints: usize,
    pub conflicts_detected: usize,
    pub synced_domains: usize,
}

#[derive(Debug, Clone)]
pub struct ConflictDetectionResult {
    pub claim_a: ClaimId,
    pub claim_b: ClaimId,
    pub detected: bool,
    pub evidence: Vec<String>,
}

pub fn reconcile_checkpoint(client: &mut UniverseClient, bundle: &CheckpointBundle) -> ProtocolResult<usize> {
    let cp_id = extract_checkpoint_hash(&bundle.checkpoint);
    let domain_state = client.local_domains.get_mut(&bundle.checkpoint.domain_id).ok_or_else(|| ProtocolError::NotFound {
        what: "domain not found".to_string(),
    })?;
    if let Some(existing) = &domain_state.final_checkpoint {
        if bundle.checkpoint.height < existing.height {
            return Err(ProtocolError::Conflict {
                reason: "replayed checkpoint would rewind local domain finality".to_string(),
            });
        }
        if bundle.checkpoint.height == existing.height && existing.hash != bundle.checkpoint.hash {
            return Err(ProtocolError::Conflict {
                reason: "checkpoint height conflict: same height with different hash".to_string(),
            });
        }
    }
    domain_state.final_checkpoint = Some(bundle.checkpoint.clone());
    let sync_state = client
        .sync_state
        .get_mut(&bundle.checkpoint.domain_id)
        .ok_or_else(|| ProtocolError::NotFound {
            what: "sync state missing".to_string(),
        })?;
    if !sync_state.frontier.contains(&cp_id) {
        sync_state.frontier.push(cp_id.clone());
    }
    sync_state.last_trusted_checkpoint = Some(cp_id.clone());
    sync_state.sync_watermark = sync_state.sync_watermark.max(bundle.checkpoint.height as i64);
    Ok(1)
}

pub fn detect_conflict(
    _client: &UniverseClient,
    claim_a: &SettlementClaim,
    claim_b: &SettlementClaim,
) -> ConflictDetectionResult {
    let conflict = claim_a.claim_id != claim_b.claim_id
        && claim_a.origin_domain == claim_b.origin_domain
        && claim_a.finality != crate::types::FinalityStage::BilaterallySettled
        && claim_b.finality != crate::types::FinalityStage::BilaterallySettled
        && (claim_a.lock_event_id == claim_b.lock_event_id
            || (claim_a.asset_id == claim_b.asset_id && claim_a.amount == claim_b.amount));
    let evidence = if conflict {
        let mut evidence = vec![
            format!(
                "conflicting claims in domain {} for event {}",
                claim_a.origin_domain, claim_a.lock_event_id
            ),
            format!("claim_a={} claim_b={}", claim_a.claim_id, claim_b.claim_id),
        ];
        if claim_a.finality == crate::types::FinalityStage::AcceptedByRemoteLedger
            && claim_b.finality == crate::types::FinalityStage::AcceptedByRemoteLedger
        {
            evidence.push("both claims already accepted by remote".to_string());
        }
        evidence
    } else {
        Vec::new()
    };
    ConflictDetectionResult {
        claim_a: claim_a.claim_id.clone(),
        claim_b: claim_b.claim_id.clone(),
        detected: conflict,
        evidence,
    }
}

pub fn slash_or_dispute(client: &mut UniverseClient, claim_id: &str, dispute_kind: &str) -> ProtocolResult<String> {
    let claim = client.pending_claims.get(claim_id).ok_or_else(|| ProtocolError::NotFound {
        what: "claim not found".to_string(),
    })?;
    let dispute = DisputeRecord {
        dispute_id: format!("dispute-{claim_id}"),
        claim_id: claim_id.to_string(),
        kind: dispute_kind.to_string(),
        evidence: Default::default(),
        created_at: 0,
        status: "open".to_string(),
    };
    client.disputes.insert(dispute.dispute_id.clone(), dispute);
    Ok(format!("dispute opened for claim {}", claim.claim_id))
}

pub fn resync_after_offline(client: &mut UniverseClient, trusted_anchors: Vec<String>) -> ProtocolResult<ReconcileResult> {
    let mut reconciled = 0;
    let mut conflicts = 0;
    let mut unique_anchors = BTreeSet::new();
    for anchor in trusted_anchors {
        unique_anchors.insert(anchor);
    }
    let _synced = unique_anchors.len();

    let mut domain_applied = HashSet::new();
    for anchor in &unique_anchors {
        let (target_domain, cp_id, optional_height) = parse_trusted_anchor(client, anchor)?;

        if let Some(domain) = &target_domain {
            let sync_state = client
                .sync_state
                .get_mut(domain)
                .ok_or_else(|| ProtocolError::NotFound {
                    what: "trusted anchor references unknown domain".to_string(),
                })?;
            if !sync_state.frontier.contains(&cp_id) {
                sync_state.frontier.push(cp_id.clone());
            }
            sync_state.last_trusted_checkpoint = Some(cp_id.clone());
            if let Some(height) = optional_height {
                sync_state.sync_watermark = sync_state.sync_watermark.max(height);
            }
            domain_applied.insert(domain.clone());
        } else {
            for state in client.sync_state.values_mut() {
                if !state.frontier.contains(&cp_id) {
                    state.frontier.push(cp_id.clone());
                }
                state.last_trusted_checkpoint = Some(cp_id.clone());
                if let Some(height) = optional_height {
                    state.sync_watermark = state.sync_watermark.max(height);
                }
                domain_applied.insert(state.domain.clone());
            }
        }
    }

    let mut targets: Vec<String> = client
        .pending_claims
        .iter()
        .filter_map(|(id, claim)| {
            if claim.finality == crate::types::FinalityStage::ProvisionallyCredited {
                Some(id.clone())
            } else {
                None
            }
        })
        .collect();

    for claim_id in targets.drain(..) {
        if let Some(claim) = client.pending_claims.get(&claim_id) {
            if claim.finality == crate::types::FinalityStage::BilaterallySettled {
                continue;
            }
        }
        claims_update_transition_log(client, &claim_id);
        conflicts += 0;
        reconciled += 1;
    }
    Ok(ReconcileResult {
        reconciled_checkpoints: reconciled,
        conflicts_detected: conflicts,
        synced_domains: domain_applied.len(),
    })
}

fn parse_trusted_anchor(
    client: &UniverseClient,
    anchor: &str,
) -> ProtocolResult<(Option<String>, String, Option<i64>)> {
    let parts: Vec<&str> = anchor.split(':').collect();
    if parts.len() == 2 {
        let possible_domain = parts[0];
        if client.sync_state.contains_key(possible_domain) {
            return Ok((Some(possible_domain.to_string()), parts[1].to_string(), None));
        }
        return Ok((None, anchor.to_string(), None));
    }

    if parts.len() >= 3 {
        let possible_domain = parts[0];
        if !client.sync_state.contains_key(possible_domain) {
            return Err(ProtocolError::NotFound {
                what: "trusted anchor references unknown domain".to_string(),
            });
        }
        let height = parts[1].parse::<i64>().ok();
        let checkpoint_id = parts.last().copied().unwrap_or_default().to_string();
        return Ok((Some(possible_domain.to_string()), checkpoint_id, height));
    }

    Ok((None, anchor.to_string(), None))
}

fn claims_update_transition_log(client: &mut UniverseClient, claim_id: &str) {
    if let Some(claim) = client.pending_claims.get(claim_id) {
        let current = client
            .claim_status
            .get(claim_id)
            .cloned()
            .unwrap_or_else(|| claim.finality.clone());
        client.transition_log.push(crate::client::SettlementTransitionLog {
            transition_id: client.transition_log.len() as u64 + 1,
            claim_id: claim_id.to_string(),
            from_stage: current.clone(),
            to_stage: current.clone(),
            reason: "offline replay checkpoint".to_string(),
            risk_label: claim.risk_label.clone(),
        });
        client.claim_status.insert(claim_id.to_string(), current);
    }
}
