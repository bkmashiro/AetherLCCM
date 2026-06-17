use std::collections::HashMap;

use crate::errors::{ProtocolError, ProtocolResult};
use crate::types::{CryptoStatus, Hash, RiskLabel, SpacetimeTime};

#[derive(Debug, Clone)]
pub struct CryptoEra {
    pub era_id: String,
    pub hard_reject_after: SpacetimeTime,
    pub soft_deprecation_warning: Option<SpacetimeTime>,
    pub accepted_suites: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CryptoRisk {
    pub crypto_era_id: String,
    pub weakest_signature_suite: String,
    pub weakest_hash_suite: String,
    pub years_until_hard_reject: Option<i64>,
    pub years_until_soft_warning: Option<i64>,
    pub renewal_chain_depth: u32,
    pub has_hash_based_anchor: bool,
    pub has_physical_anchor: bool,
    pub risk_label: RiskLabel,
}

#[derive(Debug, Clone)]
pub struct CryptoPolicy {
    pub require_multi_family_archival: bool,
    pub allow_qkd_aux_only: bool,
    pub challenge_window: SpacetimeTime,
}

#[derive(Debug, Clone)]
pub struct CryptoVerification {
    pub accepted: bool,
    pub status: CryptoStatus,
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CryptoEraRegistry {
    pub eras: HashMap<String, CryptoEra>,
}

impl CryptoEraRegistry {
    pub fn default() -> Self {
        let mut eras = HashMap::new();
        eras.insert(
            "era-stable-multi-family".to_string(),
            CryptoEra {
                era_id: "era-stable-multi-family".to_string(),
                hard_reject_after: 200,
                soft_deprecation_warning: Some(140),
                accepted_suites: vec!["ml-dsa".to_string(), "slh-dsa".to_string()],
            },
        );
        eras.insert(
            "era-deprecated-single".to_string(),
            CryptoEra {
                era_id: "era-deprecated-single".to_string(),
                hard_reject_after: 80,
                soft_deprecation_warning: Some(40),
                accepted_suites: vec!["ml-dsa".to_string()],
            },
        );
        eras.insert(
            "era-stale".to_string(),
            CryptoEra {
                era_id: "era-stale".to_string(),
                hard_reject_after: 0,
                soft_deprecation_warning: None,
                accepted_suites: vec![],
            },
        );
        Self { eras }
    }

    pub fn resolve_by_hash(&self, proof_root: &Hash) -> CryptoEra {
        if proof_root == &[0u8; 32] {
            return self.eras.get("era-stale").cloned().unwrap_or(CryptoEra {
                era_id: "era-stale".to_string(),
                hard_reject_after: 0,
                soft_deprecation_warning: None,
                accepted_suites: Vec::new(),
            });
        }
        if proof_root[0] % 3 == 0 {
            return self
                .eras
                .get("era-deprecated-single")
                .cloned()
                .unwrap_or_else(|| self.backup_era());
        }
        self.eras
            .get("era-stable-multi-family")
            .cloned()
            .unwrap_or_else(|| self.backup_era())
    }

