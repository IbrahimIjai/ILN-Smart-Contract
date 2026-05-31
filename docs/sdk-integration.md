# SDK Integration Guide

Practical TypeScript examples for every major interaction with the ILN contract on Stellar.
All examples target the **testnet** deployment and use the
[`@stellar/stellar-sdk`](https://www.npmjs.com/package/@stellar/stellar-sdk) package.

---

## Prerequisites

```bash
npm install @stellar/stellar-sdk
```

```ts
import {
  Contract,
  Keypair,
  Networks,
  SorobanRpc,
  TransactionBuilder,
  nativeToScVal,
  scValToNative,
  xdr,
  Address,
} from "@stellar/stellar-sdk";

// ── Testnet constants ────────────────────────────────────────────────────────
const RPC_URL        = "https://soroban-testnet.stellar.org";
const CONTRACT_ID    = "CD3TE3IAHM737P236XZL2OYU275ZKD6MN7YH7PYYAXYIGEH55OPEWYJC";
const USDC_TOKEN     = "CBIELTK6YBZJU5UP2WWQEUCYKLPU6AUNZ2BQ4WWFEIE3USCIHMXQDAMA"; // testnet USDC SAC
const NETWORK_PHRASE = Networks.TESTNET;
const BASE_FEE       = "100";

const server = new SorobanRpc.Server(RPC_URL);

// Helper: sign, simulate, and submit a transaction
async function invoke(
  caller: Keypair,
  method: string,
  args: xdr.ScVal[]
): Promise<xdr.ScVal> {
  const account  = await server.getAccount(caller.publicKey());
  const contract = new Contract(CONTRACT_ID);

  const tx = new TransactionBuilder(account, {
    fee: BASE_FEE,
    networkPassphrase: NETWORK_PHRASE,
  })
    .addOperation(contract.call(method, ...args))
    .setTimeout(30)
    .build();

  // Simulate to get the footprint and resource fee
  const sim = await server.simulateTransaction(tx);
  if (SorobanRpc.Api.isSimulationError(sim)) {
    throw new Error(`Simulation failed: ${sim.error}`);
  }

  const prepared = SorobanRpc.assembleTransaction(tx, sim).build();
  prepared.sign(caller);

  const response = await server.sendTransaction(prepared);
  if (response.status === "ERROR") {
    throw new Error(`Submit failed: ${JSON.stringify(response.errorResult)}`);
  }

  // Poll until confirmed
  let result = await server.getTransaction(response.hash);
  while (result.status === "NOT_FOUND") {
    await new Promise((r) => setTimeout(r, 1000));
    result = await server.getTransaction(response.hash);
  }

  if (result.status !== "SUCCESS") {
    throw new Error(`Transaction failed: ${result.status}`);
  }

  return result.returnValue ?? xdr.ScVal.scvVoid();
}
```

---

## 1. Submit an invoice

Called by the **freelancer** to register an unpaid invoice on-chain.

```ts
/**
 * submit_invoice(freelancer, payer, amount, due_date, discount_rate, token)
 *
 * @param freelancer    - Keypair of the invoice submitter
 * @param payerAddress  - Stellar address of the payer (client)
 * @param amountUsdc    - Invoice value in USDC (e.g. 500.00 → pass 500_000_000 stroops)
 * @param dueDateUnix   - Unix timestamp of the payment due date
 * @param discountBps   - Discount rate in basis points (e.g. 300 = 3 %)
 * @returns             - The new invoice ID (u64)
 */
async function submitInvoice(
  freelancer: Keypair,
  payerAddress: string,
  amountUsdc: bigint,
  dueDateUnix: number,
  discountBps: number
): Promise<bigint> {
  const args = [
    new Address(freelancer.publicKey()).toScVal(),   // freelancer
    new Address(payerAddress).toScVal(),             // payer
    nativeToScVal(amountUsdc, { type: "i128" }),     // amount (stroops)
    nativeToScVal(dueDateUnix, { type: "u64" }),     // due_date
    nativeToScVal(discountBps, { type: "u32" }),     // discount_rate
    new Address(USDC_TOKEN).toScVal(),               // token
  ];

  const result = await invoke(freelancer, "submit_invoice", args);
  return scValToNative(result) as bigint;
}

// Usage
const freelancer = Keypair.fromSecret("S...");
const invoiceId  = await submitInvoice(
  freelancer,
  "GPAYER...",
  500_000_000n,                          // 500 USDC (6 decimals)
  Math.floor(Date.now() / 1000) + 86400, // due in 24 h
  300                                    // 3 % discount
);
console.log("Invoice ID:", invoiceId);
```

**Key constraints**
- `amount` must be ≥ 1 000 000 (1 USDC minimum).
- `due_date` must be at least 24 hours in the future and no more than 365 days out.
- `discount_rate` must be between 1 and the contract's `MaxDiscountRate` (default 5 000 bps).
- `token` must be on the contract's approved token list.

---

## 2. Fund an invoice

Called by a **liquidity provider** to advance capital against a pending invoice.
The LP pays `amount − discount` and the freelancer receives that amount immediately.

```ts
/**
 * fund_invoice(funder, invoice_id, fund_amount)
 *
 * @param lp          - Keypair of the liquidity provider
 * @param invoiceId   - ID returned by submit_invoice
 * @param fundAmount  - Amount to fund in stroops (must equal invoice.amount for full funding)
 */
async function fundInvoice(
  lp: Keypair,
  invoiceId: bigint,
  fundAmount: bigint
): Promise<void> {
  const args = [
    new Address(lp.publicKey()).toScVal(),       // funder
    nativeToScVal(invoiceId, { type: "u64" }),   // invoice_id
    nativeToScVal(fundAmount, { type: "i128" }), // fund_amount
  ];

  await invoke(lp, "fund_invoice", args);
  console.log(`Invoice ${invoiceId} funded with ${fundAmount} stroops`);
}

// Usage — fund the full invoice amount
const lp = Keypair.fromSecret("S...");
await fundInvoice(lp, invoiceId, 500_000_000n);
```

**What happens on-chain**
1. LP transfers `amount × (1 − discount_rate / 10 000)` to the contract.
2. Contract immediately forwards that amount to the freelancer.
3. Invoice status transitions `Pending → Funded`.

> **Partial funding** is supported. Call `fund_invoice` multiple times with smaller
> amounts until `amount_funded == amount`. Status will be `PartiallyFunded` until full.

---

## 3. Mark an invoice paid

Called by the **payer** to settle the invoice. The contract releases funds to the LP.

```ts
/**
 * mark_paid(payer, invoice_id, amount)
 *
 * @param payer      - Keypair of the payer (must match invoice.payer)
 * @param invoiceId  - Invoice to settle
 * @param amount     - Amount being paid now in stroops (can be partial)
 */
async function markPaid(
  payer: Keypair,
  invoiceId: bigint,
  amount: bigint
): Promise<void> {
  const args = [
    nativeToScVal(invoiceId, { type: "u64" }),  // invoice_id
    nativeToScVal(amount, { type: "i128" }),    // amount
  ];

  await invoke(payer, "mark_paid", args);
  console.log(`Invoice ${invoiceId} marked paid`);
}

// Usage — full settlement
const payer = Keypair.fromSecret("S...");
await markPaid(payer, invoiceId, 500_000_000n);
```

**What happens on-chain**
1. Payer transfers `amount` to the contract.
2. Contract distributes proportionally to all funders (principal + discount yield).
3. Invoice status transitions `Funded → Paid`.
4. Payer's on-chain reputation score increments by 1.

> Partial payments are accepted. The invoice stays `Funded` until `amount_paid == amount`.

---

## 4. Query an invoice

Read-only — no signing required.

```ts
/**
 * get_invoice(invoice_id) → Invoice
 *
 * Returns the full invoice struct including status, amounts, and parties.
 */
async function getInvoice(invoiceId: bigint): Promise<Record<string, unknown>> {
  const account  = await server.getAccount(Keypair.random().publicKey()); // throwaway
  const contract = new Contract(CONTRACT_ID);

  const tx = new TransactionBuilder(account, {
    fee: BASE_FEE,
    networkPassphrase: NETWORK_PHRASE,
  })
    .addOperation(
      contract.call(
        "get_invoice",
        nativeToScVal(invoiceId, { type: "u64" })
      )
    )
    .setTimeout(30)
    .build();

  const sim = await server.simulateTransaction(tx);
  if (SorobanRpc.Api.isSimulationError(sim)) {
    throw new Error(`Simulation failed: ${sim.error}`);
  }

  return scValToNative(sim.result!.retval) as Record<string, unknown>;
}

// Usage
const invoice = await getInvoice(invoiceId);
console.log(invoice);
/*
{
  id:                 1n,
  freelancer:         "GFREELANCER...",
  payer:              "GPAYER...",
  token:              "CUSDC...",
  amount:             500000000n,
  due_date:           1748700000n,
  discount_rate:      300,
  status:             { tag: "Funded" },
  funder:             { tag: "Some", values: ["GLP..."] },
  funded_at:          { tag: "Some", values: [1748613600n] },
  amount_funded:      500000000n,
  amount_paid:        0n,
  submitter_reputation: 0
}
*/
```

**Invoice status values:** `Pending` · `PartiallyFunded` · `Funded` · `Paid` · `Defaulted` · `Appealed` · `Disputed` · `Expired` · `Cancelled`

---

## 5. Query contract stats

Returns aggregate protocol metrics — no signing required.

```ts
/**
 * get_contract_stats() → ContractStats
 *
 * Returns total invoices, total funded, total paid, and volume by token.
 */
async function getContractStats(): Promise<Record<string, unknown>> {
  const account  = await server.getAccount(Keypair.random().publicKey());
  const contract = new Contract(CONTRACT_ID);

  const tx = new TransactionBuilder(account, {
    fee: BASE_FEE,
    networkPassphrase: NETWORK_PHRASE,
  })
    .addOperation(contract.call("get_contract_stats"))
    .setTimeout(30)
    .build();

  const sim = await server.simulateTransaction(tx);
  if (SorobanRpc.Api.isSimulationError(sim)) {
    throw new Error(`Simulation failed: ${sim.error}`);
  }

  return scValToNative(sim.result!.retval) as Record<string, unknown>;
}

// Usage
const stats = await getContractStats();
console.log(stats);
/*
{
  total_invoices: 42n,
  total_funded:   38n,
  total_paid:     31n,
  volume:         [ ["CUSDC...", 19500000000n] ]
}
*/
```

---

## 6. Query reputation

```ts
/**
 * get_reputation(address) → ReputationProfile
 *
 * Returns the detailed on-chain reputation profile for any address.
 */
async function getReputation(address: string): Promise<Record<string, unknown>> {
  const account  = await server.getAccount(Keypair.random().publicKey());
  const contract = new Contract(CONTRACT_ID);

  const tx = new TransactionBuilder(account, {
    fee: BASE_FEE,
    networkPassphrase: NETWORK_PHRASE,
  })
    .addOperation(
      contract.call("get_reputation", new Address(address).toScVal())
    )
    .setTimeout(30)
    .build();

  const sim = await server.simulateTransaction(tx);
  if (SorobanRpc.Api.isSimulationError(sim)) {
    throw new Error(`Simulation failed: ${sim.error}`);
  }

  return scValToNative(sim.result!.retval) as Record<string, unknown>;
}

// Usage
const profile = await getReputation("GPAYER...");
console.log(profile);
/*
{
  address:              "GPAYER...",
  score:                12,
  invoices_submitted:   0,
  invoices_paid:        12,
  invoices_defaulted:   1
}
*/
```

You can also fetch the raw numeric score or the suggested discount rate for a payer:

```ts
// payer_score(payer) → u32
async function payerScore(address: string): Promise<number> {
  const account  = await server.getAccount(Keypair.random().publicKey());
  const contract = new Contract(CONTRACT_ID);
  const tx = new TransactionBuilder(account, { fee: BASE_FEE, networkPassphrase: NETWORK_PHRASE })
    .addOperation(contract.call("payer_score", new Address(address).toScVal()))
    .setTimeout(30)
    .build();
  const sim = await server.simulateTransaction(tx);
  if (SorobanRpc.Api.isSimulationError(sim)) throw new Error(sim.error);
  return scValToNative(sim.result!.retval) as number;
}

// suggested_discount_rate(payer) → u32  (basis points)
async function suggestedDiscountRate(address: string): Promise<number> {
  const account  = await server.getAccount(Keypair.random().publicKey());
  const contract = new Contract(CONTRACT_ID);
  const tx = new TransactionBuilder(account, { fee: BASE_FEE, networkPassphrase: NETWORK_PHRASE })
    .addOperation(contract.call("suggested_discount_rate", new Address(address).toScVal()))
    .setTimeout(30)
    .build();
  const sim = await server.simulateTransaction(tx);
  if (SorobanRpc.Api.isSimulationError(sim)) throw new Error(sim.error);
  return scValToNative(sim.result!.retval) as number;
}
```

---

## 7. List invoices by submitter (paginated)

```ts
/**
 * list_invoices_by_submitter(submitter, page, page_size) → Invoice[]
 *
 * page_size is capped at 50 by the contract.
 */
async function listInvoicesBySubmitter(
  address: string,
  page = 0,
  pageSize = 10
): Promise<unknown[]> {
  const account  = await server.getAccount(Keypair.random().publicKey());
  const contract = new Contract(CONTRACT_ID);

  const tx = new TransactionBuilder(account, { fee: BASE_FEE, networkPassphrase: NETWORK_PHRASE })
    .addOperation(
      contract.call(
        "list_invoices_by_submitter",
        new Address(address).toScVal(),
        nativeToScVal(page, { type: "u32" }),
        nativeToScVal(pageSize, { type: "u32" })
      )
    )
    .setTimeout(30)
    .build();

  const sim = await server.simulateTransaction(tx);
  if (SorobanRpc.Api.isSimulationError(sim)) throw new Error(sim.error);
  return scValToNative(sim.result!.retval) as unknown[];
}
```

---

## Amount conventions

| Token | Decimals | 1 unit in stroops |
|-------|----------|-------------------|
| USDC  | 6        | `1_000_000`       |
| XLM   | 7        | `10_000_000`      |

All amounts passed to and returned from the contract are in **stroops** (the token's smallest unit). Never pass floating-point values.

```ts
// Convert human-readable USDC to stroops
const toUsdcStroops = (usdc: number): bigint => BigInt(Math.round(usdc * 1_000_000));

// Convert stroops back to USDC
const fromUsdcStroops = (stroops: bigint): number => Number(stroops) / 1_000_000;
```

---

## Error handling

The contract returns typed errors. After `scValToNative` they surface as objects with a `tag` field:

```ts
// Wrap invoke() to surface contract errors cleanly
async function safeInvoke(caller: Keypair, method: string, args: xdr.ScVal[]) {
  try {
    return await invoke(caller, method, args);
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    // Soroban encodes contract errors as "Error(contract, N)"
    const match = msg.match(/Error\(contract, (\d+)\)/);
    if (match) {
      const code = parseInt(match[1], 10);
      const CONTRACT_ERRORS: Record<number, string> = {
        1:  "AlreadyInitialized",
        2:  "InvoiceNotFound",
        3:  "AlreadyFunded",
        4:  "AlreadyPaid",
        5:  "NotFunded",
        6:  "NotYetDefaulted",
        7:  "Unauthorized",
        8:  "InvalidAmount",
        9:  "InvalidDiscountRate",
        10: "InvalidDueDate",
        11: "OverfundingRejected",
        12: "OverpaymentRejected",
        13: "InvoiceDefaulted",
        14: "ContractPaused",
        15: "SelfInvoice",
        16: "DueDateTooSoon",
        17: "DueDateTooFar",
        18: "PayerReputationTooLow",
        19: "NotApprovedFunder",
        20: "AlreadyInQueue",
        21: "BatchTooLarge",
      };
      throw new Error(`Contract error ${code}: ${CONTRACT_ERRORS[code] ?? "Unknown"}`);
    }
    throw err;
  }
}
```

---

## Testing against testnet

Fund your testnet accounts with Friendbot before running any examples:

```bash
# Fund an account on testnet
curl "https://friendbot.stellar.org?addr=<YOUR_PUBLIC_KEY>"
```

You also need testnet USDC. Mint it from the testnet USDC SAC admin or use the
[Stellar Laboratory](https://laboratory.stellar.org) to set a trustline and receive tokens.

```ts
// Quick smoke test — submit → fund → pay → verify
async function smokeTest() {
  const freelancer = Keypair.random();
  const lp         = Keypair.random();
  const payer      = Keypair.random();

  // Fund accounts via Friendbot
  for (const kp of [freelancer, lp, payer]) {
    await fetch(`https://friendbot.stellar.org?addr=${kp.publicKey()}`);
  }

  const due = Math.floor(Date.now() / 1000) + 2 * 86400; // 2 days out

  const id = await submitInvoice(freelancer, payer.publicKey(), 10_000_000n, due, 300);
  console.log("Submitted:", id);

  await fundInvoice(lp, id, 10_000_000n);
  console.log("Funded");

  await markPaid(payer, id, 10_000_000n);
  console.log("Paid");

  const inv = await getInvoice(id);
  console.assert((inv.status as { tag: string }).tag === "Paid", "Expected Paid status");
  console.log("Smoke test passed ✓");
}
```
