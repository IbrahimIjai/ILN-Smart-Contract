/**
 * Governance types for the ILN SDK.
 *
 * Mirrors the on-chain governance contract's proposal model so TypeScript
 * integrators can create proposals, vote and execute without hand-rolling
 * Soroban calls.
 */

/**
 * The set of parameter-changing actions a proposal can request.
 *
 * The numeric values match the contract's `ProposalAction` enum discriminants
 * and are used directly as the `u32` argument when encoding the call.
 */
export enum ProposalAction {
  /** Update the protocol fee, in basis points. */
  UpdateProtocolFee = 0,
  /** Update the minimum payer reputation required to submit an invoice. */
  UpdateMinReputation = 1,
  /** Update the oracle contract address (value is an address index/handle). */
  UpdateOracle = 2,
  /** Pause the contract (proposedValue ignored). */
  PauseContract = 3,
  /** Unpause the contract (proposedValue ignored). */
  UnpauseContract = 4,
  /** Update the default grace period, in seconds. */
  UpdateGracePeriod = 5,
}

/** Lifecycle status of a proposal. */
export enum ProposalStatus {
  Active = "Active",
  Passed = "Passed",
  Rejected = "Rejected",
  Executed = "Executed",
}

/** A governance proposal as returned by the contract. */
export interface Proposal {
  /** Unique proposal identifier. */
  id: bigint;
  /** The parameter-changing action this proposal requests. */
  action: ProposalAction;
  /** The proposed new value for the action's parameter. */
  proposedValue: bigint;
  /** Hex-encoded 32-byte hash of the off-chain proposal description. */
  descriptionHash: string;
  /** Address that created the proposal. */
  proposer: string;
  /** Total weight of votes in support. */
  votesFor: bigint;
  /** Total weight of votes against. */
  votesAgainst: bigint;
  /** Current lifecycle status. */
  status: ProposalStatus;
  /** Unix timestamp (seconds) when voting closes. */
  votingEndsAt: number;
}

/** Optional filter for {@link listProposals}. */
export interface ProposalFilter {
  /** Only return proposals in this status. */
  status?: ProposalStatus;
  /** Only return proposals created by this address. */
  proposer?: string;
}

/** Result of creating a proposal. */
export interface CreateProposalResult {
  proposalId: bigint;
  txHash: string;
}
