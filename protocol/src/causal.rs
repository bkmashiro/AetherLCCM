use crate::errors::{ProtocolError, ProtocolResult};
use crate::types::{EventId, LightconeStatus, Message, Region3D, SpacetimeCoord};

#[derive(Debug, Clone)]
pub struct CausalDependency {
    pub from_event: EventId,
    pub to_event: EventId,
    pub via_checkpoint: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CausalCertificate {
    pub dependencies: Vec<CausalDependency>,
    pub frontier: Vec<EventId>,
    pub time_interval: (i64, i64),
    pub signature_bundle: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct LightconeHop {
    pub from_event: EventId,
    pub to_event: EventId,
    pub from_region: Region3D,
    pub to_region: Region3D,
    pub min_distance_ly: f64,
    pub from_t_min: i64,
    pub to_t_min: i64,
}

#[derive(Debug, Clone)]
pub struct LightconeProof {
    pub hops: Vec<LightconeHop>,
    pub status: LightconeStatus,
    pub reason: Option<String>,
}

pub fn lightcone_possible(e1: &SpacetimeCoord, e2: &SpacetimeCoord, speed_ly_per_year: f64) -> bool {
    if e2.time_interval.t_min < e1.time_interval.t_min {
        return false;
    }

    let delta_t = (e2.time_interval.t_min - e1.time_interval.t_max) as f64;
    let min_distance = e1.position_region.min_distance_ly(&e2.position_region);
    delta_t >= min_distance / speed_ly_per_year
}

pub fn prove_lightcone(
    from_event: &str,
    to_event: &str,
    coord_chain: &[SpacetimeCoord],
    speed_ly_per_year: f64,
) -> ProtocolResult<LightconeProof> {
    if coord_chain.len() < 2 {
        return Err(ProtocolError::Validation {
            reason: "lightcone proof requires at least source and sink coords".to_string(),
        });
    }

    let mut hops = Vec::new();
    for i in 0..coord_chain.len() - 1 {
        let a = &coord_chain[i];
        let b = &coord_chain[i + 1];
        let hop = LightconeHop {
            from_event: from_event.to_string(),
            to_event: to_event.to_string(),
            from_region: a.position_region.clone(),
            to_region: b.position_region.clone(),
            min_distance_ly: a.position_region.min_distance_ly(&b.position_region),
            from_t_min: a.time_interval.t_min,
            to_t_min: b.time_interval.t_min,
        };
        hops.push(hop);
    }

    let valid = coord_chain.windows(2).all(|w| lightcone_possible(&w[0], &w[1], speed_ly_per_year));
    let status = if valid { LightconeStatus::Valid } else { LightconeStatus::Invalid };
    let reason = if valid {
        None
    } else {
        Some("one or more causal hops violate the lightcone".to_string())
    };

    Ok(LightconeProof {
        hops,
        status,
        reason,
    })
}

pub fn validate_message_route(message: &Message, min_hops: usize) -> ProtocolResult<()> {
    validate_message_route_with_policy(message, min_hops, 1)
}

pub fn validate_message_route_with_policy(
    message: &Message,
    min_hops: usize,
    min_nonce_chars: usize,
) -> ProtocolResult<()> {
    if message.route.len() < min_hops {
        return Err(ProtocolError::Validation {
            reason: "message route is shorter than protocol minimum".to_string(),
        });
    }
    if message.route.is_empty() {
        return Err(ProtocolError::Validation {
            reason: "message route cannot be empty".to_string(),
        });
    }
    if message.anti_replay_nonce.len() < min_nonce_chars {
        return Err(ProtocolError::Validation {
            reason: "anti-replay nonce too short for policy".to_string(),
        });
    }
    Ok(())
}

pub fn validate_event_dependency_chain(
    observed_event_id: &str,
    dependencies: &[EventId],
    known_event_ids: &[EventId],
) -> ProtocolResult<()> {
    let has_event = |id: &str| known_event_ids.iter().any(|known| known == id);
    for dependency in dependencies {
        if dependency == observed_event_id {
            return Err(ProtocolError::Validation {
                reason: "event cannot depend on itself".to_string(),
            });
        }
        if !has_event(dependency) {
            return Err(ProtocolError::Validation {
                reason: format!("missing dependency event {dependency} in local ledger"),
            });
        }
    }
    Ok(())
}

pub fn validate_claim_lightcone(
    origin: &SpacetimeCoord,
    destination: &SpacetimeCoord,
    speed_ly_per_year: f64,
) -> ProtocolResult<LightconeStatus> {
    let proof = prove_lightcone("origin", "destination", &[origin.clone(), destination.clone()], speed_ly_per_year)?;
    if proof.status != LightconeStatus::Valid {
        return Err(ProtocolError::Validation {
            reason: "lightcone proof indicates claim path is impossible".to_string(),
        });
    }
    Ok(LightconeStatus::Valid)
}
