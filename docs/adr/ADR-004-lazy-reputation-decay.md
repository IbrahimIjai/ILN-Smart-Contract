# ADR-004: Lazy Reputation Decay

**Date:** 2024-05-01  
**Status:** Accepted

## Context

The ILN reputation system assigns scores (0–100) to payers and LPs. Scores
should decay over time when an account is inactive, so that a payer who paid
reliably three years ago but has since gone insolvent does not retain a high
score indefinitely.

Two broad approaches exist for implementing time-based decay:

1. **Eager decay** — a background process (cron job, keeper bot, or on-chain
   scheduler) periodically updates every account's score.
2. **Lazy decay** — the score is stored as a `(value, last_activity_ledger)`
   pair and the decayed value is computed on-demand when the score is read.

Soroban has no native scheduler or cron mechanism. Any eager approach would
require an off-chain keeper, introducing a centralised dependency and a liveness
assumption.

## Decision

Reputation decay is **lazy**: the raw score and the ledger number of the last
activity are stored. When `get_payer_score` or `get_lp_score` is called, the
contract computes how many full decay periods have elapsed since
`last_activity_ledger` and applies the decay formula iteratively:

```rust
let periods = ledgers_elapsed / decay_period_ledgers;
for _ in 0..periods {
    score = score * (10_000 - decay_rate_bps) / 10_000;
}
```

The decayed value is returned to the caller but **not written back** to storage
unless a state-changing function (e.g. `mark_paid`, `claim_default`) is
executing. Read-only queries are free of storage writes.

Default parameters: `decay_rate_bps = 50` (0.5% per period),
`decay_period_ledgers = 259_200` (≈ 15 days at 5 s/ledger).

## Alternatives Considered

| Alternative | Why rejected |
|-------------|--------------|
| **Eager decay via off-chain keeper** | Introduces a centralised liveness dependency; if the keeper goes down, scores stagnate. Contradicts the no-backend design goal. |
| **Eager decay via Soroban scheduled invocations** | Soroban does not have a native scheduler. Simulating one requires a privileged caller and adds a trusted role. |
| **Continuous decay (per-ledger)** | Requires either a keeper or a complex lazy formula involving exponentiation. Per-period decay is simpler to audit and reason about. |
| **No decay** | A payer who paid 50 invoices in 2024 and then disappeared would retain a score of 100 indefinitely. LPs would fund their invoices at premium rates and lose money. Decay is necessary for score validity. |
| **Decay on write only** | Score would only decay when the account performs an action. A dormant account's score would never decay, defeating the purpose. |

## Consequences

**Positive:**
- No off-chain keeper required; the contract is fully self-contained.
- Storage writes are minimised — read-only score queries do not touch storage,
  keeping instruction costs low.
- Decay parameters (`decay_rate_bps`, `decay_period_ledgers`) are governable;
  the community can adjust the decay speed without a contract upgrade.
- The formula is simple integer arithmetic, easy to audit and reproduce
  off-chain for UI display.

**Negative / Trade-offs:**
- The first state-changing transaction after a long dormancy period must compute
  potentially many decay periods in a loop. With default parameters a 3-year
  dormancy yields ~73 periods — negligible CPU cost, but worth noting.
- Score displayed in a UI may differ from the score stored in contract state
  (the stored value is the pre-decay raw score). Frontends must call the
  read function, not read storage directly.
- Decay is not applied atomically to all accounts at a block boundary; two
  accounts with the same raw score but different `last_activity_ledger` values
  will show different effective scores at the same query time. This is
  intentional but can be surprising.
