//! Core data types for Alpenglow consensus

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt;

/// Unique identifier for a validator
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ValidatorId(pub u64);

impl fmt::Display for ValidatorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "V{}", self.0)
    }
}

/// Stake weight for a validator
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct StakeWeight(pub u64);

impl StakeWeight {
    pub fn new(weight: u64) -> Self {
        Self(weight)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl std::ops::Add for StakeWeight {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self(self.0 + other.0)
    }
}

impl std::ops::AddAssign for StakeWeight {
    fn add_assign(&mut self, other: Self) {
        self.0 += other.0;
    }
}

impl std::iter::Sum for StakeWeight {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(StakeWeight(0), |a, b| a + b)
    }
}

/// Slot number (height in the chain)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Slot(pub u64);

impl fmt::Display for Slot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Slot{}", self.0)
    }
}

impl Slot {
    pub fn next(&self) -> Self {
        Slot(self.0 + 1)
    }
}

/// Block identifier (hash)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BlockId([u8; 32]);

impl BlockId {
    pub fn new(hash: [u8; 32]) -> Self {
        Self(hash)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for BlockId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Block<{:x}{:x}{:x}{:x}>",
               self.0[0], self.0[1], self.0[2], self.0[3])
    }
}

/// Block proposal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub id: BlockId,
    pub slot: Slot,
    pub parent: Option<BlockId>,
    pub leader: ValidatorId,
    pub transactions: Vec<Vec<u8>>,  // Simplified transaction data
    pub timestamp: u64,
}

impl Block {
    pub fn compute_id(&self) -> BlockId {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(&bincode::serialize(&self.slot).unwrap());
        hasher.update(&bincode::serialize(&self.parent).unwrap());
        hasher.update(&bincode::serialize(&self.leader).unwrap());
        hasher.update(&bincode::serialize(&self.timestamp).unwrap());
        let result = hasher.finalize();
        let mut id = [0u8; 32];
        id.copy_from_slice(&result);
        BlockId(id)
    }
}

/// Voting round
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VoteRound {
    Round1,  // Notarization vote (fast path)
    Round2,  // Finalization vote (fallback path)
}

/// Vote on a block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vote {
    pub validator: ValidatorId,
    pub block_id: BlockId,
    pub slot: Slot,
    pub round: VoteRound,
    pub signature: Vec<u8>,  // Simplified signature
}

/// Vote collection for a specific block
#[derive(Debug, Clone)]
pub struct VoteSet {
    pub block_id: BlockId,
    pub round1_votes: HashMap<ValidatorId, Vote>,
    pub round2_votes: HashMap<ValidatorId, Vote>,
}

impl VoteSet {
    pub fn new(block_id: BlockId) -> Self {
        Self {
            block_id,
            round1_votes: HashMap::new(),
            round2_votes: HashMap::new(),
        }
    }

    pub fn add_vote(&mut self, vote: Vote) {
        match vote.round {
            VoteRound::Round1 => {
                self.round1_votes.insert(vote.validator, vote);
            }
            VoteRound::Round2 => {
                self.round2_votes.insert(vote.validator, vote);
            }
        }
    }

    pub fn round1_count(&self) -> usize {
        self.round1_votes.len()
    }

    pub fn round2_count(&self) -> usize {
        self.round2_votes.len()
    }
}

/// Finalized block certificate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinalizationCertificate {
    pub block_id: BlockId,
    pub slot: Slot,
    pub round: VoteRound,
    pub votes: Vec<Vote>,
    pub total_stake: StakeWeight,
}

/// Validator configuration
#[derive(Debug, Clone)]
pub struct ValidatorConfig {
    pub id: ValidatorId,
    pub stake: StakeWeight,
    pub is_byzantine: bool,
    pub is_offline: bool,
}

/// Network of validators with stake distribution
#[derive(Debug, Clone)]
pub struct ValidatorSet {
    validators: HashMap<ValidatorId, ValidatorConfig>,
    total_stake: StakeWeight,
}

impl ValidatorSet {
    pub fn new() -> Self {
        Self {
            validators: HashMap::new(),
            total_stake: StakeWeight(0),
        }
    }

    pub fn add_validator(&mut self, config: ValidatorConfig) {
        self.total_stake += config.stake;
        self.validators.insert(config.id, config);
    }

    pub fn get_validator(&self, id: &ValidatorId) -> Option<&ValidatorConfig> {
        self.validators.get(id)
    }

    pub fn total_stake(&self) -> StakeWeight {
        self.total_stake
    }

    pub fn honest_validators(&self) -> impl Iterator<Item = &ValidatorConfig> {
        self.validators
            .values()
            .filter(|v| !v.is_byzantine && !v.is_offline)
    }

    pub fn calculate_stake(&self, validator_ids: &HashSet<ValidatorId>) -> StakeWeight {
        validator_ids
            .iter()
            .filter_map(|id| self.validators.get(id))
            .map(|v| v.stake)
            .sum()
    }

    pub fn check_fast_quorum(&self, stake: StakeWeight) -> bool {
        let threshold = (self.total_stake.0 * 80) / 100;
        stake.0 >= threshold
    }

    pub fn check_fallback_quorum(&self, stake: StakeWeight) -> bool {
        let threshold = (self.total_stake.0 * 60) / 100;
        stake.0 >= threshold
    }

    pub fn len(&self) -> usize {
        self.validators.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validator_set() {
        let mut vset = ValidatorSet::new();
        vset.add_validator(ValidatorConfig {
            id: ValidatorId(1),
            stake: StakeWeight(100),
            is_byzantine: false,
            is_offline: false,
        });
        vset.add_validator(ValidatorConfig {
            id: ValidatorId(2),
            stake: StakeWeight(100),
            is_byzantine: false,
            is_offline: false,
        });
        vset.add_validator(ValidatorConfig {
            id: ValidatorId(3),
            stake: StakeWeight(100),
            is_byzantine: false,
            is_offline: false,
        });

        assert_eq!(vset.total_stake(), StakeWeight(300));
        assert!(vset.check_fast_quorum(StakeWeight(240)));
        assert!(!vset.check_fast_quorum(StakeWeight(239)));
        assert!(vset.check_fallback_quorum(StakeWeight(180)));
        assert!(!vset.check_fallback_quorum(StakeWeight(179)));
    }

    #[test]
    fn test_vote_set() {
        let block_id = BlockId::new([1u8; 32]);
        let mut vote_set = VoteSet::new(block_id);

        let vote1 = Vote {
            validator: ValidatorId(1),
            block_id,
            slot: Slot(0),
            round: VoteRound::Round1,
            signature: vec![],
        };

        vote_set.add_vote(vote1);
        assert_eq!(vote_set.round1_count(), 1);
        assert_eq!(vote_set.round2_count(), 0);
    }
}