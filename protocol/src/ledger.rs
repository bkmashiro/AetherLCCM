use std::collections::VecDeque;

use crate::errors::{ProtocolError, ProtocolResult};
use crate::checkpoint::Checkpoint;
use crate::types::{
    hash_chunks, hash_to_hex, CURRENT_PROTOCOL_VERSION, DomainId, Event, EventId, Hash,
    SpacetimeCoord, Transaction, TxId,
};

#[derive(Debug, Clone)]
pub struct LedgerEventBatch {
    pub height: u64,
    pub events: Vec<Event>,
}

#[derive(Debug, Clone)]
pub struct LocalLedgerDomain {
    pub domain_id: DomainId,
    pub chain_height: u64,
    pub final_checkpoint: Option<Checkpoint>,
    pub event_log: VecDeque<Event>,
    pub checkpoint_history: Vec<Checkpoint>,
}

impl LocalLedgerDomain {
    pub fn new(domain_id: DomainId) -> Self {
        Self {
            domain_id,
            chain_height: 0,
            final_checkpoint: None,
            event_log: VecDeque::new(),
            checkpoint_history: Vec::new(),
        }
    }

    pub fn submit_local_tx(
        &mut self,
        tx_id: TxId,
        tx: &Transaction,
    ) -> ProtocolResult<Event> {
        if tx.domain_id != self.domain_id {
            return Err(ProtocolError::Validation {
                reason: "transaction domain does not match local ledger domain".to_string(),
            });
        }
        if !tx.coord.is_valid() {
            return Err(ProtocolError::Validation {
                reason: "invalid transaction spacetime coordinate".to_string(),
            });
        }
        let event_id = format!("ev:{}:{}", tx.domain_id, tx_id);
        if self.event_log.iter().any(|evt| evt.event_id == event_id) {
            return Err(ProtocolError::Conflict {
                reason: "event id already exists in domain ledger".to_string(),
            });
        }

        let event = Event {
            event_id,
            actor_id: tx.actor_id.clone(),
            domain_id: tx.domain_id.clone(),
            kind: format!("{:?}", tx.kind),
            payload_hash: tx.payload_hash,
            coord: tx.coord.clone(),
            local_sequence: self.event_log.len() as u64 + 1,
            causal_dependencies: tx.causal_dependencies.clone(),
            signatures: tx.signatures.clone(),
        };
        self.event_log.push_back(event.clone());
        self.chain_height += 1;

        Ok(event)
    }

    pub fn has_event(&self, event_id: &EventId) -> bool {
        self.event_log.iter().any(|evt| evt.event_id == *event_id)
    }

    pub fn get_event(&self, event_id: &EventId) -> Option<&Event> {
        self.event_log.iter().find(|evt| &evt.event_id == event_id)
    }

    pub fn event_log_root(&self) -> Hash {
        let blobs: Vec<Vec<u8>> = self.event_log.iter().map(|evt| evt.canonical_bytes()).collect();
        let refs: Vec<&[u8]> = blobs.iter().map(Vec::as_slice).collect();
        hash_chunks(&refs)
    }

    pub fn build_checkpoint_root(
        &self,
        tx_root: &Hash,
        event_root: &Hash,
        coord: &SpacetimeCoord,
        prev_checkpoint_hash: Option<&Hash>,
        epoch: u64,
    ) -> Hash {
        let mut storage: Vec<Vec<u8>> = vec![
            self.domain_id.as_bytes().to_vec(),
            epoch.to_le_bytes().to_vec(),
            self.chain_height.to_le_bytes().to_vec(),
            (self.event_log.len() as u64).to_le_bytes().to_vec(),
            tx_root.to_vec(),
            event_root.to_vec(),
        ];
        if let Some(prev) = prev_checkpoint_hash {
            storage.push(prev.to_vec());
        } else {
            storage.push([0u8; 32].to_vec());
        }
        storage.push(coord.encode_bytes());
        let chunk_refs: Vec<&[u8]> = storage.iter().map(Vec::as_slice).collect();
        hash_chunks(&chunk_refs)
    }

    pub fn finalize_checkpoint(
        &mut self,
        checkpoint_id: String,
        tx_root: Hash,
        coord: SpacetimeCoord,
    ) -> ProtocolResult<Checkpoint> {
        if self.domain_id.is_empty() {
            return Err(ProtocolError::Validation {
                reason: "domain id must not be empty".to_string(),
            });
        }
        if coord.is_valid() == false {
            return Err(ProtocolError::Validation {
                reason: "checkpoint coordinate is invalid".to_string(),
            });
        }
        if let Some(final_cp) = &self.final_checkpoint {
            if self.chain_height <= final_cp.height {
                return Err(ProtocolError::Conflict {
                    reason: "checkpoint height must increase over prior finalized checkpoint".to_string(),
                });
            }
        }

        let tx_root = if tx_root == [0u8; 32] {
            self.event_log_root()
        } else {
            tx_root
        };
        let event_log_root = self.event_log_root();
        let prev_checkpoint_hash = self.final_checkpoint.as_ref().map(|cp| cp.state_root);
        let checkpoint_root = self.build_checkpoint_root(
            &tx_root,
            &event_log_root,
            &coord,
            prev_checkpoint_hash.as_ref(),
            CURRENT_PROTOCOL_VERSION as u64,
        );
        let observed_id = hash_to_hex(&checkpoint_root);
        if !checkpoint_id.is_empty() && checkpoint_id != observed_id {
            return Err(ProtocolError::Validation {
                reason: "provided checkpoint id does not match local checkpoint commitment".to_string(),
            });
        }

        let checkpoint = Checkpoint {
            domain_id: self.domain_id.clone(),
            height: self.chain_height,
            epoch: CURRENT_PROTOCOL_VERSION as u64,
            prev_checkpoint_hash,
            state_root: checkpoint_root,
            tx_root,
            event_log_root,
            export_root: event_log_root,
            import_root: tx_root,
            observed_remote_root: event_log_root,
            dispute_root: checkpoint_root,
            validator_set_root: tx_root,
            coord,
            protocol_version: CURRENT_PROTOCOL_VERSION,
            state_commitment: checkpoint_root,
            hash: observed_id,
        };
        self.final_checkpoint = Some(checkpoint.clone());
        self.checkpoint_history.push(checkpoint.clone());
        Ok(checkpoint)
    }

    pub fn finalize_checkpoint_auto(
        &mut self,
        tx_root: Hash,
        coord: SpacetimeCoord,
    ) -> ProtocolResult<Checkpoint> {
        self.finalize_checkpoint(String::new(), tx_root, coord)
    }

    pub fn last_event_id(&self) -> ProtocolResult<Option<EventId>> {
        Ok(self.event_log.back().map(|e| e.event_id.clone()))
    }
}
