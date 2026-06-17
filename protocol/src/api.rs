use crate::causal::{LightconeProof, prove_lightcone};
use crate::checkpoint::{CheckpointBundle, CheckpointVerification};
use crate::errors::{ProtocolError, ProtocolResult};
use crate::types::{
    ClaimId, DomainId, EventId, SettlementClaim, SpacetimeTime, SpacetimeCoord, TxId, TxKind, Transaction,
};
use crate::{client::UniverseClient, settlement};

#[derive(Debug, Clone)]
pub struct SubmitLocalTxRequest {
    pub domain_id: DomainId,
    pub actor_id: String,
    pub kind: TxKind,
    pub payload_hash: [u8; 32],
    pub coord: SpacetimeCoord,
    pub causal_dependencies: Vec<EventId>,
    pub signatures: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct LockForExportRequest {
    pub lock_event_id: EventId,
    pub origin_domain: DomainId,
    pub remote_domain: DomainId,
    pub asset_id: String,
    pub amount: u64,
    pub coord: SpacetimeCoord,
    pub settlement_horizon_years: u64,
}

#[derive(Debug, Clone)]
pub struct VerifyRemoteCheckpointRequest {
    pub bundle: CheckpointBundle,
    pub observer_time: SpacetimeTime,
}

#[derive(Debug, Clone)]
pub struct VerifySettlementClaimRequest {
    pub claim_id: ClaimId,
    pub remote_observer_id: String,
}

#[derive(Debug, Clone)]
pub struct AcceptRemoteClaimRequest {
    pub claim_id: ClaimId,
    pub remote_checkpoint_id: String,
}

#[derive(Debug, Clone)]
pub struct ProvisionallyCreditRequest {
    pub claim_id: ClaimId,
    pub credit_line_id: String,
}

#[derive(Debug, Clone)]
pub struct ReconcileCheckpointRequest {
    pub bundle: CheckpointBundle,
}

#[derive(Debug, Clone)]
pub struct DetectConflictRequest {
    pub claim_a: ClaimId,
    pub claim_b: ClaimId,
}

#[derive(Debug, Clone)]
pub struct SlashOrDisputeRequest {
    pub claim_id: ClaimId,
    pub dispute_reason: String,
}

#[derive(Debug, Clone)]
pub struct ResyncAfterOfflineRequest {
    pub trusted_anchors: Vec<String>,
}

pub fn submit_local_tx(client: &mut UniverseClient, request: SubmitLocalTxRequest) -> ProtocolResult<crate::types::Event> {
    let tx_id = make_tx_id(&request.domain_id, &request.actor_id, &request.payload_hash);
    let tx = Transaction {
        tx_id: tx_id.clone(),
        domain_id: request.domain_id.clone(),
        kind: request.kind,
        actor_id: request.actor_id,
        payload_hash: request.payload_hash,
        coord: request.coord.clone(),
        inputs: Vec::new(),
        outputs: Vec::new(),
        causal_dependencies: request.causal_dependencies,
        signatures: request.signatures,
    };
    let domain = client.local_domains.get_mut(&request.domain_id).ok_or_else(|| ProtocolError::NotFound {
        what: "domain not found".to_string(),
    })?;
    let event = domain.submit_local_tx(tx_id, &tx)?;
    Ok(event)
}

pub fn lock_for_export(client: &mut UniverseClient, request: LockForExportRequest) -> ProtocolResult<SettlementClaim> {
    settlement::lock_for_export(
        client,
        request.lock_event_id,
        request.origin_domain,
        request.remote_domain,
        request.asset_id,
        request.amount,
        request.coord,
        request.settlement_horizon_years,
    )
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
    settlement::create_settlement_claim(
        client,
        claim_id,
        lock_event_id,
        origin_domain,
        remote_domain,
        asset_id,
        amount,
    )
}

pub fn verify_remote_checkpoint(client: &UniverseClient, request: VerifyRemoteCheckpointRequest) -> ProtocolResult<CheckpointVerification> {
    if request.observer_time < 0 {
        return Err(ProtocolError::Validation {
            reason: "observer_time must be non-negative".to_string(),
        });
    }
    settlement::verify_remote_checkpoint(client, &request.bundle)
}

pub fn verify_settlement_claim(
    client: &mut UniverseClient,
    request: VerifySettlementClaimRequest,
) -> ProtocolResult<crate::settlement::SettlementDecision> {
    let _ = request.remote_observer_id;
    settlement::verify_settlement_claim(client, &request.claim_id)
}

pub fn accept_remote_claim(client: &mut UniverseClient, request: AcceptRemoteClaimRequest) -> ProtocolResult<crate::settlement::SettlementDecision> {
    settlement::accept_remote_claim(client, &request.claim_id, &request.remote_checkpoint_id)
}

pub fn provisionally_credit(client: &mut UniverseClient, request: ProvisionallyCreditRequest) -> ProtocolResult<crate::settlement::SettlementDecision> {
    settlement::provisionally_credit(client, &request.claim_id, &request.credit_line_id)
}

pub fn reconcile_checkpoint(
    client: &mut UniverseClient,
    request: ReconcileCheckpointRequest,
) -> ProtocolResult<usize> {
    let bundles = request.bundle;
    crate::sync::reconcile_checkpoint(client, &bundles)
}

pub fn detect_conflict(
    client: &UniverseClient,
    request: DetectConflictRequest,
) -> ProtocolResult<crate::sync::ConflictDetectionResult> {
    let claim_a = client.pending_claims.get(&request.claim_a).ok_or_else(|| ProtocolError::NotFound {
        what: "claim_a missing".to_string(),
    })?;
    let claim_b = client.pending_claims.get(&request.claim_b).ok_or_else(|| ProtocolError::NotFound {
        what: "claim_b missing".to_string(),
    })?;
    Ok(crate::sync::detect_conflict(client, claim_a, claim_b))
}

pub fn slash_or_dispute(
    client: &mut UniverseClient,
    request: SlashOrDisputeRequest,
) -> ProtocolResult<String> {
    crate::sync::slash_or_dispute(client, &request.claim_id, &request.dispute_reason)
}

pub fn resync_after_offline(
    client: &mut UniverseClient,
    request: ResyncAfterOfflineRequest,
) -> ProtocolResult<crate::sync::ReconcileResult> {
    crate::sync::resync_after_offline(client, request.trusted_anchors)
}

pub fn lightcone_witness_path(
    route: Vec<crate::types::SpacetimeCoord>,
    speed_ly_per_year: f64,
) -> ProtocolResult<LightconeProof> {
    prove_lightcone("source", "sink", &route, speed_ly_per_year)
}

fn make_tx_id(domain: &str, actor: &str, payload: &[u8; 32]) -> TxId {
    let payload_hex: String = payload.iter().map(|byte| format!("{:02x}", byte)).collect();
    format!("tx:{}:{}:{}", domain, actor, payload_hex)
}