    fn backup_era(&self) -> CryptoEra {
        CryptoEra {
            era_id: "era-backup".to_string(),
            hard_reject_after: 120,
            soft_deprecation_warning: Some(90),
            accepted_suites: vec!["ml-dsa".to_string()],
        }
    }
}

pub fn verify_checkpoint_crypto(
    era: &CryptoEra,
    now: SpacetimeTime,
    proof: &Hash,
) -> ProtocolResult<CryptoVerification> {
    if proof == &[0u8; 32] {
        return Ok(CryptoVerification {
            accepted: false,
            status: CryptoStatus::CryptoStaleNeedsRenewal,
            reason: Some("checkpoint proof is placeholder hash".to_string()),
        });
    }
    if era.soft_deprecation_warning.is_none() && now > era.hard_reject_after {
        return Ok(CryptoVerification {
            accepted: false,
            status: CryptoStatus::CryptoBrokenUntrusted,
            reason: Some("proof timestamp after hard reject boundary".to_string()),
        });
    }
    if now > era.hard_reject_after {
        return Ok(CryptoVerification {
            accepted: false,
            status: CryptoStatus::CryptoBrokenUntrusted,
            reason: Some("proof timestamp after hard reject boundary".to_string()),
        });
    }
    if let Some(warn) = era.soft_deprecation_warning {
        if now >= warn {
            return Ok(CryptoVerification {
                accepted: true,
                status: CryptoStatus::CryptoDeprecatedButRenewed,
                reason: Some("proof in soft-deprecated range; renewable requirements apply".to_string()),
            });
        }
    }
    Ok(CryptoVerification {
        accepted: true,
        status: CryptoStatus::CryptoCurrent,
        reason: None,
    })
}

pub fn verify_checkpoint_claim(
    root: &Hash,
    observed_time: SpacetimeTime,
    registry: &CryptoEraRegistry,
) -> ProtocolResult<CryptoVerification> {
    let era = registry.resolve_by_hash(root);
    verify_checkpoint_crypto(&era, observed_time, root)
}

pub fn can_grant_new_credit(
    status: &CryptoStatus,
    risk: &CryptoRisk,
    settlement_horizon_years: u64,
) -> ProtocolResult<bool> {
    if matches!(
        status,
        CryptoStatus::CryptoStaleNeedsRenewal
            | CryptoStatus::CryptoBrokenUntrusted
            | CryptoStatus::CryptoUnknownSuite
    ) {
        return Ok(false);
    }
    if settlement_horizon_years == 0 {
        return Ok(false);
    }
    if let Some(years) = risk.years_until_hard_reject {
        if years < settlement_horizon_years as i64 {
            return Ok(false);
        }
    }
    if !risk.has_hash_based_anchor {
        return Ok(false);
    }
    if !risk.has_physical_anchor && matches!(risk.risk_label, RiskLabel::Low | RiskLabel::Medium) {
        // prefer explicit anchor-backed proofs for staged credit
        return Ok(false);
    }
    if matches!(
        risk.risk_label,
        RiskLabel::DoNotAcceptAtAll | RiskLabel::DoNotAcceptForCredit
    ) {
        return Ok(false);
    }
    Ok(true)
}

pub fn evaluate_crypto_risk(
    proof_root: &Hash,
    observed_time: SpacetimeTime,
    registry: &CryptoEraRegistry,
    renewal_parent: Option<&Hash>,
) -> ProtocolResult<CryptoRisk> {
    let verification = verify_checkpoint_claim(proof_root, observed_time, registry)?;
    let era = registry.resolve_by_hash(proof_root);
    let risk_label = match verification.status {
        CryptoStatus::CryptoCurrent => RiskLabel::Medium,
        CryptoStatus::CryptoDeprecatedButRenewed => RiskLabel::High,
        CryptoStatus::CryptoStaleNeedsRenewal => RiskLabel::DoNotAcceptForCredit,
        CryptoStatus::CryptoBrokenUntrusted => RiskLabel::DoNotAcceptAtAll,
        CryptoStatus::CryptoUnknownSuite => RiskLabel::High,
        CryptoStatus::PhysicalAnchorOnly => RiskLabel::High,
    };
    let remaining_hard = era.hard_reject_after - observed_time;
    let remaining_soft = era.soft_deprecation_warning.map(|warn| warn - observed_time);

    Ok(CryptoRisk {
        crypto_era_id: era.era_id,
        weakest_signature_suite: era
            .accepted_suites
            .get(0)
            .cloned()
            .unwrap_or_else(|| "unknown".to_string()),
        weakest_hash_suite: "shake256".to_string(),
        years_until_hard_reject: Some(remaining_hard),
        years_until_soft_warning: remaining_soft,
        renewal_chain_depth: if renewal_parent.is_some() {
            1
        } else {
            0
        },
        has_hash_based_anchor: !era.accepted_suites.is_empty(),
        has_physical_anchor: true,
        risk_label,
    })
}

pub fn assert_era_renewal_chain(
    new_root: &Hash,
    renewal_root: Option<&Hash>,
    observed_time: SpacetimeTime,
    registry: &CryptoEraRegistry,
) -> ProtocolResult<CryptoVerification> {
    let new_verification = verify_checkpoint_claim(new_root, observed_time, registry)?;
    if !new_verification.accepted {
        return Err(ProtocolError::Crypto {
            reason: "new checkpoint does not pass base era verification".to_string(),
        });
    }
    if let Some(parent) = renewal_root {
        let parent_verification = verify_checkpoint_claim(parent, observed_time, registry)?;
        if !parent_verification.accepted {
            return Err(ProtocolError::Crypto {
                reason: "renewal parent checkpoint is not trustable".to_string(),
            });
        }
    }
    Ok(new_verification)
}
