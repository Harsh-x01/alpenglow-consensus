# Alpenglow Consensus Architecture

## Overview

Alpenglow is a formally verified Byzantine Fault Tolerant (BFT) consensus protocol designed for Solana. It achieves fast finality through a dual-path voting mechanism while maintaining strong safety and liveness guarantees.

## Core Components

### 1. Votor (Voting Mechanism)

**Purpose**: Manages the dual-path concurrent voting strategy for block finalization.

**Key Features**:
- **Fast Path**: Single-round finalization with 80% stake threshold
- **Fallback Path**: Two-round finalization with 60% stake threshold
- **Concurrent Execution**: Both paths run simultaneously for optimal performance

**Implementation**: `src/votor.rs`

**State Machine**:
```
Slot Start → Round 1 (80% target)
    ├─→ 80% reached → Finalized (fast path)
    └─→ timeout → Round 2 (60% target)
           └─→ 60% reached → Finalized (fallback path)
```

### 2. Rotor (Data Propagation)

**Purpose**: Efficiently disseminates block data across the validator network using erasure coding.

**Key Features**:
- **Erasure Coding**: Splits blocks into shreds for parallel transmission
- **Stake-Weighted Relays**: Selects relays based on validator stake
- **Reconstruction Threshold**: Requires only 80% of shreds for block recovery
- **Fast Propagation**: Optimized for low-latency distribution

**Implementation**: `src/rotor.rs`

**Flow**:
```
Leader → Encode Block → Distribute Shreds → Relays → Validators
                                                    ↓
                              Reconstruct Block ← 80% Shreds
```

### 3. Consensus Engine

**Purpose**: Orchestrates the overall consensus process, integrating Votor and Rotor.

**Implementation**: `src/consensus.rs`

**Responsibilities**:
- Block proposal (when leader)
- Shred reception and block reconstruction
- Vote casting and processing
- Round timeout management
- Slot transitions and leader rotation

## Protocol Flow

### Normal Operation (Fast Path)

1. **Block Proposal**:
   - Leader for slot `n` creates a new block
   - Block is encoded into erasure-coded shreds
   - Shreds distributed to stake-weighted relays

2. **Block Propagation**:
   - Relays forward shreds to all validators
   - Validators reconstruct block from ≥80% of shreds
   - Honest validators receive block within network delay (~30ms)

3. **Round 1 Voting**:
   - Upon receiving complete block, validators cast notarization votes
   - Votes are aggregated and counted by stake weight
   - If ≥80% stake votes within round 1 timeout (~100ms):
     - Block immediately finalized
     - Certificate broadcast to all validators

4. **Slot Transition**:
   - Leader rotates to next validator
   - New slot begins with round 1

### Fallback Operation (Slow Path)

If fast path fails (e.g., due to network delays or <80% online):

1. **Round 1 Timeout**:
   - After ~100ms, system advances to round 2
   - No finalization occurred in round 1

2. **Round 2 Voting**:
   - Validators cast finalization votes
   - Lower threshold: ≥60% stake required
   - Timeout: additional ~150ms

3. **Fallback Finalization**:
   - If ≥60% stake votes in round 2:
     - Block finalized via fallback path
     - Typically occurs within ~250ms total

### Fault Scenarios

**Byzantine Leader** (≤20% stake):
- May propose invalid block or not propose at all
- Honest validators detect and skip to next slot
- No impact on liveness (next leader proceeds)

**Offline Validators** (≤20% stake):
- Don't participate in voting
- Fast path may fail (if total online <80%)
- Fallback path succeeds (if online ≥60%)

**Network Partition**:
- If partition isolates >40% stake:
  - Consensus may stall (expected behavior)
  - Safety maintained (no forks)
- When healed, normal operation resumes

## Safety Properties

### Formal Guarantees

1. **No Fork**: At most one block can be finalized per slot
   - Proven via TLA+ invariant checking
   - Holds under ≤20% Byzantine + ≤20% offline assumption

2. **Quorum Intersection**: Any two quorums must overlap by >40% honest stake
   - Prevents conflicting finalizations
   - Mathematical proof in `proofs/safety.md`

