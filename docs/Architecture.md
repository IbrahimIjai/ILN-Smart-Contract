# ILN Technical Architecture

> **Design rationale:** The major design choices behind this architecture are
> documented as Architecture Decision Records in [`docs/adr/`](adr/README.md).

## Overview

Invoice Liquidity Network is a two-sided protocol on Stellar that connects invoice holders (freelancers, SMEs) with liquidity providers (DeFi users). The core logic lives in a single Soroban smart contract that acts as a trustless escrow — holding funds, enforcing payment terms, and releasing money automatically based on on-chain state.

There is no backend server. There is no admin key. All state transitions are triggered by the three parties involved in each invoice: the freelancer, the liquidity provider, and the payer.

---

## System actors

|             Actor           |       Role       |                  What they do                |
|-----------------------------|------------------|----------------------------------------------|
|         **Freelancer**      | Invoice holder   | Submits unpaid invoices, receives immediate liquidity |
|           **Payer**         | Invoice debtor   | The client who owes the money, settles the invoice on-chain |
| **Liquidity provider (LP)** | Funder           | Funds invoices at a discount, earns yield when payer settles |
|        **ILN Contract**     | Trustless escrow | Holds funds, enforces rules, routes payments |

---

## Contract storage model

The contract uses Soroban's persistent storage with two key types:

```
StorageKey::InvoiceCount       → u64 (auto-incrementing ID counter)
StorageKey::Invoice(id: u64)   → Invoice struct
```

Each `Invoice` struct holds the full state of one invoice:

```rust
Invoice {
    id:            u64,
    freelancer:    Address,
    payer:         Address,
    amount:        i128,      // full invoice value in stroops
    due_date:      u64,       // Unix timestamp
    discount_rate: u32,       // basis points (e.g. 300 = 3%)
    status:        InvoiceStatus,
    funder:        Option<Address>,
    funded_at:     Option<u64>,
}
```

Invoice status follows a strict one-way state machine:

```
Pending → Funded → Paid
                 → Defaulted
```

No status transition can go backwards. A Paid or Defaulted invoice is terminal.

---

## Contract flow

### Step 1 — submit_invoice()

Called by the freelancer to register an unpaid invoice.

```
Freelancer
    │
    ├─ require_auth()                  freelancer must sign
    ├─ validate amount > 0
    ├─ validate discount_rate 1–5000 bps
    ├─ validate due_date > now
    ├─ assign next invoice ID
    ├─ save Invoice { status: Pending }
    └─ emit "submitted" event
```

No funds move at this step. The invoice sits in `Pending` state until an LP funds it.

---

### Step 2 — fund_invoice()

Called by a liquidity provider to fund a Pending invoice.

```
LP
 │
 ├─ require_auth()
 ├─ load invoice, assert status == Pending
 ├─ calculate:
 │     discount_amount  = amount × discount_rate / 10_000
 │     freelancer_payout = amount − discount_amount
 │
 ├─ token.transfer(LP → contract, amount)
 │     LP sends the full invoice value to the contract
 │
 ├─ token.transfer(contract → freelancer, freelancer_payout)
 │     Contract immediately pays out (amount − discount) to freelancer
 │     Freelancer has their liquidity. Invoice is now funded.
 │
 ├─ contract holds: discount_amount in escrow
 ├─ update Invoice { status: Funded, funder, funded_at }
 └─ emit "funded" event
```

After this step:
- The freelancer has received their money (minus the discount)
- The LP has committed funds and is waiting for the payer to settle
- The contract holds the discount amount in escrow

---

### Step 3a — mark_paid() — happy path

Called by the payer to settle the invoice in full.

```
Payer
 │
 ├─ require_auth()                     only the registered payer can call this
 ├─ load invoice, assert status == Funded
 ├─ assert due_date has not passed (optional: allow late payment)
 │
 ├─ token.transfer(payer → contract, amount)
 │     Payer sends the full invoice value to the contract
 │
 ├─ token.transfer(contract → funder, amount)
 │     Contract releases the full amount to the LP
 │     LP receives: their principal back + the escrowed discount = yield
 │
 ├─ update Invoice { status: Paid }
 └─ emit "paid" event
```

After this step the LP has earned the discount spread as yield:

```
LP sent:        1,000 USDC  (at fund_invoice time)
LP received:    1,000 USDC  (from payer settlement)
Escrowed yield:    30 USDC  (3% discount, returned alongside principal)
Net yield:          3.00%
```

---

### Step 3b — claim_default() — unhappy path

Called by the LP after the due date passes without payment.

```
LP (original funder)
 │
 ├─ require_auth()
 ├─ load invoice, assert status == Funded
 ├─ assert env.ledger().timestamp() > due_date
 │
 ├─ token.transfer(contract → funder, discount_amount)
 │     LP recovers only the escrowed discount
 │     The freelancer's payout cannot be reversed
 │
 ├─ update Invoice { status: Defaulted }
 └─ emit "defaulted" event
```

