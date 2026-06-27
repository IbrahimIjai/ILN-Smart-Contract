import {
  Contract,
  SorobanRpc,
  TransactionBuilder,
  BASE_FEE,
  nativeToScVal,
  scValToNative,
  Account,
  Transaction,
} from "@stellar/stellar-sdk";
import { ILNError } from "../errors.js";
import {
  ProposalAction,
  ProposalStatus,
  type Proposal,
  type ProposalFilter,
  type CreateProposalResult,
} from "../types/governance.js";

/**
 * Build, simulate, sign and submit a governance transaction, polling until the
 * network confirms it. Shared by the write methods below.
 */
async function sendGovernanceCall(
  server: SorobanRpc.Server,
  sourceAccount: Account,
  networkPassphrase: string,
  op: ReturnType<Contract["call"]>,
  signTransaction: (tx: Transaction) => Promise<Transaction> | Transaction
): Promise<{ txHash: string; returnValue: unknown }> {
  const tx = new TransactionBuilder(sourceAccount, {
    fee: BASE_FEE,
    networkPassphrase,
  })
    .addOperation(op)
    .setTimeout(30)
    .build();

  const sim = await server.simulateTransaction(tx);
  if (SorobanRpc.Api.isSimulationError(sim)) {
    throw ILNError.fromError(sim.error);
  }

  const assembledTx = SorobanRpc.assembleTransaction(tx, sim).build();
  const signedTx = await signTransaction(assembledTx);
  const sendResult = await server.sendTransaction(signedTx);
  if (sendResult.errorResultXdr) {
    throw new Error(`Transaction failed: ${sendResult.errorResultXdr}`);
  }

  let status = await server.getTransaction(sendResult.hash);
  let retries = 0;
  while (status.status === SorobanRpc.Api.GetTransactionStatus.NOT_FOUND && retries < 15) {
    await new Promise(r => setTimeout(r, 2000));
    status = await server.getTransaction(sendResult.hash);
    retries++;
  }
  if (status.status === SorobanRpc.Api.GetTransactionStatus.FAILED) {
    throw new Error("Transaction failed during execution");
  }

  const returnValue =
    status.status === SorobanRpc.Api.GetTransactionStatus.SUCCESS && status.returnValue
      ? scValToNative(status.returnValue)
      : undefined;

  return { txHash: sendResult.hash, returnValue };
}

/** Normalise a raw contract proposal record into a {@link Proposal}. */
function parseProposal(raw: Record<string, unknown>): Proposal {
  const statusTag =
    (raw["status"] as any)?.tag ?? String(raw["status"]);
  return {
    id: BigInt(String(raw["id"])),
    action: Number(raw["action"]) as ProposalAction,
    proposedValue: BigInt(String(raw["proposed_value"] ?? 0)),
    descriptionHash: raw["description_hash"]
      ? Buffer.from(raw["description_hash"] as any).toString("hex")
      : "",
    proposer: String(raw["proposer"]),
    votesFor: BigInt(String(raw["votes_for"] ?? 0)),
    votesAgainst: BigInt(String(raw["votes_against"] ?? 0)),
    status: (ProposalStatus as any)[statusTag] ?? (statusTag as ProposalStatus),
    votingEndsAt: Number(raw["voting_ends_at"] ?? 0),
  };
}

/**
 * Create a new governance proposal.
 *
 * @param server Soroban RPC server
 * @param contractAddress Governance contract address
 * @param action The parameter-changing action to propose
 * @param proposedValue The proposed new value for the action's parameter
 * @param descriptionHash Hex-encoded 32-byte hash of the off-chain description
 * @param sourceAccount The proposer's account
 * @param signTransaction A function to sign the transaction
 * @param networkPassphrase The network passphrase
 * @returns The new proposalId and txHash
 * @throws {ILNError} When simulation or execution fails
 */
