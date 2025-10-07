------------------------ MODULE Alpenglow ------------------------
(***************************************************************************
 * TLA+ Specification of the Alpenglow Consensus Protocol
 *
 * This module models the Alpenglow consensus protocol as described in the
 * whitepaper v1.1 (July 22, 2025). It includes:
 * - Votor: Dual-path voting mechanism (80% fast, 60% fallback)
 * - Rotor: Abstract data dissemination model
 * - Byzantine and crash fault tolerance (20% + 20%)
 *
 * Author: TLA+ Modeling Agent
 * Date: 2025
 ***************************************************************************)

EXTENDS Naturals, FiniteSets, Sequences, TLC

CONSTANTS
    Validators,      \* Set of all validators
    Blocks,          \* Set of possible blocks
    MaxSlot,         \* Maximum slot number to model
    ByzantineSet,    \* Set of Byzantine (adversarial) validators
    OfflineSet       \* Set of offline (crashed) validators

VARIABLES
    slot,            \* Current slot number
    leader,          \* Current leader for the slot
    proposed,        \* Set of proposed blocks per slot
    received,        \* [validator -> set of received blocks]
    votesRound1,     \* [block -> set of validators who voted in round 1]
    votesRound2,     \* [block -> set of validators who voted in round 2]
    finalized,       \* Set of finalized (block, slot) pairs
    round,           \* Current round within slot (1 or 2)
    messages         \* Set of messages in transit

vars == <<slot, leader, proposed, received, votesRound1, votesRound2,
          finalized, round, messages>>

(***************************************************************************)
(* Type Invariants and Helper Functions                                    *)
(***************************************************************************)

ValidatorStake == [v \in Validators |-> 1]  \* Equal stake for simplicity
TotalStake == Cardinality(Validators)

HonestValidators == Validators \ (ByzantineSet \cup OfflineSet)

StakeOf(vs) == Cardinality(vs)  \* Simplified: each validator = 1 stake

\* Quorum thresholds
FastQuorum == (80 * TotalStake) \div 100       \* 80% for 1-round
FallbackQuorum == (60 * TotalStake) \div 100   \* 60% for 2-round

TypeOK ==
    /\ slot \in 0..MaxSlot
    /\ leader \in Validators
    /\ proposed \subseteq [slot: 0..MaxSlot, block: Blocks, leader: Validators]
    /\ received \in [Validators -> SUBSET Blocks]
    /\ votesRound1 \in [Blocks -> SUBSET Validators]
    /\ votesRound2 \in [Blocks -> SUBSET Validators]
    /\ finalized \subseteq [block: Blocks, slot: 0..MaxSlot]
    /\ round \in {1, 2}

(***************************************************************************)
(* Initial State                                                           *)
(***************************************************************************)

Init ==
    /\ slot = 0
    /\ leader \in HonestValidators  \* Start with honest leader
    /\ proposed = {}
    /\ received = [v \in Validators |-> {}]
    /\ votesRound1 = [b \in Blocks |-> {}]
    /\ votesRound2 = [b \in Blocks |-> {}]
    /\ finalized = {}
    /\ round = 1
    /\ messages = {}

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
    /\ UNCHANGED <<slot, leader, received, votesRound1, votesRound2,
                   finalized, round, messages>>

\* Rotor dissemination: honest validators receive proposed block
DisseminateBlock ==
    /\ \E p \in proposed :
        /\ p.slot = slot
        /\ \E v \in HonestValidators :
            /\ p.block \notin received[v]
            /\ received' = [received EXCEPT ![v] = @ \union {p.block}]
    /\ UNCHANGED <<slot, leader, proposed, votesRound1, votesRound2,
                   finalized, round, messages>>

\* Validator casts round 1 vote (notarization)
VoteRound1 ==
    /\ round = 1
    /\ \E v \in HonestValidators, b \in Blocks :
        /\ b \in received[v]
        /\ v \notin votesRound1[b]  \* Haven't voted for this block yet
        /\ votesRound1' = [votesRound1 EXCEPT ![b] = @ \union {v}]
    /\ UNCHANGED <<slot, leader, proposed, received, votesRound2,
                   finalized, round, messages>>

