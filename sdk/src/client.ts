/**
 * ILNClient — entry point for the Invoice Liquidity Network SDK.
 *
 * Provides factory methods for common environments so integrators can
 * get started with a one-liner:
 *
 * ```ts
 * import { ILNClient } from "@iln/sdk";
 *
 * const client = ILNClient.testnet(signer);
 * const reputation = await client.getReputation("G...");
 * ```
 *
 * ## Architecture
 *
 * `ILNClient` is a thin wrapper around the SDK's free functions. It holds
 * the RPC server, network passphrase, contract address, and signer so
 * every method call uses the same configuration automatically.
 */

import { SorobanRpc } from "@stellar/stellar-sdk";
import type { ISigner } from "./signers/ISigner.js";

// ---------------------------------------------------------------------------
// Defaults
// ---------------------------------------------------------------------------

/**
 * Public Soroban RPC endpoint for Stellar Testnet.
 */
export const TESTNET_RPC_URL = "https://soroban-testnet.stellar.org";

/**
 * Public Soroban RPC endpoint for Stellar Mainnet (Pubnet).
 */
export const MAINNET_RPC_URL = "https://soroban.stellar.org";

// ---------------------------------------------------------------------------
// Configuration types
// ---------------------------------------------------------------------------

/** Full configuration for a custom ILNClient. */
export interface ILNClientConfig {
  /** Soroban RPC endpoint URL. */
  rpcUrl: string;
  /** Stellar network passphrase (e.g. `Networks.TESTNET`). */
  networkPassphrase: string;
  /** Deployed invoice-liquidity contract address. */
  contractId: string;
  /**
   * Optional signer for methods that require authentication (e.g. fundInvoice).
   * Read-only methods like getReputation work without a signer.
   */
  signer?: ISigner;
}

// ---------------------------------------------------------------------------
// ILNClient
// ---------------------------------------------------------------------------

/**
 * Configured SDK client bound to a specific network and contract.
 *
 * @example
 * ```ts
 * // Testnet
 * const client = ILNClient.testnet(mySigner);
 *
 * // Custom RPC (e.g. local validator node)
 * const client = ILNClient.custom({
 *   rpcUrl: "http://localhost:8000/soroban/rpc",
 *   networkPassphrase: Networks.STANDALONE,
 *   contractId: "CDEPLOYED...",
 *   signer: mySigner,
 * });
 * ```
 */
export class ILNClient {
  /** Soroban RPC server instance. */
  readonly rpc: SorobanRpc.Server;
  /** Stellar network passphrase. */
  readonly networkPassphrase: string;
  /** Deployed invoice-liquidity contract address. */
  readonly contractId: string;
  /** Optional signer for authenticated methods. */
  readonly signer?: ISigner;

  // Cached imports (lazy-loaded for tree-shaking)
  private _getReputation?: typeof import("./methods/reputation.js").getReputation;
  private _getContractStats?: typeof import("./methods/stats.js").getContractStats;

  constructor(config: ILNClientConfig) {
    this.rpc = new SorobanRpc.Server(config.rpcUrl);
    this.networkPassphrase = config.networkPassphrase;
    this.contractId = config.contractId;
    this.signer = config.signer;
  }

  // --------------------------------------------------------------------------
  // Factory methods
  // --------------------------------------------------------------------------

  /**
   * Create a client pre-configured for Stellar Testnet.
   *
   * @param signer   - Optional signer for authenticated methods
   * @param options  - Override defaults (rpcUrl, contractId)
   *
   * @example
   * ```ts
   * const client = ILNClient.testnet(freighterSigner);
   * ```
   */
  static testnet(
    signer?: ISigner,
    options?: { rpcUrl?: string; contractId?: string }
  ): ILNClient {
    return new ILNClient({
      rpcUrl: options?.rpcUrl ?? TESTNET_RPC_URL,
      networkPassphrase: "Test SDF Network ; September 2015",
      contractId:
        options?.contractId ??
        // Published testnet deployment: the canonical contract ID from
        // the latest testnet CI/CD deployment. Update here when redeploying.
        // TODO: replace with actual testnet contract ID once deployed
        "CD2Q6M76VFLHNHDNROENMX7PJ5OBYBMVPM73S4M6XAJXN3NKCBJQPLUC",
      signer,
    });
  }

  /**
   * Create a client pre-configured for Stellar Mainnet (Pubnet).
   *
   * @param signer   - Optional signer for authenticated methods
   * @param options  - Override defaults (rpcUrl, contractId)
   *
   * @example
   * ```ts
   * const client = ILNClient.mainnet(freighterSigner);
   * ```
   */
  static mainnet(
    signer?: ISigner,
    options?: { rpcUrl?: string; contractId?: string }
  ): ILNClient {
    // Future-proof: we allow configuring mainnet ahead of deployment
    // so integrators can test their integration code against the API shape.
    return new ILNClient({
      rpcUrl: options?.rpcUrl ?? MAINNET_RPC_URL,
      networkPassphrase: "Public Global Stellar Network ; September 2015",
      contractId:
        options?.contractId ??
        // TODO: replace with actual mainnet contract ID after mainnet deployment
        "",
      signer,
    });
  }

  /**
   * Create a client with fully custom configuration.
   *
   * Use this for local development (standalone network), Futurenet, or
   * private Stellar deployments.
   *
   * @param config - Full ILNClientConfig
   */
  static custom(config: ILNClientConfig): ILNClient {
    return new ILNClient(config);
  }

  // --------------------------------------------------------------------------
  // Methods
  // --------------------------------------------------------------------------

  /**
   * Fetch the detailed reputation profile for an address.
   *
   * Read-only; does not require a signer.
   *
   * @param address - Stellar G… address to query
   * @returns ReputationProfile (zeroed for unknown addresses)
   */
  async getReputation(
    address: string
  ): Promise<import("./methods/reputation.js").ReputationProfile> {
    if (!this._getReputation) {
      this._getReputation = (await import("./methods/reputation.js"))
        .getReputation;
    }
    return this._getReputation(this.rpc, this.contractId, address, this.networkPassphrase);
  }

  /**
   * Fetch protocol-wide statistics.
   *
   * Read-only; does not require a signer.
   *
   * @returns ContractStats
   */
  async getContractStats(): Promise<
    import("./methods/stats.js").ContractStats
  > {
    if (!this._getContractStats) {
      this._getContractStats = (await import("./methods/stats.js"))
        .getContractStats;
    }
    return this._getContractStats(this.rpc, this.contractId, this.networkPassphrase);
  }
}

// ---------------------------------------------------------------------------
// Singleton
// ---------------------------------------------------------------------------

/**
 * Default ILNClient singleton.
 *
 * Must be initialised via `iln.configure(...)` before use.
 *
 * @example
 * ```ts
 * import { iln } from "@iln/sdk";
 *
 * iln.configure({ rpcUrl: "...", networkPassphrase: Networks.TESTNET, contractId: "..." });
 * await iln.getReputation("G...");
 * ```
 */
class ILNSingleton {
  private _client: ILNClient | null = null;

  configure(config: ILNClientConfig): void {
    this._client = new ILNClient(config);
  }

  /** Access the underlying client. Throws if not configured. */
  get client(): ILNClient {
    if (!this._client) {
      throw new Error(
        "ILN singleton not configured. Call iln.configure({...}) first."
      );
    }
    return this._client;
  }

  async getReputation(address: string) {
    return this.client.getReputation(address);
  }

  async getContractStats() {
    return this.client.getContractStats();
  }
}

export const iln = new ILNSingleton();
