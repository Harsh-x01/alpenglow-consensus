------------------------ MODULE AlpenglowEnhanced ------------------------
(***************************************************************************
 * ENHANCED TLA+ Specification of Alpenglow Consensus Protocol
 *
 * This version includes:
 * - Skip certificates for Byzantine leader timeouts
 * - Bounded time properties for finalization
 * - Enhanced Rotor erasure coding model
 * - Comprehensive safety and liveness properties
 *
 * Addresses formal verification challenge requirements
 ***************************************************************************)

EXTENDS Naturals, FiniteSets, Sequences, TLC

CONSTANTS
    Validators,      \* Set of all validators
    Blocks,          \* Set of possible blocks
    MaxSlot,         \* Maximum slot number to model
    ByzantineSet,    \* Set of Byzantine (adversarial) validators
    OfflineSet,      \* Set of offline (crashed) validators
    MaxTime,         \* Maximum time for bounded model checking
    Round1Timeout,   \* Timeout for round 1 (in time units)
    Round2Timeout,   \* Timeout for round 2 (in time units)
    SlotTimeout      \* Timeout for entire slot

VARIABLES
    slot,            \* Current slot number
    leader,          \* Current leader for the slot
    proposed,        \* Set of proposed blocks per slot
    received,        \* [validator -> set of received blocks]
    shreds,          \* [validator -> [block -> set of shred indices]]
    votesRound1,     \* [block -> set of validators who voted in round 1]
    votesRound2,     \* [block -> set of validators who voted in round 2]
    skipVotes,       \* [slot -> set of validators who voted to skip]
    finalized,       \* Set of finalized (block, slot) pairs
    skipped,         \* Set of skipped slots (due to Byzantine leader)
    round,           \* Current round within slot (1 or 2)
    time,            \* Global logical time
    messages,        \* Set of messages in transit
    partitioned,     \* Set of {partition1, partition2} (empty = no partition)
    partitionHealed  \* Boolean: whether partition has healed

vars == <<slot, leader, proposed, received, shreds, votesRound1, votesRound2,
          skipVotes, finalized, skipped, round, time, messages, partitioned, partitionHealed>>

(***************************************************************************)
(* Type Invariants and Helper Functions                                    *)
(***************************************************************************)

ValidatorStake == [v \in Validators |-> 1]  \* Equal stake for simplicity
TotalStake == Cardinality(Validators)

HonestValidators == Validators \ (ByzantineSet \cup OfflineSet)

StakeOf(vs) == Cardinality(vs)  \* Simplified: each validator = 1 stake

\* Network partition helpers
CanCommunicate(v1, v2) ==
    IF partitioned = {}
    THEN TRUE  \* No partition, all can communicate
    ELSE \E p \in partitioned : v1 \in p /\ v2 \in p  \* Same partition

Partition1 == IF partitioned # {} THEN CHOOSE p \in partitioned : TRUE ELSE {}
Partition2 == IF partitioned # {} THEN CHOOSE p \in partitioned : p # Partition1 ELSE {}

\* Quorum thresholds
FastQuorum == (80 * TotalStake) \div 100       \* 80% for 1-round
FallbackQuorum == (60 * TotalStake) \div 100   \* 60% for 2-round
SkipQuorum == (60 * TotalStake) \div 100       \* 60% to skip slot

\* Erasure coding parameters
TotalShreds == Cardinality(Validators)
MinSredsForReconstruction == (80 * TotalShreds) \div 100  \* 80% threshold

TypeOK ==
    /\ slot \in 0..MaxSlot
    /\ leader \in Validators
    /\ proposed \subseteq [slot: 0..MaxSlot, block: Blocks, leader: Validators]
    /\ received \in [Validators -> SUBSET Blocks]
    /\ votesRound1 \in [Blocks -> SUBSET Validators]
    /\ votesRound2 \in [Blocks -> SUBSET Validators]
    /\ skipVotes \in [0..MaxSlot -> SUBSET Validators]
    /\ finalized \subseteq [block: Blocks, slot: 0..MaxSlot, round: {1, 2}, time: 0..MaxTime]
    /\ skipped \subseteq 0..MaxSlot
    /\ round \in {1, 2}
    /\ time \in 0..MaxTime
    /\ partitioned \in SUBSET (SUBSET Validators)
    /\ partitionHealed \in BOOLEAN