\* Check if fast path quorum reached (80%)
CheckFastQuorum ==
    /\ round = 1
    /\ \E b \in Blocks :
        /\ StakeOf(votesRound1[b]) >= FastQuorum
        /\ finalized' = finalized \union {[block |-> b, slot |-> slot]}
    /\ UNCHANGED <<slot, leader, proposed, received, votesRound1, votesRound2,
                   round, messages>>

\* Move to round 2 if fast quorum not reached
AdvanceToRound2 ==
    /\ round = 1
    /\ ~\E b \in Blocks : StakeOf(votesRound1[b]) >= FastQuorum
    /\ round' = 2
    /\ UNCHANGED <<slot, leader, proposed, received, votesRound1, votesRound2,
                   finalized, messages>>

\* Validator casts round 2 vote (finalization)
VoteRound2 ==
    /\ round = 2
    /\ \E v \in HonestValidators, b \in Blocks :
        /\ b \in received[v]
        /\ v \notin votesRound2[b]
        /\ votesRound2' = [votesRound2 EXCEPT ![b] = @ \union {v}]
    /\ UNCHANGED <<slot, leader, proposed, received, votesRound1,
                   finalized, round, messages>>

\* Check if fallback quorum reached (60%)
CheckFallbackQuorum ==
    /\ round = 2
    /\ \E b \in Blocks :
        /\ StakeOf(votesRound2[b]) >= FallbackQuorum
        /\ finalized' = finalized \union {[block |-> b, slot |-> slot]}
    /\ UNCHANGED <<slot, leader, proposed, received, votesRound1, votesRound2,
                   round, messages>>

\* Advance to next slot
NextSlot ==
    /\ slot < MaxSlot
    /\ slot' = slot + 1
    /\ leader' \in Validators  \* Leader rotation (can be Byzantine)
    /\ round' = 1
    /\ votesRound1' = [b \in Blocks |-> {}]
    /\ votesRound2' = [b \in Blocks |-> {}]
    /\ UNCHANGED <<proposed, received, finalized, messages>>

(***************************************************************************)
(* Specification                                                           *)
(***************************************************************************)

Next ==
    \/ ProposeBlock
    \/ DisseminateBlock
    \/ VoteRound1
    \/ CheckFastQuorum
    \/ AdvanceToRound2
    \/ VoteRound2
    \/ CheckFallbackQuorum
    \/ NextSlot

Spec == Init /\ [][Next]_vars

(***************************************************************************)
(* Safety Properties (Invariants)                                         *)
(***************************************************************************)

\* SAFETY: No two different blocks can be finalized in the same slot
NoFork ==
    \A f1, f2 \in finalized :
        (f1.slot = f2.slot) => (f1.block = f2.block)

\* If a block is finalized, it must have had sufficient votes
QuorumValidity ==
    \A f \in finalized :
        \/ StakeOf(votesRound1[f.block]) >= FastQuorum
        \/ StakeOf(votesRound2[f.block]) >= FallbackQuorum

\* No validator votes twice for different blocks in same round
VotingIntegrity ==
    /\ \A b1, b2 \in Blocks, v \in Validators :
        (b1 # b2 /\ v \in votesRound1[b1]) => v \notin votesRound1[b2]
    /\ \A b1, b2 \in Blocks, v \in Validators :
        (b1 # b2 /\ v \in votesRound2[b1]) => v \notin votesRound2[b2]

(***************************************************************************)
(* Liveness Properties (for model checking with fairness)                 *)
(***************************************************************************)

\* Eventually, if an honest leader proposes, the block gets finalized
\* (under assumptions: sufficient honest validators)
EventualFinalization ==
    []<>(\E f \in finalized : f.slot = slot)

(***************************************************************************)
(* Fault Tolerance Constraints                                            *)
(***************************************************************************)

FaultAssumptions ==
    /\ Cardinality(ByzantineSet) <= (20 * TotalStake) \div 100
    /\ Cardinality(OfflineSet) <= (20 * TotalStake) \div 100
    /\ Cardinality(ByzantineSet \cup OfflineSet) <= (40 * TotalStake) \div 100

(***************************************************************************)
(* Theorems (for TLAPS proof assistant)                                   *)
(***************************************************************************)

THEOREM SafetyTheorem == Spec => []NoFork
THEOREM QuorumTheorem == Spec => []QuorumValidity

=============================================================================