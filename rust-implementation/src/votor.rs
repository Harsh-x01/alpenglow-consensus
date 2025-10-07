//! Votor: Voting mechanism implementation
//!
//! Implements the dual-path concurrent voting strategy:
//! - Round 1: Notarization votes targeting 80% quorum (fast path)
//! - Round 2: Finalization votes targeting 60% quorum (fallback path)

use crate::types::*;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum VotorError {
    #[error("Double vote detected for validator {0}")]
    DoubleVote(ValidatorId),

    #[error("Invalid vote round")]
    InvalidRound,

    #[error("Validator {0} not in validator set")]
    UnknownValidator(ValidatorId),

    #[error("Block not found: {0}")]
    BlockNotFound(BlockId),
}

/// Votor state machine for managing votes and finalization
pub struct Votor {
    /// Current slot
    current_slot: Slot,

    /// Current round (1 or 2)
    current_round: VoteRound,

    /// Vote sets per block
    vote_sets: HashMap<BlockId, VoteSet>,

    /// Finalized blocks
    finalized: Vec<FinalizationCertificate>,

    /// Validator set with stakes
    validator_set: ValidatorSet,
}

impl Votor {
    pub fn new(validator_set: ValidatorSet) -> Self {
        Self {
            current_slot: Slot(0),
            current_round: VoteRound::Round1,
            vote_sets: HashMap::new(),
            finalized: Vec::new(),
            validator_set,
        }
    }

    /// Process a vote from a validator
    pub fn process_vote(&mut self, vote: Vote) -> Result<Option<FinalizationCertificate>, VotorError> {
        // Validate vote
        self.validate_vote(&vote)?;

        // Get or create vote set for this block
        let vote_set = self
            .vote_sets
            .entry(vote.block_id)
            .or_insert_with(|| VoteSet::new(vote.block_id));

        // Check for double voting
        match vote.round {
            VoteRound::Round1 => {
                if vote_set.round1_votes.contains_key(&vote.validator) {
                    return Err(VotorError::DoubleVote(vote.validator));
                }
            }
            VoteRound::Round2 => {
                if vote_set.round2_votes.contains_key(&vote.validator) {
                    return Err(VotorError::DoubleVote(vote.validator));
                }
            }
        }

        // Add vote
        vote_set.add_vote(vote.clone());

        // Check if we can finalize
        self.check_finalization(vote.block_id, vote.slot)
    }

    /// Check if a block can be finalized
    fn check_finalization(
        &mut self,
        block_id: BlockId,
        slot: Slot,
    ) -> Result<Option<FinalizationCertificate>, VotorError> {
        let vote_set = self
            .vote_sets
            .get(&block_id)
            .ok_or(VotorError::BlockNotFound(block_id))?;

        // Check fast path (80% in round 1)
        let round1_stake = self.calculate_vote_stake(&vote_set.round1_votes);
        if self.validator_set.check_fast_quorum(round1_stake) {
            let cert = self.create_certificate(
                block_id,
                slot,
                VoteRound::Round1,
                &vote_set.round1_votes,
                round1_stake,
            );
            self.finalized.push(cert.clone());
            return Ok(Some(cert));
        }

        // Check fallback path (60% in round 2)
        if matches!(self.current_round, VoteRound::Round2) {
            let round2_stake = self.calculate_vote_stake(&vote_set.round2_votes);
            if self.validator_set.check_fallback_quorum(round2_stake) {
                let cert = self.create_certificate(
                    block_id,
                    slot,
                    VoteRound::Round2,
                    &vote_set.round2_votes,
                    round2_stake,
                );
                self.finalized.push(cert.clone());
                return Ok(Some(cert));
            }
        }

        Ok(None)
    }

    /// Calculate total stake from a set of votes
    fn calculate_vote_stake(&self, votes: &HashMap<ValidatorId, Vote>) -> StakeWeight {
        votes
            .keys()
            .filter_map(|id| self.validator_set.get_validator(id))
            .map(|v| v.stake)
            .sum()
    }