(***************************************************************************)
(* Initial State                                                           *)
(***************************************************************************)

Init ==
    /\ slot = 0
    /\ leader \in Validators  \* Can be honest or Byzantine
    /\ proposed = {}
    /\ received = [v \in Validators |-> {}]
    /\ shreds = [v \in Validators |-> [b \in Blocks |-> {}]]
    /\ votesRound1 = [b \in Blocks |-> {}]
    /\ votesRound2 = [b \in Blocks |-> {}]
    /\ skipVotes = [s \in 0..MaxSlot |-> {}]
    /\ finalized = {}
    /\ skipped = {}
    /\ round = 1
    /\ time = 0
    /\ messages = {}
    /\ partitioned = {}  \* Start with no partition
    /\ partitionHealed = FALSE

(***************************************************************************)
(* Erasure Coding Model                                                    *)
(***************************************************************************)

\* Block can be reconstructed if validator has ≥80% of shreds
CanReconstruct(v, b) ==
    Cardinality(shreds[v][b]) >= MinSredsForReconstruction

\* Leader distributes shreds to validators (honest relays)
DistributeShreds(p) ==
    /\ p \in proposed
    /\ p.slot = slot
    /\ p.leader = leader
    /\ \E v \in HonestValidators :
        \* Validator receives shreds (modeling relay propagation)
        /\ Cardinality(shreds[v][p.block]) < TotalShreds
        /\ shreds' = [shreds EXCEPT ![v][p.block] =
                        shreds[v][p.block] \union {CHOOSE n \in 1..TotalShreds :
                            n \notin shreds[v][p.block]}]

(***************************************************************************)
(* Actions                                                                 *)
(***************************************************************************)

\* Leader proposes a block for current slot
ProposeBlock ==
    /\ leader \in HonestValidators  \* Only honest leaders propose valid blocks
    /\ ~\E p \in proposed : p.slot = slot  \* No block proposed yet for this slot
    /\ LET newProposal == [slot |-> slot, block |-> CHOOSE b \in Blocks : TRUE,
                           leader |-> leader]
       IN proposed' = proposed \union {newProposal}
    /\ UNCHANGED <<slot, leader, received, shreds, votesRound1, votesRound2,
                   skipVotes, finalized, skipped, round, time, messages, partitioned, partitionHealed>>

\* Byzantine leader may not propose (or propose late/invalid)
ByzantineNoProposal ==
    /\ leader \in ByzantineSet
    /\ ~\E p \in proposed : p.slot = slot
    /\ UNCHANGED <<slot, leader, proposed, received, shreds, votesRound1,
                   votesRound2, skipVotes, finalized, skipped, round, time, messages, partitioned, partitionHealed>>

\* Rotor dissemination: distribute shreds via erasure coding
DisseminateShreds ==
    /\ \E p \in proposed :
        /\ p.slot = slot
        /\ DistributeShreds(p)
    /\ UNCHANGED <<slot, leader, proposed, received, votesRound1, votesRound2,
                   skipVotes, finalized, skipped, round, time, messages, partitioned, partitionHealed>>

\* Validator reconstructs block from shreds
ReconstructBlock ==
    /\ \E v \in HonestValidators, p \in proposed :
        /\ p.slot = slot
        /\ p.block \notin received[v]
        /\ CanReconstruct(v, p.block)  \* Has enough shreds
        /\ received' = [received EXCEPT ![v] = @ \union {p.block}]
    /\ UNCHANGED <<slot, leader, proposed, shreds, votesRound1, votesRound2,
                   skipVotes, finalized, skipped, round, time, messages, partitioned, partitionHealed>>

