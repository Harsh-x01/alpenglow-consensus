//! Quick demonstration without heavy dependencies

use alpenglow::{ConsensusEngine, types::*};

fn main() {
    println!("=== Alpenglow Consensus Quick Demo ===\n");

    // Create validators
    let mut validator_set = ValidatorSet::new();
    for i in 0..5 {
        validator_set.add_validator(ValidatorConfig {
            id: ValidatorId(i),
            stake: StakeWeight(100),
            is_byzantine: false,
            is_offline: false,
        });
    }
    println!("✓ Created 5 validators with 100 stake each");
    println!("  Total stake: {}\n", validator_set.total_stake().as_u64());

    // Create consensus engine
    let config = alpenglow::consensus::ConsensusConfig::default();
    let mut engine = ConsensusEngine::new(ValidatorId(0), validator_set.clone(), config);

    println!("✓ Consensus engine initialized");
    println!("  Leader: {}\n", engine.is_leader());

    // Create a block
    let mut block = Block {
        id: BlockId::new([0u8; 32]),
        slot: Slot(0),
        parent: None,
        leader: ValidatorId(0),
        transactions: vec![vec![1, 2, 3], vec![4, 5, 6]],
        timestamp: 1000,
    };
    block.id = block.compute_id();

    println!("✓ Block created");
    println!("  Block ID: {}", block.id);
    println!("  Transactions: {}\n", block.transactions.len());

    // Propose block
    match engine.propose_block(block.clone()) {
        Ok(shreds) => {
            println!("✓ Block proposed successfully");
            println!("  Shreds created: {}", shreds.len());
            println!("  Reconstruction threshold: 80%\n");
        }
        Err(e) => {
            println!("✗ Error: {}", e);
        }
    }

    println!("=== Demo Complete ===");
    println!("\nProtocol Features:");
    println!("  • Fast path: 1 round, 80% quorum (~100ms)");
    println!("  • Fallback: 2 rounds, 60% quorum (~250ms)");
    println!("  • Byzantine tolerance: 20% of stake");
    println!("  • Safety: No forks (proven)");
}
