use std::collections::HashMap;

pub const HASH_LEN: usize = 32;
pub const CURRENT_PROTOCOL_VERSION: u32 = 1;
pub const MAX_ROUTING_HOPS: usize = 64;

pub type Hash = [u8; HASH_LEN];
pub type DomainId = String;
pub type NodeId = String;
pub type EventId = String;
pub type MessageId = String;
pub type CheckpointId = String;
pub type ClaimId = String;
pub type TxId = String;
pub type SpacetimeTime = i64;

#[derive(Debug, Clone)]
pub struct TimeInterval {
    pub t_min: SpacetimeTime,
    pub t_max: SpacetimeTime,
}

#[derive(Debug, Clone)]
pub struct Region3D {
    pub center: [f64; 3],
    pub radius_ly: f64,
}

impl Region3D {
    pub fn is_valid(&self) -> bool {
        self.radius_ly.is_finite() && self.radius_ly >= 0.0
    }

    pub fn min_distance_ly(&self, other: &Region3D) -> f64 {
        let dx = self.center[0] - other.center[0];
        let dy = self.center[1] - other.center[1];
        let dz = self.center[2] - other.center[2];
        let dist = (dx * dx + dy * dy + dz * dz).sqrt();
        (dist - self.radius_ly - other.radius_ly).max(0.0)
    }
}

#[derive(Debug, Clone)]
pub struct SpacetimeCoord {
    pub frame_id: String,
    pub time_interval: TimeInterval,
    pub position_region: Region3D,
    pub uncertainty: f64,
    pub attestation: Vec<String>,
}

impl SpacetimeCoord {
    pub fn is_valid(&self) -> bool {
        self.time_interval.is_valid()
            && self.position_region.is_valid()
            && self.uncertainty.is_finite()
            && self.uncertainty >= 0.0
    }

    pub fn encode_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(self.frame_id.len() as u64).to_le_bytes());
        out.extend_from_slice(self.frame_id.as_bytes());
        out.extend_from_slice(&self.time_interval.t_min.to_le_bytes());
        out.extend_from_slice(&self.time_interval.t_max.to_le_bytes());
        for axis in self.position_region.center.iter() {
            out.extend_from_slice(&axis.to_le_bytes());
        }
        out.extend_from_slice(&self.position_region.radius_ly.to_le_bytes());
        out.extend_from_slice(&self.uncertainty.to_le_bytes());
        out.extend_from_slice(&(self.attestation.len() as u64).to_le_bytes());
        for att in &self.attestation {
            out.extend_from_slice(&(att.len() as u64).to_le_bytes());
            out.extend_from_slice(att.as_bytes());
        }
        out
    }

    pub fn has_sane_time(&self) -> bool {
        self.time_interval.t_max >= self.time_interval.t_min
    }
}

#[derive(Debug, Clone)]
pub struct Event {
    pub event_id: EventId,
    pub actor_id: NodeId,
    pub domain_id: DomainId,
    pub kind: String,
    pub payload_hash: Hash,
    pub coord: SpacetimeCoord,
    pub local_sequence: u64,
    pub causal_dependencies: Vec<EventId>,
    pub signatures: Vec<String>,
}

impl Event {
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(self.event_id.len() as u64).to_le_bytes());
        out.extend_from_slice(self.event_id.as_bytes());
        out.extend_from_slice(&(self.actor_id.len() as u64).to_le_bytes());
        out.extend_from_slice(self.actor_id.as_bytes());
        out.extend_from_slice(&(self.domain_id.len() as u64).to_le_bytes());
        out.extend_from_slice(self.domain_id.as_bytes());
        out.extend_from_slice(&(self.kind.len() as u64).to_le_bytes());
        out.extend_from_slice(self.kind.as_bytes());
        out.extend_from_slice(&self.payload_hash);
        out.extend_from_slice(&self.coord.encode_bytes());
        out.extend_from_slice(&self.local_sequence.to_le_bytes());
        out.extend_from_slice(&(self.causal_dependencies.len() as u64).to_le_bytes());
        for dependency in &self.causal_dependencies {
            out.extend_from_slice(&(dependency.len() as u64).to_le_bytes());
            out.extend_from_slice(dependency.as_bytes());
        }
        out.extend_from_slice(&(self.signatures.len() as u64).to_le_bytes());
        for signature in &self.signatures {
            out.extend_from_slice(&(signature.len() as u64).to_le_bytes());
            out.extend_from_slice(signature.as_bytes());
        }
        out
    }
}

#[derive(Debug, Clone)]
pub struct Message {
    pub msg_id: MessageId,
    pub from_event: EventId,
    pub to_event: EventId,
    pub payload_hash: Hash,
    pub route: Vec<NodeId>,
    pub send_coord: SpacetimeCoord,
    pub receive_coord: SpacetimeCoord,
    pub relay_signatures: Vec<String>,
    pub anti_replay_nonce: String,
}

impl Message {
    pub fn route_hop_count(&self) -> usize {
        self.route.len()
    }

    pub fn satisfies_min_route(&self, min_hops: usize) -> bool {
        self.route.len() >= min_hops && self.route.len() <= MAX_ROUTING_HOPS
    }
}

#[derive(Debug, Clone)]
pub struct Observation {
    pub observer_id: NodeId,
    pub observed_hash: Hash,
    pub observed_kind: String,
    pub receive_event: EventId,
    pub receive_coord: SpacetimeCoord,
    pub source_route: Vec<NodeId>,
    pub signature: String,
}

#[derive(Debug, Clone)]
pub enum TxKind {
    Transfer,
    LockForExport,
    AcceptRemoteClaim,
    Receipt,
}