\* Validator casts round 1 vote (notarization)
VoteRound1 ==
    /\ round = 1
    /\ \E v \in HonestValidators, b \in Blocks :
        /\ b \in received[v]
        /\ v \notin votesRound1[b]  \* Haven't voted for this block yet
        /\ votesRound1' = [votesRound1 EXCEPT ![b] = @ \union {v}]
    /\ UNCHANGED <<slot, leader, proposed, received, shreds, votesRound2,
                   skipVotes, finalized, skipped, round, time, messages, partitioned, partitionHealed>>

\* Check if fast path quorum reached (80%)
CheckFastQuorum ==
    /\ round = 1
    /\ \E b \in Blocks :
        /\ StakeOf(votesRound1[b]) >= FastQuorum
        /\ finalized' = finalized \union {[block |-> b, slot |-> slot,
                                            round |-> 1, time |-> time]}
    /\ UNCHANGED <<slot, leader, proposed, received, shreds, votesRound1, votesRound2,
                   skipVotes, skipped, round, time, messages>>

\* Move to round 2 if fast quorum not reached within timeout
AdvanceToRound2 ==
    /\ round = 1
    /\ time >= Round1Timeout
    /\ ~\E b \in Blocks : StakeOf(votesRound1[b]) >= FastQuorum
    /\ round' = 2
    /\ UNCHANGED <<slot, leader, proposed, received, shreds, votesRound1, votesRound2,
                   skipVotes, finalized, skipped, time, messages>>

\* Validator casts round 2 vote (finalization)
VoteRound2 ==
    /\ round = 2
    /\ \E v \in HonestValidators, b \in Blocks :
        /\ b \in received[v]
        /\ v \notin votesRound2[b]
        /\ votesRound2' = [votesRound2 EXCEPT ![b] = @ \union {v}]
    /\ UNCHANGED <<slot, leader, proposed, received, shreds, votesRound1,
                   skipVotes, finalized, skipped, round, time, messages, partitioned, partitionHealed>>

\* Check if fallback quorum reached (60%)
CheckFallbackQuorum ==
    /\ round = 2
    /\ \E b \in Blocks :
        /\ StakeOf(votesRound2[b]) >= FallbackQuorum
        /\ finalized' = finalized \union {[block |-> b, slot |-> slot,
                                            round |-> 2, time |-> time]}
    /\ UNCHANGED <<slot, leader, proposed, received, shreds, votesRound1, votesRound2,
                   skipVotes, skipped, round, time, messages>>

\* SKIP CERTIFICATE: Validator votes to skip slot (Byzantine/offline leader)
VoteSkip ==
    /\ time >= SlotTimeout
    /\ ~\E p \in proposed : p.slot = slot  \* No valid proposal received
    /\ \E v \in HonestValidators :
        /\ v \notin skipVotes[slot]
        /\ skipVotes' = [skipVotes EXCEPT ![slot] = @ \union {v}]
    /\ UNCHANGED <<slot, leader, proposed, received, shreds, votesRound1, votesRound2,
                   finalized, skipped, round, time, messages>>

\* Check if skip quorum reached
CheckSkipQuorum ==
    /\ StakeOf(skipVotes[slot]) >= SkipQuorum
    /\ slot \notin skipped
    /\ skipped' = skipped \union {slot}
    /\ UNCHANGED <<slot, leader, proposed, received, shreds, votesRound1, votesRound2,
                   skipVotes, finalized, round, time, messages>>

\* Advance to next slot (after finalization or skip)
NextSlot ==
    /\ slot < MaxSlot
    /\ \/ \E f \in finalized : f.slot = slot   \* Block finalized
       \/ slot \in skipped                      \* Slot skipped
    /\ slot' = slot + 1
    /\ leader' \in Validators  \* Leader rotation
    /\ round' = 1
    /\ time' = 0  \* Reset time for new slot
    /\ votesRound1' = [b \in Blocks |-> {}]
    /\ votesRound2' = [b \in Blocks |-> {}]
    /\ UNCHANGED <<proposed, received, shreds, skipVotes, finalized, skipped, messages, partitioned, partitionHealed>>

