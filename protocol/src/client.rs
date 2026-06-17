use std::collections::HashMap;

use crate::checkpoint::Checkpoint;
use crate::errors::{ProtocolError, ProtocolResult};
use crate::crypto::CryptoEraRegistry;
use crate::ledger::LocalLedgerDomain;
use crate::types::{
    ClientSyncState, CreditLine, DisputeRecord, DomainId, FinalityStage, Hash, RiskLabel, SettlementClaim,
    SettlementPolicy,
};

#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub local_domain: DomainId,
    pub challenge_window: i64,
    pub checkpoint_grace_period: i64,
    pub settlement_policy: SettlementPolicy,
}

#[derive(Debug, Clone)]
pub struct SettlementTransitionLog {
    pub transition_id: u64,
    pub claim_id: String,
    pub from_stage: FinalityStage,
    pub to_stage: FinalityStage,
    pub reason: String,
    pub risk_label: RiskLabel,
}

#[derive(Debug)]
pub struct UniverseClient {
    pub config: ClientConfig,
    pub local_domains: HashMap<DomainId, LocalLedgerDomain>,
    pub claim_status: HashMap<String, FinalityStage>,
    pub pending_claims: HashMap<String, SettlementClaim>,
    pub disputes: HashMap<String, DisputeRecord>,
    pub credit_lines: HashMap<String, CreditLine>,
    pub sync_state: HashMap<DomainId, ClientSyncState>,
    pub local_roots: HashMap<DomainId, Hash>,
    pub crypto_registry: CryptoEraRegistry,
    pub transition_log: Vec<SettlementTransitionLog>,
    pub default_coord: crate::types::SpacetimeCoord,
}

impl UniverseClient {
    pub fn new(config: ClientConfig, default_coord: crate::types::SpacetimeCoord) -> Self {
        Self {
            config,
            local_domains: HashMap::new(),
            claim_status: HashMap::new(),
            pending_claims: HashMap::new(),
            disputes: HashMap::new(),
            credit_lines: HashMap::new(),
            sync_state: HashMap::new(),
            local_roots: HashMap::new(),
            crypto_registry: CryptoEraRegistry::default(),
            transition_log: Vec::new(),
            default_coord,
        }
    }

    pub fn add_domain(&mut self, domain: DomainId) -> ProtocolResult<()> {
        if self.local_domains.contains_key(&domain) {
            return Err(ProtocolError::Conflict {
                reason: "domain already exists".to_string(),
            });
        }
        self.local_domains
            .insert(domain.clone(), LocalLedgerDomain::new(domain.clone()));
        self.local_roots.insert(domain.clone(), [0u8; 32]);
        self.sync_state.insert(
            domain.clone(),
            ClientSyncState {
                domain,
                last_trusted_checkpoint: None,
                frontier: Vec::new(),
                sync_watermark: 0,
            },
        );
        Ok(())
    }

    pub fn update_claim_status(
        &mut self,
        claim_id: &str,
        next: FinalityStage,
        reason: String,
        risk_label: RiskLabel,
    ) -> ProtocolResult<()> {
        let claim = self.pending_claims.get_mut(claim_id).ok_or_else(|| ProtocolError::NotFound {
            what: "claim missing".to_string(),
        })?;
        let prev = claim.finality.clone();
        if !Self::is_status_monotonic(&prev, &next) {
            return Err(ProtocolError::Conflict {
                reason: "non-monotonic claim transition".to_string(),
            });
        }
        claim.finality = next.clone();
        self.claim_status.insert(claim_id.to_string(), next.clone());
        self.transition_log.push(SettlementTransitionLog {
            transition_id: self.transition_log.len() as u64 + 1,
            claim_id: claim_id.to_string(),
            from_stage: prev,
            to_stage: next,
            reason,
            risk_label,
        });
        Ok(())
    }

    pub fn bootstrap_claim_status(&mut self, claim_id: String, stage: FinalityStage, reason: String, risk_label: RiskLabel) {
        self.claim_status.insert(claim_id.clone(), stage.clone());
        self.transition_log.push(SettlementTransitionLog {
            transition_id: self.transition_log.len() as u64 + 1,
            claim_id,
            from_stage: stage.clone(),
            to_stage: stage,
            reason,
            risk_label,
        });
    }

    pub fn get_claim_status(&self, claim_id: &str) -> Option<&FinalityStage> {
        self.claim_status.get(claim_id)
    }

    pub fn get_domain_latest_checkpoint(&self, domain: &DomainId) -> Option<&Checkpoint> {
        self.local_domains.get(domain).and_then(|d| d.final_checkpoint.as_ref())
    }

    pub fn all_claim_ids(&self) -> Vec<String> {
        self.pending_claims.keys().cloned().collect()
    }

    pub fn is_status_monotonic(prev: &FinalityStage, next: &FinalityStage) -> bool {
        use FinalityStage::*;
        let rank = |s: &FinalityStage| -> i32 {
            match s {
                LocalFinal => 0,
                RemoteObserved => 1,
                ProvisionallyCredited => 2,
                AcceptedByRemoteLedger => 3,
                OriginAcknowledged => 4,
                BilaterallySettled => 5,
            }
        };
        rank(prev) <= rank(next)
    }
}
