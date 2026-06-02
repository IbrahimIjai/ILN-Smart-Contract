# ADR-003: Discount Rate Represented in Basis Points

**Date:** 2024-05-01  
**Status:** Accepted

## Context

Every invoice has a `discount_rate` that determines how much of the invoice
value the LP retains as yield. This value must be stored on-chain, validated,
and used in arithmetic to compute the freelancer payout and the escrowed yield.

Smart contracts cannot use floating-point arithmetic safely. Soroban's WASM
environment does not expose hardware floats, and even if it did, floating-point
rounding is non-deterministic across architectures — two nodes could compute
different payouts for the same invoice, breaking consensus.

The team needed an integer representation that:

1. Covers the practical range of invoice discount rates (roughly 0.01% – 50%).
2. Supports integer division without catastrophic precision loss on typical
   invoice amounts.
3. Is a well-understood convention that auditors and integrators will recognise
   immediately.

## Decision

`discount_rate` is stored as a `u32` in **basis points** (bps), where
1 bps = 0.01%. Valid range is 1–5000 bps (0.01%–50%).

Discount arithmetic:

```rust
let discount_amount = invoice.amount
    .checked_mul(invoice.discount_rate as i128)
    .unwrap_or(0)
    / 10_000;
let freelancer_payout = invoice.amount - discount_amount;
```

All amounts are in stroops (1 USDC = 10,000,000 stroops), so even a 1 bps
discount on a 1 USDC invoice yields a non-zero integer result
(10,000,000 / 10,000 = 1,000 stroops = $0.0001).

## Alternatives Considered

| Alternative | Why rejected |
|-------------|--------------|
| **Floating-point (f64)** | Non-deterministic across platforms; not available in `no_std` WASM without a software float library; introduces rounding divergence between nodes. |
| **Fixed-point with a custom denominator (e.g. ×1,000,000)** | Higher precision, but non-standard; integrators and auditors would need to learn the convention; no benefit over bps for the 0–50% range. |
| **Percentage as integer (0–100)** | Only 1% granularity; cannot represent 0.5% or 2.5% discounts, which are common in invoice finance. |
| **Per-mille (‰, ×1,000)** | 0.1% granularity; better than integer percent but still too coarse for sub-0.1% rates; less universally recognised than bps in finance. |
| **Storing the pre-computed discount amount** | Removes the need for on-chain arithmetic but makes the rate opaque; the LP cannot verify the rate from storage alone; also prevents governance from changing the rate formula. |

## Consequences

**Positive:**
- Basis points are the standard unit in financial contracts, bond markets, and
  DeFi protocols (Uniswap, Aave, Compound all use bps). Auditors and
  integrators recognise the convention immediately.
- Integer arithmetic with a fixed denominator of 10,000 is simple, auditable,
  and produces identical results on every node.
- `checked_mul` on `i128` handles amounts up to ~1.7 × 10³⁸ stroops before
  overflow — far beyond any realistic invoice value.
- The 1–5000 bps range (0.01%–50%) covers all practical invoice discount
  scenarios while rejecting nonsensical values at submission time.

**Negative / Trade-offs:**
- Sub-basis-point precision (e.g. 0.005%) is not representable. This is
  acceptable for invoice finance but would be insufficient for, say, a
  high-frequency trading fee model.
- Integer division truncates; the freelancer always receives the floor of the
  calculated payout. The truncation error is at most 1 stroop (~$0.0000001),
  which is economically negligible.
- Callers must remember to divide by 10,000, not 100. Off-by-100 errors in
  client code are a known footgun; the ABI documentation and events must make
  the unit explicit.
