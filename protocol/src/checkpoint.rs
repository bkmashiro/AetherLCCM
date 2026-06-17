use crate::causal::CausalCertificate;
use crate::errors::{ProtocolError, ProtocolResult};
use crate::types::{
    hash_to_hex, CheckpointId, DomainId, Hash, Observation, SpacetimeCoord,
};

#[derive(Debug, Clone)]
pub struct QuorumCertificate {
    pub signers: Vec<String>,
    pub threshold: u32,
    pub root: Hash,
    pub signature_bundle: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Checkpoint {
    pub domain_id: DomainId,
    pub height: u64,
    pub epoch: u64,
    pub prev_checkpoint_hash: Option<Hash>,
    pub state_root: Hash,
    pub tx_root: Hash,
    pub event_log_root: Hash,
    pub export_root: Hash,
    pub import_root: Hash,
    pub observed_remote_root: Hash,
    pub dispute_root: Hash,
    pub validator_set_root: Hash,
    pub coord: SpacetimeCoord,
    pub protocol_version: u32,
    pub state_commitment: Hash,
    pub hash: CheckpointId,
}

#[derive(Debug, Clone)]
pub struct CheckpointBundle {
    pub checkpoint: Checkpoint,
    pub quorum_certificate: QuorumCertificate,
    pub causal_certificate: Option<CausalCertificate>,
    pub observation: Vec<Observation>,
}

#[derive(Debug, Clone)]
pub struct CheckpointVerification {
    pub checkpoint_id: CheckpointId,
    pub valid: bool,
    pub reasons: Vec<String>,
}

pub fn checkpoint_id_from_parts(checkpoint: &Checkpoint) -> CheckpointId {
    hash_to_hex(&checkpoint.state_commitment)
}

pub fn verify_checkpoint_quorum(quorum: &QuorumCertificate, expected_root: &Hash) -> ProtocolResult<()> {
    if quorum.threshold == 0 {
        return Err(ProtocolError::Validation {
            reason: "quorum threshold must be positive".to_string(),
        });
    }
    if quorum.signers.len() < quorum.threshold as usize {
        return Err(ProtocolError::Validation {
            reason: "insufficient signers for quorum threshold".to_string(),
        });
    }
    if quorum.root != *expected_root {
        return Err(ProtocolError::Validation {
            reason: "quorum root does not match checkpoint commitment".to_string(),
        });
    }
    Ok(())
}

pub fn verify_remote_checkpoint(bundle: &CheckpointBundle, local_validator_root: &Hash) -> ProtocolResult<CheckpointVerification> {
    let mut reasons = Vec::new();

    if bundle.quorum_certificate.threshold == 0 {
        reasons.push("quorum threshold missing".to_string());
    }
    if checkpoint_id_from_parts(&bundle.checkpoint) != bundle.checkpoint.hash {
        reasons.push("checkpoint hash field does not match its commitment".to_string());
    }
    if bundle.checkpoint.validator_set_root != *local_validator_root {
        reasons.push("validator set root does not match local expectations".to_string());
    }
    if let Some(causal) = &bundle.causal_certificate {
        if causal.dependencies.is_empty() && bundle.checkpoint.height > 0 {
            reasons.push("non-genesis checkpoint has empty causal frontier".to_string());
        }
    } else if bundle.checkpoint.height > 0 {
        reasons.push("non-genesis checkpoint missing causal certificate".to_string());
    }

    if let Err(err) = verify_checkpoint_quorum(&bundle.quorum_certificate, &bundle.checkpoint.state_commitment) {
        reasons.push(err.to_string());
    }

    Ok(CheckpointVerification {
        checkpoint_id: bundle.checkpoint.hash.clone(),
        valid: reasons.is_empty(),
        reasons,
    })
}

pub fn extract_checkpoint_hash(checkpoint: &Checkpoint) -> CheckpointId {
    checkpoint.hash.clone()
}

pub fn extract_checkpoint_commitment(checkpoint: &Checkpoint) -> Hash {
    checkpoint.state_commitment
}
