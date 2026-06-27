import {
  Contract,
  SorobanRpc,
  TransactionBuilder,
  BASE_FEE,
  scValToNative,
  nativeToScVal,
  Account,
} from "@stellar/stellar-sdk";
import type { Invoice } from "../types/invoice.js";
import { ILNError } from "../errors.js";
import { computeEffectiveYieldBps } from "./fundInvoice.js";

/**
 * Fetch a single invoice by its ID.
 * @param server Soroban RPC server instance
 * @param contractAddress The contract's address
 * @param invoiceId The invoice ID
 * @param sourceAccount Account used for simulation (does not consume sequence or fees)
 * @param networkPassphrase The network passphrase
 * @returns The invoice data including computed yield
 */
export async function getInvoice(
  server: SorobanRpc.Server,
  contractAddress: string,
  invoiceId: bigint,
  sourceAccount: Account,
  networkPassphrase: string
): Promise<Invoice> {
  const contract = new Contract(contractAddress);
  const op = contract.call(
    "get_invoice",
    nativeToScVal(invoiceId, { type: "u64" })
  );

  const tx = new TransactionBuilder(sourceAccount, {
    fee: BASE_FEE,
    networkPassphrase,
  })
    .addOperation(op)
    .setTimeout(30)
    .build();

  const sim = await server.simulateTransaction(tx);

  if (SorobanRpc.Api.isSimulationError(sim)) {
    if (String(sim.error).includes("NotFound") || String(sim.error).includes("Error(Contract, 1)")) {
      throw new ILNError.InvoiceNotFound(`Invoice ${invoiceId} not found`);
    }
    throw ILNError.fromError(sim.error);
  }
  if (!sim.result?.retval) {
    throw new ILNError.InvoiceNotFound(`Invoice ${invoiceId} not found`);
  }

  const raw = scValToNative(sim.result.retval) as Record<string, unknown>;
  const dueDate = Number(raw["due_date"]);
  const discountRate = Number(raw["discount_rate"]);

  return {
    id: BigInt(String(raw["id"])),
    freelancer: String(raw["freelancer"]),
    payer: String(raw["payer"]),
    token: String(raw["token"]),
    amount: BigInt(String(raw["amount"])),
    dueDate,
    discountRate,
    status: (raw["status"] as any)?.tag || String(raw["status"]) as any, // handle scval enum
    funder: raw["funder"] ? String(raw["funder"]) : undefined,
    fundedAt: raw["funded_at"] ? Number(raw["funded_at"]) : undefined,
    amountFunded: BigInt(String(raw["amount_funded"])),
    amountPaid: BigInt(String(raw["amount_paid"])),
    referralCode: raw["referral_code"] ? Buffer.from(raw["referral_code"] as any).toString('hex') : undefined,
    submitterReputation: Number(raw["submitter_reputation"]),
    effectiveYieldBps: computeEffectiveYieldBps(discountRate, dueDate),
  };
}

/**
 * List invoices submitted by a specific freelancer address.
 * @param server Soroban RPC server instance
 * @param contractAddress The contract's address
 * @param submitter The freelancer's address
 * @param sourceAccount Account used for simulation
 * @param networkPassphrase The network passphrase
 * @param page The page number (0-indexed)
 * @param pageSize The number of items per page
 * @returns Array of invoices
 */
export async function listInvoicesBySubmitter(
  server: SorobanRpc.Server,
  contractAddress: string,
  submitter: string,
  sourceAccount: Account,
  networkPassphrase: string,
  page: number = 0,
  pageSize: number = 50
): Promise<Invoice[]> {
  const contract = new Contract(contractAddress);
  const op = contract.call(
    "list_invoices_by_submitter",
    nativeToScVal(submitter, { type: "address" }),
    nativeToScVal(page, { type: "u32" }),
    nativeToScVal(pageSize, { type: "u32" })
  );

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
  return rawArr.map(raw => {
    const dueDate = Number(raw["due_date"]);
    const discountRate = Number(raw["discount_rate"]);
    return {
      id: BigInt(String(raw["id"])),
      freelancer: String(raw["freelancer"]),
      payer: String(raw["payer"]),
      token: String(raw["token"]),
      amount: BigInt(String(raw["amount"])),
      dueDate,
      discountRate,
      status: (raw["status"] as any)?.tag || String(raw["status"]) as any,
      funder: raw["funder"] ? String(raw["funder"]) : undefined,
      fundedAt: raw["funded_at"] ? Number(raw["funded_at"]) : undefined,
      amountFunded: BigInt(String(raw["amount_funded"])),
      amountPaid: BigInt(String(raw["amount_paid"])),
      referralCode: raw["referral_code"] ? Buffer.from(raw["referral_code"] as any).toString('hex') : undefined,
      submitterReputation: Number(raw["submitter_reputation"]),
      effectiveYieldBps: computeEffectiveYieldBps(discountRate, dueDate),
    };
  });
}

/**
 * List invoices funded by a specific LP address.
 * @param server Soroban RPC server instance
 * @param contractAddress The contract's address
 * @param lp The liquidity provider's address
 * @param sourceAccount Account used for simulation
 * @param networkPassphrase The network passphrase
 * @param page The page number (0-indexed)
 * @param pageSize The number of items per page
 * @returns Array of invoices
 */
export async function listInvoicesByLP(
  server: SorobanRpc.Server,
  contractAddress: string,
  lp: string,
  sourceAccount: Account,
  networkPassphrase: string,
  page: number = 0,
  pageSize: number = 50
): Promise<Invoice[]> {
  const contract = new Contract(contractAddress);
  const op = contract.call(
    "list_invoices_by_lp",
    nativeToScVal(lp, { type: "address" }),
    nativeToScVal(page, { type: "u32" }),
    nativeToScVal(pageSize, { type: "u32" })
  );

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
  return rawArr.map(raw => {
    const dueDate = Number(raw["due_date"]);
    const discountRate = Number(raw["discount_rate"]);
    return {
      id: BigInt(String(raw["id"])),
      freelancer: String(raw["freelancer"]),
      payer: String(raw["payer"]),
      token: String(raw["token"]),
      amount: BigInt(String(raw["amount"])),
      dueDate,
      discountRate,
      status: (raw["status"] as any)?.tag || String(raw["status"]) as any,
      funder: raw["funder"] ? String(raw["funder"]) : undefined,
      fundedAt: raw["funded_at"] ? Number(raw["funded_at"]) : undefined,
      amountFunded: BigInt(String(raw["amount_funded"])),
      amountPaid: BigInt(String(raw["amount_paid"])),
      referralCode: raw["referral_code"] ? Buffer.from(raw["referral_code"] as any).toString('hex') : undefined,
      submitterReputation: Number(raw["submitter_reputation"]),
      effectiveYieldBps: computeEffectiveYieldBps(discountRate, dueDate),
    };
  });
}
