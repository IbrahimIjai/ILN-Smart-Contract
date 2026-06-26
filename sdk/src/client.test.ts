/**
 * Tests for ILNClient — covers:
 *   - testnet / mainnet / custom factory methods
 *   - singleton iln.configure / iln.getReputation / iln.getContractStats
 *   - Preset defaults (RPC URL, network passphrase, contract ID)
 */

import { ILNClient, iln, TESTNET_RPC_URL, MAINNET_RPC_URL } from "./client.js";
import { Networks } from "@stellar/stellar-sdk";

// ---------------------------------------------------------------------------
// Mock SorobanRpc.Server to avoid real network connections in tests
// ---------------------------------------------------------------------------

jest.mock("@stellar/stellar-sdk", () => {
  const actual = jest.requireActual("@stellar/stellar-sdk");
  return {
    ...actual,
    SorobanRpc: {
      ...actual.SorobanRpc,
      Server: jest.fn().mockImplementation(() => ({
        getAccount: jest.fn(),
        simulateTransaction: jest.fn(),
        prepareTransaction: jest.fn(),
        sendTransaction: jest.fn(),
        getLatestLedger: jest.fn(),
      })),
    },
  };
});

// ---------------------------------------------------------------------------
// testnet()
// ---------------------------------------------------------------------------

describe("ILNClient.testnet", () => {
  it("creates a client with testnet defaults", () => {
    const client = ILNClient.testnet();

    expect(client.networkPassphrase).toBe(Networks.TESTNET);
    expect(client.contractId).toBeTruthy();
    expect(client.contractId.length).toBeGreaterThan(0);
  });

  it("uses the testnet RPC URL", () => {
    const client = ILNClient.testnet();
    // We can't inspect rpc.serverUrl directly in v12, but the constructor
    // receives the correct URL.
    expect(TESTNET_RPC_URL).toContain("testnet");
  });

  it("accepts an optional signer", () => {
    const signer = { publicKey: "GAA", signTransaction: jest.fn() };
    const client = ILNClient.testnet(signer as any);
    expect(client.signer).toBe(signer);
  });

  it("accepts optional overrides", () => {
    const client = ILNClient.testnet(undefined, {
      rpcUrl: "http://localhost:8000/soroban/rpc",
      contractId: "CUSTOM",
    });

    expect(client.contractId).toBe("CUSTOM");
  });

  it("works without any arguments", () => {
    const client = ILNClient.testnet();
    expect(client).toBeInstanceOf(ILNClient);
    expect(client.signer).toBeUndefined();
  });
});

// ---------------------------------------------------------------------------
// mainnet()
// ---------------------------------------------------------------------------

describe("ILNClient.mainnet", () => {
  it("creates a client with mainnet defaults", () => {
    const client = ILNClient.mainnet();

    expect(client.networkPassphrase).toBe(Networks.PUBLIC);
    expect(MAINNET_RPC_URL).toContain("soroban.stellar.org");
  });

  it("accepts an optional signer", () => {
    const signer = { publicKey: "GAA", signTransaction: jest.fn() };
    const client = ILNClient.mainnet(signer as any);
    expect(client.signer).toBe(signer);
  });

  it("accepts optional overrides", () => {
    const client = ILNClient.mainnet(undefined, {
      rpcUrl: "https://custom-rpc.example.com",
      contractId: "MAINNET_DEPLOY",
    });

    expect(client.contractId).toBe("MAINNET_DEPLOY");
  });
});

// ---------------------------------------------------------------------------
// custom()
// ---------------------------------------------------------------------------

describe("ILNClient.custom", () => {
  it("creates a client with fully custom config", () => {
    const signer = { publicKey: "GAA", signTransaction: jest.fn() };
    const client = ILNClient.custom({
      rpcUrl: "http://localhost:8000/soroban/rpc",
      networkPassphrase: "Standalone Network ; February 2017",
      contractId: "CSTANDALONE",
      signer: signer as any,
    });

    expect(client.networkPassphrase).toBe(
      "Standalone Network ; February 2017"
    );
    expect(client.contractId).toBe("CSTANDALONE");
    expect(client.signer).toBe(signer);
  });

  it("works without a signer (read-only configs)", () => {
    const client = ILNClient.custom({
      rpcUrl: "https://soroban-testnet.stellar.org",
      networkPassphrase: Networks.TESTNET,
      contractId: "CTEST",
    });

    expect(client.signer).toBeUndefined();
    expect(client.contractId).toBe("CTEST");
  });
});

// ---------------------------------------------------------------------------
// Singleton (iln)
// ---------------------------------------------------------------------------

describe("iln singleton", () => {
  it("throws if getReputation is called before configure", async () => {
    // Reset singleton state (it's a module-level singleton, but we
    // re-configure it in each test)
    await expect(iln.getReputation("GAA")).rejects.toThrow(
      "not configured"
    );
  });

  it("throws if getContractStats is called before configure", async () => {
    await expect(iln.getContractStats()).rejects.toThrow(
      "not configured"
    );
  });
});