#[derive(Debug, Clone)]
pub struct Transaction {
    pub tx_id: TxId,
    pub domain_id: DomainId,
    pub kind: TxKind,
    pub actor_id: NodeId,
    pub payload_hash: Hash,
    pub coord: SpacetimeCoord,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub causal_dependencies: Vec<EventId>,
    pub signatures: Vec<String>,
}

impl Transaction {
    pub fn has_inputs_or_outputs(&self) -> bool {
        !self.inputs.is_empty() || !self.outputs.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FinalityStage {
    LocalFinal,
    RemoteObserved,
    ProvisionallyCredited,
    AcceptedByRemoteLedger,
    OriginAcknowledged,
    BilaterallySettled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CryptoStatus {
    CryptoCurrent,
    CryptoDeprecatedButRenewed,
    CryptoStaleNeedsRenewal,
    CryptoBrokenUntrusted,
    CryptoUnknownSuite,
    PhysicalAnchorOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LightconeStatus {
    Valid,
    Invalid,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RiskLabel {
    Low,
    Medium,
    High,
    DoNotAcceptForCredit,
    DoNotAcceptAtAll,
}

#[derive(Debug, Clone)]
pub struct SettlementClaim {
    pub claim_id: ClaimId,
    pub lock_event_id: EventId,
    pub origin_domain: DomainId,
    pub remote_domain: DomainId,
    pub asset_id: String,
    pub amount: u64,
    pub lock_checkpoint: CheckpointId,
    pub lock_coord: SpacetimeCoord,
    pub dependencies: Vec<EventId>,
    pub observations: Vec<Observation>,
    pub dependencies_causal: Vec<String>,
    pub signature_bundle: Vec<String>,
    pub finality: FinalityStage,
    pub crypto_status: CryptoStatus,
    pub lightcone_status: LightconeStatus,
    pub risk_label: RiskLabel,
    pub settlement_horizon_years: u64,
}

#[derive(Debug, Clone)]
pub struct CreditLine {
    pub credit_line_id: String,
    pub from_domain: DomainId,
    pub to_domain: DomainId,
    pub limit: u64,
    pub used: u64,
    pub haircut: f64,
    pub risk_label: RiskLabel,
}

#[derive(Debug, Clone)]
pub struct DisputeRecord {
    pub dispute_id: String,
    pub claim_id: ClaimId,
    pub kind: String,
    pub evidence: HashMap<String, String>,
    pub created_at: SpacetimeTime,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct ClientSyncState {
    pub domain: DomainId,
    pub last_trusted_checkpoint: Option<CheckpointId>,
    pub frontier: Vec<CheckpointId>,
    pub sync_watermark: SpacetimeTime,
}

#[derive(Debug, Clone)]
pub struct SettlementPolicy {
    pub challenge_window: SpacetimeTime,
    pub max_settlement_horizon_years: u64,
    pub lightcone_speed_ly_per_year: f64,
    pub min_route_hops: usize,
    pub min_anti_replay_nonce_chars: usize,
    pub auto_settlement_risk_floor: RiskLabel,
}

impl Default for SettlementPolicy {
    fn default() -> Self {
        Self {
            challenge_window: 300,
            max_settlement_horizon_years: 10,
            lightcone_speed_ly_per_year: 1.0,
            min_route_hops: 1,
            min_anti_replay_nonce_chars: 8,
            auto_settlement_risk_floor: RiskLabel::Medium,
        }
    }
}

impl TimeInterval {
    pub fn is_valid(&self) -> bool {
        self.t_min <= self.t_max
    }
}

pub fn encode_u64(value: u64) -> [u8; 8] {
    value.to_le_bytes()
}

pub fn hash_chunks(chunks: &[&[u8]]) -> Hash {
    fn mix(mut state: u64, bytes: &[u8]) -> u64 {
        for byte in bytes {
            state ^= u64::from(*byte);
            state = state.wrapping_mul(0x100000001b3);
        }
        state
    }

    let mut state_a: u64 = 0xcbf29ce484222325;
    let mut state_b: u64 = 0x84222325cafe0001;

    for (idx, chunk) in chunks.iter().enumerate() {
        let idx_hash = encode_u64(idx as u64);
        state_a = mix(state_a, &idx_hash);
        state_b = mix(state_b, &idx_hash);
        state_a = mix(state_a, chunk);
        state_b = mix(state_b, chunk);
    }

    let state_c = mix(state_a, &state_b.to_le_bytes());
    let state_d = mix(state_b, &state_a.to_le_bytes());

    let mut out = [0u8; HASH_LEN];
    out[0..8].copy_from_slice(&state_a.to_le_bytes());
    out[8..16].copy_from_slice(&state_b.to_le_bytes());
    out[16..24].copy_from_slice(&state_c.to_le_bytes());
    out[24..32].copy_from_slice(&state_d.to_le_bytes());
    out
}

pub fn hash_to_hex(hash: &Hash) -> String {
    hash.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn hex_half_to_u8(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

pub fn hash_hex_to_bytes(hex: &str) -> Result<Hash, String> {
    if hex.len() != HASH_LEN * 2 {
        return Err(format!("hash must be {} hex chars", HASH_LEN * 2));
    }

    let mut out = [0u8; HASH_LEN];
    let bytes = hex.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let hi = hex_half_to_u8(bytes[i]).ok_or_else(|| "invalid hex character".to_string())?;
        let lo = hex_half_to_u8(bytes[i + 1]).ok_or_else(|| "invalid hex character".to_string())?;
        out[i / 2] = (hi << 4) | lo;
        i += 2;
    }
    Ok(out)
}



