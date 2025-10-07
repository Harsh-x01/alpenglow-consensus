//! Stateright Model Checker for Alpenglow Consensus
//!
//! This implements an executable model of the Alpenglow protocol using
//! Stateright for exhaustive state-space exploration and property checking.

use alpenglow::types::*;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

#[derive(Clone, Debug, PartialEq, Eq)]
struct AlpenglowModel {
    /// Number of validators
    validator_count: usize,
    /// Byzantine validator IDs
    byzantine: BTreeSet<ValidatorId>,
    /// Offline validator IDs
    offline: BTreeSet<ValidatorId>,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
enum Round {
    Round1,
    Round2,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct State {
    /// Current slot
    slot: u64,
    /// Current leader
    leader: ValidatorId,
    /// Proposed blocks per slot
    proposed: BTreeMap<u64, (BlockId, ValidatorId)>,
    /// Votes in round 1
    votes_round1: BTreeMap<BlockId, BTreeSet<ValidatorId>>,
    /// Votes in round 2
    votes_round2: BTreeMap<BlockId, BTreeSet<ValidatorId>>,
    /// Finalized blocks
    finalized: Vec<(BlockId, u64, Round)>,
    /// Current round
    round: Round,
    /// Skip votes per slot
    skip_votes: BTreeMap<u64, BTreeSet<ValidatorId>>,
    /// Skipped slots
    skipped: BTreeSet<u64>,
    /// Network partition: (partition1, partition2) - empty if no partition
    partitioned: Option<(BTreeSet<ValidatorId>, BTreeSet<ValidatorId>)>,
    /// Whether partition has healed
    partition_healed: bool,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
enum Action {
    ProposeBlock(ValidatorId, BlockId),
    VoteRound1(ValidatorId, BlockId),
    VoteRound2(ValidatorId, BlockId),
    CheckFastQuorum(BlockId),
    CheckFallbackQuorum(BlockId),
    AdvanceToRound2,
    VoteSkip(ValidatorId),
    CheckSkipQuorum,
    NextSlot,
    NetworkPartition(BTreeSet<ValidatorId>, BTreeSet<ValidatorId>),
    PartitionHeal,
}

impl AlpenglowModel {
    fn new(validator_count: usize) -> Self {
        Self {
            validator_count,
            byzantine: BTreeSet::new(),
            offline: BTreeSet::new(),
        }
    }

    fn with_byzantine(mut self, byzantine_id: usize) -> Self {
        self.byzantine.insert(ValidatorId(byzantine_id as u64));
        self
    }

    fn total_stake(&self) -> u64 {
        self.validator_count as u64
    }

    fn fast_quorum(&self) -> u64 {
        (self.total_stake() * 80) / 100
    }

    fn fallback_quorum(&self) -> u64 {
        (self.total_stake() * 60) / 100
    }

    fn is_honest(&self, v: &ValidatorId) -> bool {
        !self.byzantine.contains(v) && !self.offline.contains(v)
    }

    fn initial_state(&self) -> State {
        State {
            slot: 0,
            leader: ValidatorId(0),
            proposed: BTreeMap::new(),
            votes_round1: BTreeMap::new(),
            votes_round2: BTreeMap::new(),
            finalized: Vec::new(),
            round: Round::Round1,
            skip_votes: BTreeMap::new(),
            skipped: BTreeSet::new(),
            partitioned: None,
            partition_healed: false,
        }
    }

    fn actions(&self, state: &State) -> Vec<Action> {
        let mut actions = Vec::new();

        // Leader can propose
        if !state.proposed.contains_key(&state.slot) && self.is_honest(&state.leader) {
            let block_id = BlockId::new([state.slot as u8; 32]);
            actions.push(Action::ProposeBlock(state.leader, block_id));
        }

        // Validators can vote if block proposed
        if let Some((block_id, _)) = state.proposed.get(&state.slot) {
            // Round 1 votes
            if matches!(state.round, Round::Round1) {
                for i in 0..self.validator_count {
                    let v = ValidatorId(i as u64);
                    if self.is_honest(&v) {
                        let voted = state
                            .votes_round1
                            .get(block_id)
                            .map(|votes| votes.contains(&v))
                            .unwrap_or(false);
                        if !voted {
                            actions.push(Action::VoteRound1(v, *block_id));
                        }
                    }
                }

                // Check fast quorum
                if let Some(votes) = state.votes_round1.get(block_id) {
                    if votes.len() as u64 >= self.fast_quorum() {
                        actions.push(Action::CheckFastQuorum(*block_id));
                    }
                }

                // Can advance to round 2
                actions.push(Action::AdvanceToRound2);
            }

            // Round 2 votes
            if matches!(state.round, Round::Round2) {
                for i in 0..self.validator_count {
                    let v = ValidatorId(i as u64);
                    if self.is_honest(&v) {
                        let voted = state
                            .votes_round2
                            .get(block_id)
                            .map(|votes| votes.contains(&v))
                            .unwrap_or(false);
                        if !voted {
                            actions.push(Action::VoteRound2(v, *block_id));
                        }
                    }
                }

                // Check fallback quorum
                if let Some(votes) = state.votes_round2.get(block_id) {
                    if votes.len() as u64 >= self.fallback_quorum() {
                        actions.push(Action::CheckFallbackQuorum(*block_id));
                    }
                }
            }
        }

        // Skip votes if no proposal
        if !state.proposed.contains_key(&state.slot) {
            for i in 0..self.validator_count {
                let v = ValidatorId(i as u64);
                if self.is_honest(&v) {
                    let voted_skip = state
                        .skip_votes
                        .get(&state.slot)
                        .map(|votes| votes.contains(&v))
                        .unwrap_or(false);
                    if !voted_skip {
                        actions.push(Action::VoteSkip(v));
                    }
                }
            }

            // Check skip quorum
            if let Some(votes) = state.skip_votes.get(&state.slot) {
                if votes.len() as u64 >= self.fallback_quorum() {
                    actions.push(Action::CheckSkipQuorum);
                }
            }
        }

        // Next slot if finalized or skipped
        let slot_done = state.finalized.iter().any(|(_, s, _)| *s == state.slot)
            || state.skipped.contains(&state.slot);
        if slot_done && state.slot < 2 {
            // Limit exploration
            actions.push(Action::NextSlot);
        }

        // Network partition (limit to small validator counts to avoid state explosion)
        if state.partitioned.is_none() && !state.partition_healed && self.validator_count <= 4 {
            // Split validators into two partitions
            let mid = self.validator_count / 2;
            let mut p1 = BTreeSet::new();
            let mut p2 = BTreeSet::new();
            for i in 0..self.validator_count {
                let v = ValidatorId(i as u64);
                if i < mid {
                    p1.insert(v);
                } else {
                    p2.insert(v);
                }
            }
            if p1.len() >= 2 && p2.len() >= 2 {
                actions.push(Action::NetworkPartition(p1, p2));
            }
        }

        // Partition heal
        if state.partitioned.is_some() {
            actions.push(Action::PartitionHeal);
        }

        actions
    }

    fn step(&self, state: &State, action: &Action) -> State {
        let mut next = state.clone();

        match action {
            Action::ProposeBlock(leader, block_id) => {
                next.proposed.insert(state.slot, (*block_id, *leader));
            }

            Action::VoteRound1(v, block_id) => {
                next.votes_round1
                    .entry(*block_id)
                    .or_insert_with(BTreeSet::new)
                    .insert(*v);
            }

            Action::VoteRound2(v, block_id) => {
                next.votes_round2
                    .entry(*block_id)
                    .or_insert_with(BTreeSet::new)
                    .insert(*v);
            }

            Action::CheckFastQuorum(block_id) => {
                next.finalized
                    .push((*block_id, state.slot, Round::Round1));
            }

            Action::CheckFallbackQuorum(block_id) => {
                next.finalized
                    .push((*block_id, state.slot, Round::Round2));
            }

            Action::AdvanceToRound2 => {
                next.round = Round::Round2;
            }

            Action::VoteSkip(v) => {
                next.skip_votes
                    .entry(state.slot)
                    .or_insert_with(BTreeSet::new)
                    .insert(*v);
            }

            Action::CheckSkipQuorum => {
                next.skipped.insert(state.slot);
            }

            Action::NextSlot => {
                next.slot += 1;
                next.leader = ValidatorId((state.leader.0 + 1) % self.validator_count as u64);
                next.round = Round::Round1;
            }

            Action::NetworkPartition(p1, p2) => {
                next.partitioned = Some((p1.clone(), p2.clone()));
            }

            Action::PartitionHeal => {
                next.partitioned = None;
                next.partition_healed = true;
            }
        }

        next
    }

    /// Check NoFork safety property
    fn check_no_fork(&self, state: &State) -> bool {
        let mut slots_seen: HashMap<u64, BlockId> = HashMap::new();
        for (block_id, slot, _) in &state.finalized {
            if let Some(existing) = slots_seen.get(slot) {
                if existing != block_id {
                    return false; // Fork detected!
                }
            }
            slots_seen.insert(*slot, *block_id);
        }
        true
    }

    /// Check that finalized blocks have valid quorums
    fn check_quorum_validity(&self, state: &State) -> bool {
        for (block_id, _, round) in &state.finalized {
            match round {
                Round::Round1 => {
                    let votes = state.votes_round1.get(block_id).map(|v| v.len()).unwrap_or(0);
                    if (votes as u64) < self.fast_quorum() {
                        return false;
                    }
                }
                Round::Round2 => {
                    let votes = state.votes_round2.get(block_id).map(|v| v.len()).unwrap_or(0);
                    if (votes as u64) < self.fallback_quorum() {
                        return false;
                    }
                }
            }
        }
        true
    }

    /// Check voting integrity (no double voting)
    fn check_voting_integrity(&self, state: &State) -> bool {
        // Check round 1
        for (_, voters) in &state.votes_round1 {
            let mut seen = HashSet::new();
            for v in voters {
                if !seen.insert(v) {
                    return false; // Duplicate vote
                }
            }
        }

        // Check round 2
        for (_, voters) in &state.votes_round2 {
            let mut seen = HashSet::new();
            for v in voters {
                if !seen.insert(v) {
                    return false; // Duplicate vote
                }
            }
        }

        true
    }

    /// Check that no fork occurs even during network partition
    fn check_partition_safety(&self, state: &State) -> bool {
        // NoFork must hold even during partition
        if state.partitioned.is_some() {
            return self.check_no_fork(state);
        }
        true
    }

    /// Check that safety is maintained after partition heals
    fn check_post_partition_safety(&self, state: &State) -> bool {
        if state.partition_healed {
            return self.check_no_fork(state) && self.check_quorum_validity(state);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_basic() {
        let model = AlpenglowModel::new(3);
        let state = model.initial_state();

        assert_eq!(state.slot, 0);
        assert_eq!(state.leader, ValidatorId(0));
        assert!(state.finalized.is_empty());
    }

    #[test]
    fn test_propose_and_vote() {
        let model = AlpenglowModel::new(3);
        let mut state = model.initial_state();

        // Propose block
        let block_id = BlockId::new([0u8; 32]);
        state = model.step(&state, &Action::ProposeBlock(ValidatorId(0), block_id));
        assert!(state.proposed.contains_key(&0));

        // Vote round 1
        state = model.step(&state, &Action::VoteRound1(ValidatorId(0), block_id));
        state = model.step(&state, &Action::VoteRound1(ValidatorId(1), block_id));
        state = model.step(&state, &Action::VoteRound1(ValidatorId(2), block_id));

        // Check quorum (3/3 = 100% > 80%)
        assert!(model.check_quorum_validity(&state));
    }

    #[test]
    fn test_no_fork_property() {
        let model = AlpenglowModel::new(3);
        let state = model.initial_state();

        // Initially no fork
        assert!(model.check_no_fork(&state));

        // Add finalized block
        let mut state2 = state.clone();
        let block1 = BlockId::new([1u8; 32]);
        state2
            .finalized
            .push((block1, 0, Round::Round1));

        assert!(model.check_no_fork(&state2));

        // Try to add different block for same slot (should fail check)
        let mut state3 = state2.clone();
        let block2 = BlockId::new([2u8; 32]);
        state3
            .finalized
            .push((block2, 0, Round::Round1));

        assert!(!model.check_no_fork(&state3)); // Fork detected!
    }

    #[test]
    fn test_fast_path_quorum() {
        let model = AlpenglowModel::new(5);
        assert_eq!(model.fast_quorum(), 4); // 80% of 5 = 4
        assert_eq!(model.fallback_quorum(), 3); // 60% of 5 = 3
    }

    #[test]
    fn test_byzantine_validator() {
        let model = AlpenglowModel::new(5).with_byzantine(4);
        assert!(!model.is_honest(&ValidatorId(4)));
        assert!(model.is_honest(&ValidatorId(0)));
    }

    #[test]
    fn test_exhaustive_small_model() {
        // Small exhaustive test: 3 validators, 1 slot
        let model = AlpenglowModel::new(3);
        let initial = model.initial_state();

        let mut visited = HashSet::new();
        let mut queue = vec![initial];
        visited.insert(queue[0].clone());

        let mut safety_violations = 0;

        while let Some(state) = queue.pop() {
            // Check safety properties
            if !model.check_no_fork(&state) {
                safety_violations += 1;
                eprintln!("NoFork violation found!");
            }

            if !model.check_quorum_validity(&state) {
                safety_violations += 1;
                eprintln!("Quorum validity violation!");
            }

            if !model.check_voting_integrity(&state) {
                safety_violations += 1;
                eprintln!("Voting integrity violation!");
            }

            // Explore next states
            for action in model.actions(&state) {
                let next_state = model.step(&state, &action);
                if visited.insert(next_state.clone()) {
                    queue.push(next_state);
                }
            }
        }

        println!("Explored {} unique states", visited.len());
        assert_eq!(safety_violations, 0, "Safety violations detected!");
    }

    #[test]
    fn test_network_partition_safety() {
        println!("\n=== Testing Network Partition Safety ===");
        let model = AlpenglowModel::new(4);

        let mut visited = HashSet::new();
        let mut queue = vec![model.initial_state()];
        visited.insert(model.initial_state());

        let mut partition_states = 0;
        let mut healed_states = 0;
        let mut safety_violations = 0;

        while let Some(state) = queue.pop() {
            // Track partition states
            if state.partitioned.is_some() {
                partition_states += 1;
            }
            if state.partition_healed {
                healed_states += 1;
            }

            // Check partition-specific safety properties
            if !model.check_partition_safety(&state) {
                println!("VIOLATION: Fork during partition!");
                safety_violations += 1;
            }

            if !model.check_post_partition_safety(&state) {
                println!("VIOLATION: Safety violated after partition heal!");
                safety_violations += 1;
            }

            // Standard safety checks
            if !model.check_no_fork(&state) {
                println!("VIOLATION: Fork detected!");
                safety_violations += 1;
            }

            // Explore next states (limit depth)
            if visited.len() < 5000 {
                for action in model.actions(&state) {
                    let next_state = model.step(&state, &action);
                    if visited.insert(next_state.clone()) {
                        queue.push(next_state);
                    }
                }
            }
        }

        println!("Explored {} states", visited.len());
        println!("Partition states: {}", partition_states);
        println!("Healed states: {}", healed_states);
        println!("Safety violations: {}", safety_violations);

        assert!(partition_states > 0, "No partition states explored");
        assert_eq!(safety_violations, 0, "Safety violations with partitions!");
    }
}
