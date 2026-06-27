import { transferLPPosition } from "./transferLPPosition.js";
import { ILNError } from "../errors.js";
import { Account, SorobanRpc } from "@stellar/stellar-sdk";

const CURRENT_LP = "GA6V6P6Z7U2N4KHTD6Y3Y3V7H2P6XZY3H2P6XZY3H2P6XZY3H2P6XZ";
const NEW_LP = "GB6V6P6Z7U2N4KHTD6Y3Y3V7H2P6XZY3H2P6XZY3H2P6XZY3H2P6XZ";
const CONTRACT = "CA6V6P6Z7U2N4KHTD6Y3Y3V7H2P6XZY3H2P6XZY3H2P6XZY3H2P6XZ";
const PASS = "Test SDF Network ; September 2015";

describe("transferLPPosition", () => {
  const mockServer = {
    simulateTransaction: jest.fn(),
    sendTransaction: jest.fn(),
    getTransaction: jest.fn(),
  } as unknown as SorobanRpc.Server;

  const account = new Account(CURRENT_LP, "1");
  const sign = jest.fn((tx) => tx);

  beforeEach(() => {
    jest.clearAllMocks();
  });

  it("transfers the position successfully", async () => {
    // @ts-ignore
    mockServer.simulateTransaction.mockResolvedValue({ result: { retval: {} } });
    // @ts-ignore
    SorobanRpc.assembleTransaction = jest.fn(() => ({ build: () => ({}) }));
    // @ts-ignore
    mockServer.sendTransaction.mockResolvedValue({ status: "PENDING", hash: "txABC" });
    // @ts-ignore
    mockServer.getTransaction.mockResolvedValue({ status: SorobanRpc.Api.GetTransactionStatus.SUCCESS });

    const res = await transferLPPosition(mockServer, CONTRACT, 7n, NEW_LP, account, sign, PASS);
    expect(res.txHash).toBe("txABC");
    expect(sign).toHaveBeenCalled();
  });

  it("throws InvalidTransfer when newLP equals the current LP", async () => {
    await expect(
      transferLPPosition(mockServer, CONTRACT, 7n, CURRENT_LP, account, sign, PASS)
    ).rejects.toThrow(ILNError.InvalidTransfer);
  });

  it("throws InvalidAddress when newLP is malformed", async () => {
    await expect(
      transferLPPosition(mockServer, CONTRACT, 7n, "GBAD", account, sign, PASS)
    ).rejects.toThrow(ILNError.InvalidAddress);
  });
});