    /// Create a finalization certificate
    fn create_certificate(
        &self,
        block_id: BlockId,
        slot: Slot,
        round: VoteRound,
        votes: &HashMap<ValidatorId, Vote>,
        total_stake: StakeWeight,
    ) -> FinalizationCertificate {
        FinalizationCertificate {
            block_id,
            slot,
            round,
            votes: votes.values().cloned().collect(),
            total_stake,
        }
    }

    /// Validate a vote
    fn validate_vote(&self, vote: &Vote) -> Result<(), VotorError> {
        // Check validator exists
        if self.validator_set.get_validator(&vote.validator).is_none() {
            return Err(VotorError::UnknownValidator(vote.validator));
        }

        // Check round is valid
        if vote.slot != self.current_slot {
            // Allow votes for current slot only (simplified)
        }

        Ok(())
    }

    /// Advance to round 2 (timeout on round 1)
    pub fn advance_to_round2(&mut self) {
        self.current_round = VoteRound::Round2;
    }

    /// Move to next slot
    pub fn next_slot(&mut self) {
        self.current_slot = self.current_slot.next();
        self.current_round = VoteRound::Round1;
        // Keep vote sets for finalization verification
    }

    /// Check if a block is finalized
    pub fn is_finalized(&self, block_id: &BlockId) -> bool {
        self.finalized.iter().any(|cert| cert.block_id == *block_id)
    }

    /// Get current slot
    pub fn current_slot(&self) -> Slot {
        self.current_slot
    }

    /// Get current round
    pub fn current_round(&self) -> VoteRound {
        self.current_round
    }

    /// Get finalized blocks
    pub fn finalized_blocks(&self) -> &[FinalizationCertificate] {
        &self.finalized
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

    #[test]
    fn test_fast_path_finalization() {
        let vset = create_test_validator_set(5);
        let mut votor = Votor::new(vset);

        let block_id = BlockId::new([1u8; 32]);
        let slot = Slot(0);

        // Cast 4 out of 5 votes (80%)
        for i in 0..4 {
            let vote = Vote {
                validator: ValidatorId(i),
                block_id,
                slot,
                round: VoteRound::Round1,
                signature: vec![],
            };

            let result = votor.process_vote(vote);
            if i == 3 {
                // 4th vote should trigger finalization
                assert!(result.is_ok());
                assert!(result.unwrap().is_some());
            }
        }

        assert!(votor.is_finalized(&block_id));
    }

    #[test]
    fn test_fallback_path_finalization() {
        let vset = create_test_validator_set(5);
        let mut votor = Votor::new(vset);

        let block_id = BlockId::new([1u8; 32]);
        let slot = Slot(0);

        // Cast only 3 votes in round 1 (60%, not enough for fast path)
        for i in 0..3 {
            let vote = Vote {
                validator: ValidatorId(i),
                block_id,
                slot,
                round: VoteRound::Round1,
                signature: vec![],
            };
            let result = votor.process_vote(vote);
            assert!(result.unwrap().is_none()); // No finalization yet
        }

        // Advance to round 2
        votor.advance_to_round2();

        // Cast 3 votes in round 2 (60%, enough for fallback)
        for i in 0..3 {
            let vote = Vote {
                validator: ValidatorId(i),
                block_id,
                slot,
                round: VoteRound::Round2,
                signature: vec![],
            };
            let result = votor.process_vote(vote);
            if i == 2 {
                assert!(result.is_ok());
                assert!(result.unwrap().is_some());
            }
        }

        assert!(votor.is_finalized(&block_id));
    }

    #[test]
    fn test_double_vote_detection() {
        let vset = create_test_validator_set(3);
        let mut votor = Votor::new(vset);

        let block_id = BlockId::new([1u8; 32]);
        let slot = Slot(0);

        let vote1 = Vote {
            validator: ValidatorId(0),
            block_id,
            slot,
            round: VoteRound::Round1,
            signature: vec![],
        };

        // First vote should succeed
        assert!(votor.process_vote(vote1.clone()).is_ok());

        // Second vote from same validator should fail
        let result = votor.process_vote(vote1);
        assert!(matches!(result, Err(VotorError::DoubleVote(_))));
    }
}