In a default the LP loses `amount − discount_amount` (the freelancer's payout). The discount amount is returned as partial recourse. This is the core risk LPs accept — it is why discount rates exist.

---

## Money flow diagram

```
                    fund_invoice()
                   ┌─────────────────────────────────────────────┐
                   │                                             │
                   ▼                                             │
LP ──── 1,000 USDC ──▶ CONTRACT ──── 970 USDC ──▶ FREELANCER   │
                         │                                       │
                         │ holds 30 USDC (discount escrow)       │
                         │                                       │
                    mark_paid()                                  │
                         │                                       │
PAYER ── 1,000 USDC ──▶ CONTRACT ──── 1,000 USDC ──▶ LP        │
                                       (principal + yield)       │
                                                                 │
                    claim_default() (if payer misses due_date)   │
                         │                                       │
                       CONTRACT ──── 30 USDC ──▶ LP             │
                                     (partial recourse only)     │
                                                                 └─
```

---

## Token handling

ILN uses USDC on Stellar, accessed via Soroban's native token interface:

```rust
use soroban_sdk::token::Client as TokenClient;

fn usdc_client(env: &Env) -> TokenClient {
    let address = Address::from_str(env, USDC_TOKEN);
    TokenClient::new(env, &address)
}
```

All amounts are stored and transferred in **stroops** (Stellar's base unit). 1 USDC = 10,000,000 stroops. The contract never converts — all arithmetic is in stroops to avoid rounding errors.

Discount calculation uses integer basis points to avoid floating point:

```rust
let discount_amount = invoice.amount
    .checked_mul(invoice.discount_rate as i128)
    .unwrap_or(0)
    / 10_000;
```

---

## Event emissions

The contract emits events at each state transition so indexers, frontends, and analytics tools can track activity without polling storage:

|    Event    |     Emitted by     |   Payload  |
|-------------|--------------------|------------|
| `submitted` | `submit_invoice()` | invoice ID |
|   `funded`  |   `fund_invoice()` | invoice ID |
|    `paid`   |    `mark_paid()`   | invoice ID |
| `defaulted` |  `claim_default()` | invoice ID |

---

## Security properties

**No admin key.** There is no privileged address that can pause the contract, drain funds, or alter invoice state. Once an invoice is funded, only the registered payer can trigger `mark_paid()` and only the original funder can trigger `claim_default()`.

**Auth on every state transition.** Every function that moves funds calls `require_auth()` on the relevant party before doing anything else. Unsigned calls revert immediately.

**Integer-only arithmetic.** No floating point anywhere in the contract. All amounts are `i128` in stroops. Discount calculations use basis points and integer division. Overflow is caught with `checked_mul`.

**Immutable invoice terms.** Once submitted, the amount, payer, due date, and discount rate cannot be changed. The LP knows exactly what they are funding when they call `fund_invoice()`.

**One funder per invoice.** An invoice can only be funded once. The second call to `fund_invoice()` on a Funded invoice returns `AlreadyFunded` immediately.

---

## File structure

```
contracts/invoice_liquidity/src/
├── lib.rs        — contract entry point, all public functions
├── invoice.rs    — Invoice struct, InvoiceStatus enum, storage helpers
├── errors.rs     — ContractError enum
└── tests.rs      — unit tests (native target only)
```

---

## Deployment

The contract is compiled to `.wasm` and deployed to Stellar's network via the Stellar CLI:

```bash
# Build
cargo build --target wasm32v1-none --release

# Deploy to testnet
stellar contract deploy \
  --wasm target/wasm32v1-none/release/invoice_liquidity.wasm \
  --source alice \
  --network testnet
```

Testnet contract ID: `CD3TE3IAHM737P236XZL2OYU275ZKD6MN7YH7PYYAXYIGEH55OPEWYJC`

There is one contract instance per network. All invoices across all users are stored within that single contract's persistent storage, keyed by invoice ID.

---

## Known limitations (v1)

**Default risk is unmitigated.** If a payer defaults, the LP loses the principal they advanced to the freelancer. There is no insurance pool, no credit scoring, and no collateral requirement. LPs should assess payer quality manually.

**Payer verification is on-chain only.** `mark_paid()` requires the registered payer address to sign the transaction. There is no mechanism to verify that the on-chain payer address corresponds to the real-world client. Invoice submission is currently trust-based between freelancer and payer.

**No dispute mechanism.** If a freelancer submits a fraudulent invoice, the LP has no on-chain recourse beyond `claim_default()` after the due date. Dispute resolution is out of scope for v1.

**Single asset.** Only USDC is supported. Multi-asset support is on the roadmap.