\* Time progresses (models passage of time)
Tick ==
    /\ time < MaxTime
    /\ time' = time + 1
    /\ UNCHANGED <<slot, leader, proposed, received, shreds, votesRound1, votesRound2,
                   skipVotes, finalized, skipped, round, messages, partitioned, partitionHealed>>

(***************************************************************************)
(* Network Partition Model                                                *)
(***************************************************************************)

\* Network partition occurs (splits validators into two groups)
NetworkPartition ==
    /\ partitioned = {}  \* No current partition
    /\ ~partitionHealed  \* Haven't healed yet this run
    /\ \E p1, p2 \in SUBSET Validators :
        /\ p1 \cup p2 = Validators
        /\ p1 \cap p2 = {}
        /\ p1 # {} /\ p2 # {}
        /\ Cardinality(p1) >= 2  \* Each partition has at least 2 validators
        /\ Cardinality(p2) >= 2
        /\ partitioned' = {p1, p2}
    /\ UNCHANGED <<slot, leader, proposed, received, shreds, votesRound1, votesRound2,
                   skipVotes, finalized, skipped, round, time, messages, partitionHealed>>

\* Network partition heals (validators can communicate again)
PartitionHeal ==
    /\ partitioned # {}  \* There is a partition
    /\ partitioned' = {}
    /\ partitionHealed' = TRUE
    /\ UNCHANGED <<slot, leader, proposed, received, shreds, votesRound1, votesRound2,
                   skipVotes, finalized, skipped, round, time, messages>>

(***************************************************************************)
(* Specification                                                           *)
(***************************************************************************)

Next ==
    \/ ProposeBlock
    \/ ByzantineNoProposal
    \/ DisseminateShreds
    \/ ReconstructBlock
    \/ VoteRound1
    \/ CheckFastQuorum
    \/ AdvanceToRound2
    \/ VoteRound2
    \/ CheckFallbackQuorum
    \/ VoteSkip
    \/ CheckSkipQuorum
    \/ NextSlot
    \/ Tick
    \/ NetworkPartition
    \/ PartitionHeal

Spec == Init /\ [][Next]_vars

\* Fairness constraints for liveness
Fairness ==
    /\ WF_vars(ProposeBlock)
    /\ WF_vars(DisseminateShreds)
    /\ WF_vars(ReconstructBlock)
    /\ WF_vars(VoteRound1)
    /\ WF_vars(VoteRound2)
    /\ WF_vars(CheckFastQuorum)
    /\ WF_vars(CheckFallbackQuorum)
    /\ WF_vars(VoteSkip)
    /\ WF_vars(CheckSkipQuorum)
    /\ WF_vars(NextSlot)
    /\ SF_vars(Tick)

FairSpec == Spec /\ Fairness

(***************************************************************************)
(* Safety Properties (Invariants)                                         *)
(***************************************************************************)

\* SAFETY 1: No two different blocks can be finalized in the same slot
NoFork ==
    \A f1, f2 \in finalized :
        (f1.slot = f2.slot) => (f1.block = f2.block)

\* SAFETY 2: Finalized blocks must have valid quorum
QuorumValidity ==
    \A f \in finalized :
        \/ (f.round = 1 /\ StakeOf(votesRound1[f.block]) >= FastQuorum)
        \/ (f.round = 2 /\ StakeOf(votesRound2[f.block]) >= FallbackQuorum)