export async function createProposal(
  server: SorobanRpc.Server,
  contractAddress: string,
  action: ProposalAction,
  proposedValue: bigint,
  descriptionHash: string,
  sourceAccount: Account,
  signTransaction: (tx: Transaction) => Promise<Transaction> | Transaction,
  networkPassphrase: string
): Promise<CreateProposalResult> {
  const contract = new Contract(contractAddress);
  const op = contract.call(
    "create_proposal",
    nativeToScVal(sourceAccount.accountId(), { type: "address" }),
    nativeToScVal(action, { type: "u32" }),
    nativeToScVal(proposedValue, { type: "i128" }),
    nativeToScVal(Buffer.from(descriptionHash, "hex"), { type: "bytes", size: 32 })
  );

  const { txHash, returnValue } = await sendGovernanceCall(
    server,
    sourceAccount,
    networkPassphrase,
    op,
    signTransaction
  );

  return {
    proposalId: returnValue !== undefined ? BigInt(String(returnValue)) : 0n,
    txHash,
  };
}

/**
 * Cast a vote on an active proposal.
 *
 * @param support `true` to vote for, `false` to vote against.
 */
export async function castVote(
  server: SorobanRpc.Server,
  contractAddress: string,
  proposalId: bigint,
  support: boolean,
  sourceAccount: Account,
  signTransaction: (tx: Transaction) => Promise<Transaction> | Transaction,
  networkPassphrase: string
): Promise<{ txHash: string }> {
  const contract = new Contract(contractAddress);
  const op = contract.call(
    "cast_vote",
    nativeToScVal(sourceAccount.accountId(), { type: "address" }),
    nativeToScVal(proposalId, { type: "u64" }),
    nativeToScVal(support, { type: "bool" })
  );

  const { txHash } = await sendGovernanceCall(
    server,
    sourceAccount,
    networkPassphrase,
    op,
    signTransaction
  );
  return { txHash };
}

/**
 * Execute a proposal that has passed its vote.
 */
export async function executeProposal(
  server: SorobanRpc.Server,
  contractAddress: string,
  proposalId: bigint,
  sourceAccount: Account,
  signTransaction: (tx: Transaction) => Promise<Transaction> | Transaction,
  networkPassphrase: string
): Promise<{ txHash: string }> {
  const contract = new Contract(contractAddress);
  const op = contract.call(
    "execute_proposal",
    nativeToScVal(sourceAccount.accountId(), { type: "address" }),
    nativeToScVal(proposalId, { type: "u64" })
  );

  const { txHash } = await sendGovernanceCall(
    server,
    sourceAccount,
    networkPassphrase,
    op,
    signTransaction
  );
  return { txHash };
}

/**
 * Fetch a single proposal by ID (read-only; no signer required).
 */
export async function getProposal(
  server: SorobanRpc.Server,
  contractAddress: string,
  id: bigint,
  sourceAccount: Account,
  networkPassphrase: string
): Promise<Proposal> {
  const contract = new Contract(contractAddress);
  const op = contract.call("get_proposal", nativeToScVal(id, { type: "u64" }));

  const tx = new TransactionBuilder(sourceAccount, {
    fee: BASE_FEE,
    networkPassphrase,
  })
    .addOperation(op)
    .setTimeout(30)
    .build();

  const sim = await server.simulateTransaction(tx);
  if (SorobanRpc.Api.isSimulationError(sim)) {
    throw ILNError.fromError(sim.error);
  }
  if (!sim.result?.retval) {
    throw new ILNError(`Proposal ${id} not found`);
  }

  return parseProposal(scValToNative(sim.result.retval) as Record<string, unknown>);
}

/**
 * List proposals, optionally filtered by status and/or proposer (read-only).
 */
export async function listProposals(
  server: SorobanRpc.Server,
  contractAddress: string,
  sourceAccount: Account,
  networkPassphrase: string,
  filter?: ProposalFilter
): Promise<Proposal[]> {
  const contract = new Contract(contractAddress);
  const op = contract.call("list_proposals");

  const tx = new TransactionBuilder(sourceAccount, {
    fee: BASE_FEE,
    networkPassphrase,
  })
    .addOperation(op)
    .setTimeout(30)
    .build();

  const sim = await server.simulateTransaction(tx);
  if (SorobanRpc.Api.isSimulationError(sim)) {
    throw ILNError.fromError(sim.error);
  }
  if (!sim.result?.retval) {
    return [];
  }

  const rawArr = scValToNative(sim.result.retval) as Record<string, unknown>[];
  let proposals = rawArr.map(parseProposal);

  if (filter?.status) {
    proposals = proposals.filter(p => p.status === filter.status);
  }
  if (filter?.proposer) {
    proposals = proposals.filter(p => p.proposer === filter.proposer);
  }
  return proposals;
}
