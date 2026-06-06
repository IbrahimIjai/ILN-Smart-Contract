# Insurance Pool Design — Default Protection for LPs (Issue #123)

**Status:** Design-forward stub (interface + accounting implemented; economics & token settlement are follow-ups)
**Crate:** `contracts/insurance_pool`

## Motivation

Liquidity providers (LPs) who fund invoices bear the risk that a payer
*defaults*. Before mainnet, ILN should offer an **optional** insurance pool that
LPs can buy into for protection: they pay periodic premiums, and if an invoice
they funded defaults, the pool compensates them out of accumulated premiums.

This document describes the contract interface, the stub implementation shipped
in this PR, and the integration with the main `invoice_liquidity` contract.

## Interface

Defined in [`contracts/insurance_pool/src/insurance_interface.rs`] as the
`InsurancePoolInterface` trait (a typed `InsurancePoolInterfaceClient` is
generated for cross-contract calls):

| Method | Auth | Description |
|--------|------|-------------|
| `enroll(lp)` | `lp` | Opt an LP into the program. |
| `is_enrolled(lp) -> bool` | — | Whether `lp` is enrolled. |
| `deposit_premium(lp, amount)` | `lp` | Pay a premium; increases pool balance; auto-enrolls. |
| `claim(invoice_id) -> i128` | admin | Compensate for a defaulted invoice; returns payout. Idempotent per invoice. |
| `get_pool_balance() -> i128` | — | Total pool balance (premiums − payouts). |

Auxiliary views on the contract: `get_premiums_paid(lp)`, `get_coverage()`,
`is_claimed(invoice_id)`, plus `initialize(admin, coverage)`.

## Stub semantics (what ships here)

The stub in `contracts/insurance_pool/src/lib.rs` is a **correct, fully-tested**
implementation of the interface with intentionally simplified economics:

- **Accounting, not custody.** `deposit_premium` records the premium as pool
  *accounting* balance. A production pool would move SAC tokens into the
  contract; that token settlement is deliberately out of scope for the stub.
- **Flat coverage cap.** `claim` pays `min(coverage, pool_balance)`, where
  `coverage` is a flat per-claim cap set at `initialize`. A production pool
  would price payouts against the invoice amount, the LP's premium history, and
  remaining pool solvency.
- **Idempotency & auth.** Each `invoice_id` can be claimed once; `claim`
  requires the configured admin (the liquidity contract in production).

Ten interface tests cover initialization, enrollment, premium accumulation,
coverage-capped vs balance-capped payouts, idempotency, and the empty-pool and
invalid-amount rejection paths (`cargo test -p insurance_pool`).

## Integration with `invoice_liquidity`

The compensation hook lives on the liquidity contract's default-handling path
(`claim_default`). The design:

1. Governance stores the deployed pool address (e.g. a new
   `DataKey::InsurancePool` instance key + an admin setter).
2. When a default is confirmed for `invoice_id` funded by `lp`, and `lp` is
   enrolled, the liquidity contract invokes the pool:

```rust
// inside claim_default(), after the invoice is marked Defaulted:
if let Some(pool) = storage::get_insurance_pool(&env) {
    let client = insurance_pool::InsurancePoolInterfaceClient::new(&env, &pool);
    if client.is_enrolled(&lp) {
        let payout = client.claim(&invoice_id); // pool credits/transfers to lp
        env.events().publish(
            (symbol_short!("ins_comp"), invoice_id),
            (lp.clone(), payout),
        );
    }
}
```

3. The pool is configured with the liquidity contract as its `admin`, so only a
   genuine confirmed default can trigger `claim`.

> **Note:** This wiring is documented rather than committed in this PR because
> the `invoice_liquidity` crate does not currently compile on `main` (a botched
> merge predating this work — see the PR description). The hook above is a
> drop-in for `claim_default` once the contract builds, and the
> `InsurancePoolInterfaceClient` it relies on is already generated and exported
> by this crate.

## Follow-up work (before mainnet)

- Real SAC token custody for premiums and payouts.
- Risk-priced premiums and coverage (vs. flat cap).
- Pool solvency guards and payout prioritization across simultaneous defaults.
- Governance parameters (premium schedule, coverage ratio) + admin rotation.
- End-to-end integration tests across `invoice_liquidity` ⇄ `insurance_pool`.
