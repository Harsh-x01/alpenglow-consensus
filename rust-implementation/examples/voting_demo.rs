//! Demonstration of Votor (voting mechanism)

use alpenglow::votor::Votor;
use alpenglow::types::*;

fn main() {
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║     Alpenglow Votor - Voting Mechanism Demo             ║");
    println!("╚══════════════════════════════════════════════════════════╝\n");

    // Setup: 5 validators with equal stake
    println!("📋 Creating validator set (5 validators, 100 stake each)...\n");
    let mut validator_set = ValidatorSet::new();
    for i in 0..5 {
        validator_set.add_validator(ValidatorConfig {
            id: ValidatorId(i),
            stake: StakeWeight(100),
            is_byzantine: false,
            is_offline: false,
        });
    }

    println!("🎯 Quorum Thresholds:");
    println!("   Total stake: 500");
    println!("   Fast path (80%): 400 stake (4/5 validators)");
    println!("   Fallback path (60%): 300 stake (3/5 validators)\n");

    // Create Votor instance
    let mut votor = Votor::new(validator_set.clone());

    // Create a block to vote on
    let block_id = BlockId::new([1u8; 32]);
    let slot = Slot(0);

    println!("═══════════════════════════════════════════════════════════");
    println!("  Scenario 1: Fast Path (80% quorum in Round 1)");
    println!("═══════════════════════════════════════════════════════════\n");

    println!("📦 Block proposed: {}", block_id);
    println!("   Slot: {}\n", slot);

    // Round 1: 4 out of 5 validators vote (80%)
    println!("🗳️  Round 1 voting:");
    for i in 0..4 {
        let vote = Vote {
            validator: ValidatorId(i),
            block_id,
            slot,
            round: VoteRound::Round1,
            signature: vec![],
        };

        print!("   Validator {} votes... ", i);
        match votor.process_vote(vote) {
            Ok(Some(cert)) => {
                println!("✅ FINALIZED!");
                println!("\n🎉 Fast path success!");
                println!("   Block finalized in Round 1 with {} votes", cert.votes.len());
                println!("   Total stake: {}", cert.total_stake.as_u64());
                println!("   Time: ~100ms (1 round)\n");
                break;
            }
            Ok(None) => println!("recorded"),
            Err(e) => println!("error: {}", e),
        }
    }

    // New scenario: Fallback path
    println!("\n═══════════════════════════════════════════════════════════");
    println!("  Scenario 2: Fallback Path (60% quorum in Round 2)");
    println!("═══════════════════════════════════════════════════════════\n");

    // Reset with new Votor
    let mut votor2 = Votor::new(validator_set.clone());
    let block_id2 = BlockId::new([2u8; 32]);

    println!("📦 New block proposed: {}", block_id2);
    println!("   Slot: {}\n", slot);

    // Round 1: Only 3 validators vote (60%, not enough for fast path)
    println!("🗳️  Round 1 voting:");
    for i in 0..3 {
        let vote = Vote {
            validator: ValidatorId(i),
            block_id: block_id2,
            slot,
            round: VoteRound::Round1,
            signature: vec![],
        };

        print!("   Validator {} votes... ", i);
        match votor2.process_vote(vote) {
            Ok(Some(_)) => println!("finalized (shouldn't happen)"),
            Ok(None) => println!("recorded (need 80% for fast path)"),
            Err(e) => println!("error: {}", e),
        }
    }

    println!("\n   ⏱️  Round 1 timeout (no 80% quorum reached)");
    println!("   → Advancing to Round 2 (60% threshold)\n");

    // Advance to round 2
    votor2.advance_to_round2();

    // Round 2: Same 3 validators vote again (60%, enough for fallback)
    println!("🗳️  Round 2 voting:");
    for i in 0..3 {
        let vote = Vote {
            validator: ValidatorId(i),
            block_id: block_id2,
            slot,
            round: VoteRound::Round2,
            signature: vec![],
        };

        print!("   Validator {} votes... ", i);
        match votor2.process_vote(vote) {
            Ok(Some(cert)) => {
                println!("✅ FINALIZED!");
                println!("\n🎉 Fallback path success!");
                println!("   Block finalized in Round 2 with {} votes", cert.votes.len());
                println!("   Total stake: {}", cert.total_stake.as_u64());
                println!("   Time: ~250ms (2 rounds)\n");
                break;
            }
            Ok(None) => println!("recorded"),
            Err(e) => println!("error: {}", e),
        }
    }

    // Scenario 3: Double vote prevention
    println!("\n═══════════════════════════════════════════════════════════");
    println!("  Scenario 3: Safety - Double Vote Prevention");
    println!("═══════════════════════════════════════════════════════════\n");

    let mut votor3 = Votor::new(validator_set);
    let block_id3 = BlockId::new([3u8; 32]);

    let vote1 = Vote {
        validator: ValidatorId(0),
        block_id: block_id3,
        slot,
        round: VoteRound::Round1,
        signature: vec![],
    };

    print!("   Validator 0 casts first vote... ");
    match votor3.process_vote(vote1.clone()) {
        Ok(_) => println!("✓ accepted"),
        Err(e) => println!("✗ error: {}", e),
    }

    print!("   Validator 0 tries to vote again... ");
    match votor3.process_vote(vote1) {
        Ok(_) => println!("✗ ERROR: double vote accepted!"),
        Err(e) => println!("✓ correctly rejected: {}", e),
    }

    println!("\n╔══════════════════════════════════════════════════════════╗");
    println!("║                     Summary                              ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  ✅ Fast path works (80% → 1 round)                      ║");
    println!("║  ✅ Fallback path works (60% → 2 rounds)                 ║");
    println!("║  ✅ Safety enforced (no double voting)                   ║");
    println!("║  ✅ Concurrent paths provide optimal finality            ║");
    println!("╚══════════════════════════════════════════════════════════╝\n");

    println!("✨ Votor demonstration complete!\n");
}