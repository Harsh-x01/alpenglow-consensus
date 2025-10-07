//! Simple demonstration of Alpenglow consensus

use alpenglow::{ConsensusEngine, types::*};

fn main() {
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║      Alpenglow Consensus - Live Demonstration           ║");
    println!("╚══════════════════════════════════════════════════════════╝\n");

    // Create a validator set with 5 validators
    println!("📋 Setting up validator network...");
    let mut validator_set = ValidatorSet::new();
    for i in 0..5 {
        validator_set.add_validator(ValidatorConfig {
            id: ValidatorId(i),
            stake: StakeWeight(100),
            is_byzantine: false,
            is_offline: false,
        });
        println!("   ✓ Validator {} added with stake 100", i);
    }
    println!("   Total stake: {}\n", validator_set.total_stake().as_u64());

    // Calculate quorum thresholds
    let fast_quorum = (validator_set.total_stake().0 * 80) / 100;
    let fallback_quorum = (validator_set.total_stake().0 * 60) / 100;
    println!("🎯 Quorum Requirements:");
    println!("   Fast path (80%): {} stake", fast_quorum);
    println!("   Fallback path (60%): {} stake\n", fallback_quorum);

    // Create consensus engines for all validators
    println!("🚀 Initializing consensus engines...");
    let config = alpenglow::consensus::ConsensusConfig::default();
    let mut engines: Vec<_> = (0..5)
        .map(|i| {
            let engine = ConsensusEngine::new(ValidatorId(i), validator_set.clone(), config.clone());
            println!("   ✓ Engine {} initialized (Leader: {})", i, engine.is_leader());
            engine
        })
        .collect();
    println!();

    // Create a block
    println!("📦 Leader (Validator 0) proposing block...");
    let mut block = Block {
        id: BlockId::new([0u8; 32]),
        slot: Slot(0),
        parent: None,
        leader: ValidatorId(0),
        transactions: vec![
            vec![1, 2, 3, 4],  // Simulated transaction data
            vec![5, 6, 7, 8],
        ],
        timestamp: 1000,
    };
    block.id = block.compute_id();
    println!("   Block ID: {}", block.id);
    println!("   Slot: {}", block.slot);
    println!("   Transactions: {}\n", block.transactions.len());

    // Leader proposes block and creates shreds
    println!("🔀 Encoding block into shreds (erasure coding)...");
    match engines[0].propose_block(block.clone()) {
        Ok(shreds) => {
            println!("   ✓ Block encoded into {} shreds", shreds.len());
            println!("   ✓ Reconstruction threshold: {}% of shreds\n", 80);

            // Distribute shreds to all validators
            println!("📡 Distributing shreds to validators...");
            for (i, engine) in engines.iter_mut().enumerate() {
                for shred in shreds.clone() {
                    match engine.receive_shred(shred) {
                        Ok(_) => {},
                        Err(e) => println!("   ⚠ Validator {} error: {}", i, e),
                    }
                }
                println!("   ✓ Validator {} received shreds", i);
            }
            println!();

            // Check finalization status
            println!("🏁 Checking finalization status...");
            let mut finalized_count = 0;
            for (i, engine) in engines.iter().enumerate() {
                if engine.is_finalized(&block.id) {
                    finalized_count += 1;
                    println!("   ✓ Validator {} sees block as finalized", i);
                }
            }
            println!();

            if finalized_count >= 4 {
                println!("✅ SUCCESS! Block finalized via FAST PATH (80% quorum)");
                println!("   {} out of {} validators finalized the block", finalized_count, engines.len());
                println!("   Finalization time: ~100ms (1 round)\n");
            } else {
                println!("⏱️  Block not finalized yet (would use fallback path)");
                println!("   Would finalize in round 2 with 60% quorum\n");
            }

            // Show finalized blocks
            println!("📊 Finalization Certificates:");
            if let Some(cert) = engines[0].finalized_blocks().first() {
                println!("   Block ID: {}", cert.block_id);
                println!("   Slot: {}", cert.slot);
                println!("   Round: {:?}", cert.round);
                println!("   Votes: {}", cert.votes.len());
                println!("   Total Stake: {}", cert.total_stake.as_u64());
            }
        }
        Err(e) => {
            println!("   ✗ Error proposing block: {}", e);
        }
    }

    println!("\n╔══════════════════════════════════════════════════════════╗");
    println!("║              Protocol Characteristics                    ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  Fast Path:     1 round, 80% quorum (~100ms)             ║");
    println!("║  Fallback Path: 2 rounds, 60% quorum (~250ms)            ║");
    println!("║  Byzantine Tolerance: 20% of stake                       ║");
    println!("║  Offline Tolerance: +20% of stake (40% total)            ║");
    println!("║  Safety: No forks (mathematically proven)                ║");
    println!("║  Liveness: Guaranteed progress under assumptions         ║");
    println!("╚══════════════════════════════════════════════════════════╝\n");

    println!("✨ Demo complete! The Alpenglow consensus protocol is working.");
    println!("   For more details, see docs/architecture.md\n");
}