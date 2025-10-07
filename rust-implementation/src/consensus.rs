//! Main consensus engine integrating Votor and Rotor

use crate::rotor::{Rotor, Shred};
use crate::types::*;
use crate::votor::Votor;
use std::time::{Duration, Instant};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConsensusError {
    #[error("Votor error: {0}")]
    VotorError(#[from] crate::votor::VotorError),

    #[error("Rotor error: {0}")]
    RotorError(#[from] crate::rotor::RotorError),

    #[error("Not the leader for slot {0}")]
    NotLeader(Slot),

    #[error("Invalid slot: expected {expected}, got {got}")]
    InvalidSlot { expected: Slot, got: Slot },
}

/// Main consensus engine state
pub struct ConsensusEngine {
    /// Our validator ID
    validator_id: ValidatorId,

    /// Validator set
    validator_set: ValidatorSet,

    /// Votor for vote management
    votor: Votor,

    /// Rotor for block propagation
    rotor: Rotor,

    /// Current leader
    current_leader: ValidatorId,

    /// Round 1 start time
    round1_start: Option<Instant>,

    /// Configuration
    config: ConsensusConfig,
}

#[derive(Debug, Clone)]
pub struct ConsensusConfig {
    pub round1_timeout: Duration,
    pub round2_timeout: Duration,
}

impl Default for ConsensusConfig {
    fn default() -> Self {
        Self {
            round1_timeout: Duration::from_millis(crate::ROUND1_TIMEOUT_MS),
            round2_timeout: Duration::from_millis(crate::ROUND2_TIMEOUT_MS),
        }
    }
}

impl ConsensusEngine {
    pub fn new(
        validator_id: ValidatorId,
        validator_set: ValidatorSet,
        config: ConsensusConfig,
    ) -> Self {
        let votor = Votor::new(validator_set.clone());
        let rotor = Rotor::new(validator_set.clone());

        // Determine initial leader (simplified: validator 0)
        let current_leader = ValidatorId(0);

        Self {
            validator_id,
            validator_set,
            votor,
            rotor,
            current_leader,
            round1_start: None,
            config,
        }
    }

    /// Start a new slot as leader
    pub fn propose_block(&mut self, block: Block) -> Result<Vec<Shred>, ConsensusError> {
        if self.current_leader != self.validator_id {
            return Err(ConsensusError::NotLeader(block.slot));
        }

        if block.slot != self.votor.current_slot() {
            return Err(ConsensusError::InvalidSlot {
                expected: self.votor.current_slot(),
                got: block.slot,
            });
        }

        // Encode block into shreds
        let shreds = self.rotor.encode_block(&block)?;

        // Start round 1 timer
        self.round1_start = Some(Instant::now());

        // In a real implementation, broadcast shreds to relays
        // For now, just return them for manual distribution

        Ok(shreds)
    }

    /// Receive a shred from the network
    pub fn receive_shred(&mut self, shred: Shred) -> Result<(), ConsensusError> {
        // Try to reconstruct block
        if let Some(block) = self.rotor.receive_shred(shred)? {
            // Block reconstructed, cast our vote if we're honest
            self.vote_for_block(block)?;
        }

        Ok(())
    }

    /// Cast a vote for a block
    fn vote_for_block(&mut self, block: Block) -> Result<(), ConsensusError> {
        // Don't vote if we're Byzantine or offline
        if let Some(config) = self.validator_set.get_validator(&self.validator_id) {
            if config.is_byzantine || config.is_offline {
                return Ok(());
            }
        }

        let vote = Vote {
            validator: self.validator_id,
            block_id: block.id,
            slot: block.slot,
            round: self.votor.current_round(),
            signature: vec![], // Simplified: no actual signature
        };

        // Process our own vote
        self.process_vote(vote)?;

        Ok(())
    }

    /// Process a vote from any validator
    pub fn process_vote(&mut self, vote: Vote) -> Result<Option<FinalizationCertificate>, ConsensusError> {
        let cert = self.votor.process_vote(vote)?;

        if let Some(ref certificate) = cert {
            tracing::info!(
                "Block {} finalized in slot {} via {:?}",
                certificate.block_id,
                certificate.slot,
                certificate.round
            );
        }

        Ok(cert)
    }

    /// Check if round 1 timeout has expired
    pub fn check_round1_timeout(&mut self) -> bool {
        if let Some(start) = self.round1_start {
            if start.elapsed() >= self.config.round1_timeout {
                self.advance_to_round2();
                return true;
            }
        }
        false
    }

    /// Advance to round 2
    fn advance_to_round2(&mut self) {
        tracing::info!("Advancing to round 2 for slot {}", self.votor.current_slot());
        self.votor.advance_to_round2();
    }

    /// Move to the next slot
    pub fn next_slot(&mut self) {
        self.votor.next_slot();
        self.round1_start = None;

        // Rotate leader (simplified: round-robin)
        let next_leader_idx = (self.current_leader.0 + 1) % self.validator_set.len() as u64;
        self.current_leader = ValidatorId(next_leader_idx);

        tracing::info!(
            "Advanced to slot {}, leader is {}",
            self.votor.current_slot(),
            self.current_leader
        );
    }

    /// Check if we are the current leader
    pub fn is_leader(&self) -> bool {
        self.current_leader == self.validator_id
    }

    /// Get current slot
    pub fn current_slot(&self) -> Slot {
        self.votor.current_slot()
    }

    /// Get finalized blocks
    pub fn finalized_blocks(&self) -> &[FinalizationCertificate] {
        self.votor.finalized_blocks()
    }

    /// Check if a block is finalized
    pub fn is_finalized(&self, block_id: &BlockId) -> bool {
        self.votor.is_finalized(block_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_validator_set(count: usize) -> ValidatorSet {
        let mut vset = ValidatorSet::new();
        for i in 0..count {
            vset.add_validator(ValidatorConfig {
                id: ValidatorId(i as u64),
                stake: StakeWeight(100),
                is_byzantine: false,
                is_offline: false,
            });
        }
        vset
    }

    fn create_test_block(slot: u64, leader: ValidatorId) -> Block {
        let mut block = Block {
            id: BlockId::new([0u8; 32]),
            slot: Slot(slot),
            parent: None,
            leader,
            transactions: vec![],
            timestamp: 1000 + slot,
        };
        block.id = block.compute_id();
        block
    }

    #[test]
    fn test_consensus_engine_creation() {
        let vset = create_test_validator_set(5);
        let config = ConsensusConfig::default();
        let engine = ConsensusEngine::new(ValidatorId(0), vset, config);

        assert_eq!(engine.current_slot(), Slot(0));
        assert!(engine.is_leader());
    }

    #[test]
    fn test_block_proposal_and_finalization() {
        let vset = create_test_validator_set(5);
        let config = ConsensusConfig::default();

        // Create engines for all validators
        let mut engines: Vec<_> = (0..5)
            .map(|i| ConsensusEngine::new(ValidatorId(i), vset.clone(), config.clone()))
            .collect();

        // Leader (validator 0) proposes a block
        let block = create_test_block(0, ValidatorId(0));
        let shreds = engines[0].propose_block(block.clone()).unwrap();

        // Distribute shreds to all validators and collect votes
        let mut votes = Vec::new();
        for (i, engine) in engines.iter_mut().enumerate() {
            for shred in shreds.clone() {
                engine.receive_shred(shred).ok();
            }
            // Create vote from this validator
            votes.push(Vote {
                validator: ValidatorId(i as u64),
                block_id: block.id,
                slot: block.slot,
                round: VoteRound::Round1,
                signature: vec![],
            });
        }

        // Process all votes in all engines
        for engine in &mut engines {
            for vote in votes.clone() {
                engine.process_vote(vote).ok();
            }
        }

        // Check if block is finalized (should be with 5/5 = 100% > 80%)
        for engine in &engines {
            assert!(engine.is_finalized(&block.id));
        }
    }
}