3. **Voting Integrity**: No honest validator votes twice for different blocks
   - Enforced by state machine logic
   - Verified in implementation tests

## Liveness Properties

### Conditions

Under these assumptions, liveness is guaranteed:
- ≤20% Byzantine validators by stake
- ≤20% offline validators by stake
- Network eventually delivers messages (partial synchrony)
- Leader schedule provides eventual honest leader

### Guarantees

1. **Finalization Progress**:
   - With ≥80% online honest: 1-round finality (~100ms)
   - With ≥60% online honest: 2-round finality (~250ms)

2. **Recovery from Failures**:
   - Byzantine leader causes at most 1-slot delay
   - Next honest leader resumes normal operation

3. **No Permanent Stall**:
   - Proven via temporal logic (see `proofs/liveness.md`)
   - Verified with bounded model checking

## Performance Characteristics

### Latency

- **Median Finality**: 100-150ms (fast path)
- **99th Percentile**: <400ms (fallback path)
- **Network Delay**: ~30ms for block propagation (Rotor)
- **Vote Aggregation**: ~50-70ms for quorum formation

### Throughput

- **Blocks per Second**: Limited by network, not consensus
- **Transaction Capacity**: Depends on block size and validation
- **Scalability**: O(n) message complexity per block

### Resource Usage

- **Bandwidth**: Optimized with erasure coding (each validator receives ~80% of block size)
- **Computation**: Erasure coding overhead + signature verification
- **Storage**: Minimal (only recent slots for recovery)

## Comparison with Other Protocols

| Feature | Alpenglow | PBFT | Tendermint | HotStuff |
|---------|-----------|------|------------|----------|
| Finality | 1-2 rounds | 3 phases | 2 rounds | 1 round |
| Latency | 100-250ms | 300-500ms | 200-400ms | 100-200ms |
| Byzantine Tolerance | 20% | 33% | 33% | 33% |
| Offline Tolerance | +20% | 0% | 0% | 0% |
| Message Complexity | O(n) | O(n²) | O(n²) | O(n) |

**Key Advantages**:
- **Dual Fault Tolerance**: Handles both Byzantine and crash faults
- **Concurrent Paths**: Optimistic fast path with guaranteed fallback
- **Erasure Coding**: Efficient block propagation via Rotor

## Integration with Solana

### Replacement for Tower BFT

Alpenglow replaces Solana's current Tower BFT consensus with:
- Faster finality (Tower: 13+ slots → Alpenglow: 1-2 slots)
- Stronger guarantees (proven safety and liveness)
- Better network efficiency (erasure coding)

### Compatibility

- **Leader Schedule**: Reuses Solana's existing stake-weighted rotation
- **Transaction Processing**: Unchanged (consensus only finalizes blocks)
- **Proof of History**: Can be integrated or removed (not required for Alpenglow)

### Migration Path

1. Deploy Alpenglow on testnet
2. Run parallel consensus for verification
3. Gradual mainnet rollout with feature flag
4. Full replacement after stability confirmation

## Future Enhancements

### Potential Improvements

1. **Dynamic Thresholds**: Adjust 80%/60% based on network conditions
2. **Sharding Support**: Extend to multiple parallel chains
3. **Optimistic Responsiveness**: Further reduce latency in optimal conditions
4. **Zero-Knowledge Proofs**: Enhance privacy for certain transaction types

### Research Directions

- **Asynchronous Finality**: Remove timing assumptions entirely
- **Cross-Chain Consensus**: Coordinate with other blockchains
- **Quantum Resistance**: Upgrade cryptographic primitives

## References

- Alpenglow Whitepaper v1.1 (July 22, 2025)
- TLA+ Specification: `tla-spec/Alpenglow.tla`
- Implementation: `rust-implementation/src/`
- Safety Proof: `proofs/safety.md`
- Liveness Proof: `proofs/liveness.md`

---

For implementation details, see:
- [TLA+ Specification Guide](tla-spec.md)
- [Implementation Guide](implementation.md)
- [Testing Strategy](testing.md)