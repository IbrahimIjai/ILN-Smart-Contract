/**
 * getContractStats — fetch protocol-wide statistics from the contract.
 *
 * Reads the single `get_contract_stats()` view call. No signer or
 * transaction fees required (read-only simulation).
 */

import {
  Contract,
  SorobanRpc,
  TransactionBuilder,
  Account,
  BASE_FEE,
  scValToNative,
  Networks,
} from "@stellar/stellar-sdk";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/**
 * Protocol-wide statistics returned by `get_contract_stats()`.
 *
 * Mirrors `ContractStats` in the Rust contract.
 */
export interface ContractStats {
  /** Total number of invoices ever created. */
  totalInvoices: bigint;
  /** Cumulative number of fully-funded invoices. */
  totalFunded: bigint;
  /** Cumulative number of paid invoices. */
  totalPaid: bigint;
  /** Total USDC volume (in stroops, 6 decimals). */
  totalVolumeUsdc: bigint;
  /** Total EURC volume (in stroops, 6 decimals). */
  totalVolumeEurc: bigint;
  /** Total XLM volume (in stroops, 7 decimals). */
  totalVolumeXlm: bigint;
  /** Per-token volume map: token address → volume. */
  volumeByToken: Record<string, bigint>;
  /** Total volume normalized to USD (depends on oracle price feed). */
  totalVolumeUsdNormalized: bigint;
}

// ---------------------------------------------------------------------------
// getContractStats
// ---------------------------------------------------------------------------

/**
 * Query protocol-wide statistics from the contract.
 *
 * Read-only — no signer, no fees, no on-chain mutation.
 *
 * @param server              - Soroban RPC server for the target network
 * @param contractId          - Deployed invoice-liquidity contract address
 * @param networkPassphrase   - Stellar network passphrase (default: TESTNET)
 * @returns ContractStats
 *
 * @throws When the Soroban simulation fails (RPC unreachable, contract not found)
 *
 * @example
 * ```ts
 * const stats = await getContractStats(server, CONTRACT_ID);
 * console.log(`Total invoices: ${stats.totalInvoices}`);
 * console.log(`USDC volume:    ${stats.totalVolumeUsdc}`);
 * ```
 */
export async function getContractStats(
  server: SorobanRpc.Server,
  contractId: string,
  networkPassphrase: string = Networks.TESTNET
): Promise<ContractStats> {
  const contract = new Contract(contractId);
  const op = contract.call("get_contract_stats");

  const sourceAccount = new Account(
    "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
    "0"
  );

  const simTx = new TransactionBuilder(sourceAccount, {
    fee: BASE_FEE,
    networkPassphrase,
  })
    .addOperation(op)
    .setTimeout(30)
    .build();

  const sim = await server.simulateTransaction(simTx);

  if (SorobanRpc.Api.isSimulationError(sim)) {
    throw new Error(`get_contract_stats simulation failed: ${sim.error}`);
  }

  if (!sim.result?.retval) {
    return {
      totalInvoices: 0n,
      totalFunded: 0n,
      totalPaid: 0n,
      totalVolumeUsdc: 0n,
      totalVolumeEurc: 0n,
      totalVolumeXlm: 0n,
      volumeByToken: {},
      totalVolumeUsdNormalized: 0n,
    };
  }

  const raw = scValToNative(sim.result.retval) as Record<string, unknown>;

  // Parse per-token volumes: the contract returns a Vec<(Address, i128)>
  const volumeByToken: Record<string, bigint> = {};
  const rawTokenVolumes = raw["token_volumes"] as Array<[string, string]> | undefined;
  if (Array.isArray(rawTokenVolumes)) {
    for (const [token, volume] of rawTokenVolumes) {
      volumeByToken[token] = BigInt(volume);
    }
  }

  return {
    totalInvoices: BigInt(String(raw["total_invoices"] ?? "0")),
    totalFunded: BigInt(String(raw["total_funded"] ?? "0")),
    totalPaid: BigInt(String(raw["total_paid"] ?? "0")),
    totalVolumeUsdc: BigInt(String(raw["total_volume_usdc"] ?? "0")),
    totalVolumeEurc: BigInt(String(raw["total_volume_eurc"] ?? "0")),
    totalVolumeXlm: BigInt(String(raw["total_volume_xlm"] ?? "0")),
    volumeByToken,
    totalVolumeUsdNormalized: BigInt(
      String(raw["total_volume_usd_normalized"] ?? "0")
    ),
  };
}
