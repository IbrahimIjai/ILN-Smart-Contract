import {
  createProposal,
  castVote,
  executeProposal,
  getProposal,
  listProposals,
} from "./governance.js";
import { ProposalAction, ProposalStatus } from "../types/governance.js";
import { Account, SorobanRpc, scValToNative } from "@stellar/stellar-sdk";

jest.mock("@stellar/stellar-sdk", () => {
  const actual = jest.requireActual("@stellar/stellar-sdk");
  return { ...actual, scValToNative: jest.fn() };
});

const PROPOSER = "GA6V6P6Z7U2N4KHTD6Y3Y3V7H2P6XZY3H2P6XZY3H2P6XZY3H2P6XZ";
const CONTRACT = "CA6V6P6Z7U2N4KHTD6Y3Y3V7H2P6XZY3H2P6XZY3H2P6XZY3H2P6XZ";
const PASS = "Test SDF Network ; September 2015";
const HASH32 = "ab".repeat(32);

const mockScValToNative = scValToNative as unknown as jest.Mock;

describe("governance", () => {
  const mockServer = {
    simulateTransaction: jest.fn(),
    sendTransaction: jest.fn(),
    getTransaction: jest.fn(),
  } as unknown as SorobanRpc.Server;

  const account = new Account(PROPOSER, "1");
  const sign = jest.fn((tx) => tx);

  beforeEach(() => {
    jest.clearAllMocks();
    // @ts-ignore
    SorobanRpc.assembleTransaction = jest.fn(() => ({ build: () => ({}) }));
    // @ts-ignore
    mockServer.simulateTransaction.mockResolvedValue({ result: { retval: {} } });
    // @ts-ignore
    mockServer.sendTransaction.mockResolvedValue({ status: "PENDING", hash: "txGOV" });
    // @ts-ignore
    mockServer.getTransaction.mockResolvedValue({
      status: SorobanRpc.Api.GetTransactionStatus.SUCCESS,
      returnValue: {},
    });
  });

  it("createProposal returns the new proposalId and txHash", async () => {
    mockScValToNative.mockReturnValue(5n);
    const res = await createProposal(
      mockServer, CONTRACT, ProposalAction.UpdateProtocolFee, 250n, HASH32, account, sign, PASS
    );
    expect(res).toEqual({ proposalId: 5n, txHash: "txGOV" });
    expect(sign).toHaveBeenCalled();
  });

  it("castVote submits and returns txHash", async () => {
    const res = await castVote(mockServer, CONTRACT, 5n, true, account, sign, PASS);
    expect(res.txHash).toBe("txGOV");
  });

  it("executeProposal submits and returns txHash", async () => {
    const res = await executeProposal(mockServer, CONTRACT, 5n, account, sign, PASS);
    expect(res.txHash).toBe("txGOV");
  });

  it("getProposal parses a raw proposal record", async () => {
    mockScValToNative.mockReturnValue({
      id: "5",
      action: 0,
      proposed_value: "250",
      proposer: PROPOSER,
      votes_for: "10",
      votes_against: "2",
      status: { tag: "Active" },
      voting_ends_at: 1700,
    });
    const p = await getProposal(mockServer, CONTRACT, 5n, account, PASS);
    expect(p.id).toBe(5n);
    expect(p.action).toBe(ProposalAction.UpdateProtocolFee);
    expect(p.votesFor).toBe(10n);
    expect(p.status).toBe(ProposalStatus.Active);
  });

  it("listProposals filters by status", async () => {
    mockScValToNative.mockReturnValue([
      { id: "1", action: 0, proposer: PROPOSER, status: { tag: "Active" } },
      { id: "2", action: 0, proposer: PROPOSER, status: { tag: "Executed" } },
    ]);
    const active = await listProposals(mockServer, CONTRACT, account, PASS, {
      status: ProposalStatus.Active,
    });
    expect(active).toHaveLength(1);
    expect(active[0].id).toBe(1n);
  });
});