\* SAFETY 3: No validator votes twice for different blocks in same round
VotingIntegrity ==
    /\ \A b1, b2 \in Blocks, v \in Validators :
        (b1 # b2 /\ v \in votesRound1[b1]) => v \notin votesRound1[b2]
    /\ \A b1, b2 \in Blocks, v \in Validators :
        (b1 # b2 /\ v \in votesRound2[b1]) => v \notin votesRound2[b2]

\* SAFETY 4: Skip certificates are valid
SkipCertificateValidity ==
    \A s \in skipped :
        StakeOf(skipVotes[s]) >= SkipQuorum

\* SAFETY 5: Can't finalize and skip same slot
NoFinalizeAndSkip ==
    \A f \in finalized :
        f.slot \notin skipped

\* SAFETY 6: Honest validators only vote for blocks they received
VoteImpliesReceipt ==
    \A b \in Blocks, v \in HonestValidators :
        (v \in votesRound1[b] \/ v \in votesRound2[b]) => b \in received[v]

\* SAFETY 7: Blocks can only be reconstructed with enough shreds
ReconstructionValidity ==
    \A v \in HonestValidators, b \in Blocks :
        (b \in received[v]) => (Cardinality(shreds[v][b]) >= MinSredsForReconstruction)

\* SAFETY 8: No fork even during network partition
NoForkDuringPartition ==
    (partitioned # {}) => NoFork

\* SAFETY 9: Safety maintained after partition heals
SafetyAfterHeal ==
    partitionHealed => NoFork

(***************************************************************************)
(* Liveness Properties (for model checking with fairness)                 *)
(***************************************************************************)

\* LIVENESS 1: Every slot eventually finalizes or gets skipped
EventualProgress ==
    \A s \in 0..MaxSlot :
        <>((\E f \in finalized : f.slot = s) \/ s \in skipped)

\* LIVENESS 2: Honest leader proposals eventually finalize
HonestLeaderFinalization ==
    []((\E p \in proposed : p.slot = slot /\ p.leader \in HonestValidators)
        => <>(\E f \in finalized : f.slot = slot))

\* LIVENESS 3: Progress resumes after partition heals
PartitionRecovery ==
    [](partitionHealed => <>(\E s \in 0..MaxSlot :
        s \in skipped \/ \E f \in finalized : f.slot = s))

\* LIVENESS 4: Fast path completes in bounded time (Round1Timeout)
FastPathBoundedTime ==
    \A f \in finalized :
        (f.round = 1) => (f.time <= Round1Timeout)

\* LIVENESS 4: Fallback path completes in bounded time
FallbackPathBoundedTime ==
    \A f \in finalized :
        (f.round = 2) => (f.time <= Round1Timeout + Round2Timeout)

\* LIVENESS 5: No indefinite stall
NoStall ==
    []<>(slot' > slot \/ slot = MaxSlot)

(***************************************************************************)
(* Resilience Properties                                                   *)
(***************************************************************************)

\* Fault assumptions hold
FaultAssumptions ==
    /\ Cardinality(ByzantineSet) <= (20 * TotalStake) \div 100
    /\ Cardinality(OfflineSet) <= (20 * TotalStake) \div 100
    /\ Cardinality(ByzantineSet \cup OfflineSet) <= (40 * TotalStake) \div 100
    /\ Cardinality(HonestValidators) >= (60 * TotalStake) \div 100

\* Safety maintained under fault assumptions
SafetyUnderFaults ==
    FaultAssumptions => []NoFork

\* Liveness maintained with honest leader
LivenessUnderFaults ==
    (FaultAssumptions /\ leader \in HonestValidators) => EventualProgress

(***************************************************************************)
(* Theorems (for TLAPS proof assistant)                                   *)
(***************************************************************************)

THEOREM SafetyTheorem == Spec => []NoFork
THEOREM QuorumTheorem == Spec => []QuorumValidity
THEOREM IntegrityTheorem == Spec => []VotingIntegrity
THEOREM SkipValidityTheorem == Spec => []SkipCertificateValidity
THEOREM BoundedTimeTheorem == FairSpec => []FastPathBoundedTime
THEOREM ProgressTheorem == FairSpec => EventualProgress

=============================================================================
