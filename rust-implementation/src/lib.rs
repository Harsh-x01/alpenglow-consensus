//! # Alpenglow Consensus Implementation
//!
//! A formally verified implementation of the Alpenglow consensus protocol.
//!
//! ## Overview
//!
//! Alpenglow provides:
//! - **Fast path**: 1-round finality with 80% stake
//! - **Fallback path**: 2-round finality with 60% stake
//! - **Fault tolerance**: 20% Byzantine + 20% offline (40% total)
//!
//! ## Architecture
//!
//! - `votor`: Voting mechanism with concurrent dual-path finalization
//! - `rotor`: Data propagation with erasure coding
//! - `types`: Core data structures and message formats
//! - `consensus`: Main consensus engine

pub mod consensus;
pub mod rotor;
pub mod types;
pub mod votor;

pub use consensus::ConsensusEngine;
pub use types::{Block, BlockId, Slot, StakeWeight, ValidatorId, Vote};

/// Protocol version
pub const PROTOCOL_VERSION: u8 = 1;

/// Default timeout for round 1 (milliseconds)
pub const ROUND1_TIMEOUT_MS: u64 = 100;

/// Default timeout for round 2 (milliseconds)
pub const ROUND2_TIMEOUT_MS: u64 = 150;

/// Fast path quorum threshold (80%)
pub const FAST_QUORUM_PCT: u8 = 80;

/// Fallback path quorum threshold (60%)
pub const FALLBACK_QUORUM_PCT: u8 = 60;

/// Maximum Byzantine fault tolerance (20%)
pub const MAX_BYZANTINE_PCT: u8 = 20;

/// Maximum offline tolerance (20%)
pub const MAX_OFFLINE_PCT: u8 = 20;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants() {
        assert_eq!(FAST_QUORUM_PCT, 80);
        assert_eq!(FALLBACK_QUORUM_PCT, 60);
        assert!(FAST_QUORUM_PCT > FALLBACK_QUORUM_PCT);
        assert_eq!(MAX_BYZANTINE_PCT + MAX_OFFLINE_PCT, 40);
    }
}