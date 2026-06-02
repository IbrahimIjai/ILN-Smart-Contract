# ADR-002: Single-Contract Architecture

**Date:** 2024-05-01  
**Status:** Accepted

## Context

The ILN protocol needs to store invoice state, enforce payment rules, hold
escrow funds, and emit events. The team had to decide whether to split these
responsibilities across multiple specialised contracts or consolidate them into
one contract.

The protocol's v1 scope is deliberately narrow: one asset (USDC), three actors
(freelancer, payer, LP), and four state transitions (submit → fund → paid /
defaulted). Premature decomposition would add cross-contract call overhead and
complicate the security model before the core logic is proven.

Soroban cross-contract calls are synchronous but each hop consumes additional
CPU and memory instructions, increasing fees. For a protocol targeting
micro-transactions, every instruction counts.

## Decision

All core invoice logic lives in a **single Soroban contract**
(`contracts/invoice_liquidity`). Governance (`iln_governance`), reputation bonus
(`reputation_bonus`), and distribution (`iln_distribution`) are separate
contracts that call into the core contract, but the core contract itself is
monolithic.

## Alternatives Considered

| Alternative | Why rejected |
|-------------|--------------|
| **Separate escrow contract** | Splitting fund custody from logic requires a cross-contract call on every `fund_invoice` and `mark_paid`, doubling the instruction cost for the hot path. The security benefit is marginal when both contracts are immutable WASM. |
| **Factory pattern (one contract per invoice)** | Eliminates storage key collisions but creates unbounded contract deployment costs; indexing becomes harder; upgrading logic requires migrating every deployed instance. |
| **Proxy / upgradeable pattern** | Soroban does not have a native proxy standard; implementing one adds complexity and a privileged upgrade key, which contradicts the no-admin-key design goal. |
| **Microservice contracts (storage, logic, events)** | Maximises separation of concerns but each cross-contract call adds latency and fee overhead; the added complexity is not justified at v1 scale. |

## Consequences

**Positive:**
- Single audit surface — reviewers only need to understand one contract's
  storage layout and state machine.
- No cross-contract call overhead on the critical fund/pay path.
- Deployment is a single WASM upload; no orchestration of multiple contract
  addresses.
- All invoice state is co-located, making storage layout reasoning
  straightforward (see `docs/storage-layout.md`).

**Negative / Trade-offs:**
- The contract will grow as features are added; without careful module
  boundaries it can become hard to navigate. Mitigated by splitting into
  `lib.rs`, `invoice.rs`, `errors.rs`.
- Upgrading the contract requires a full WASM replacement; there is no
  partial upgrade path. Any storage migration must be handled in the new WASM
  at first invocation.
- All invoices share the same contract instance, so a critical bug affects
  every invoice simultaneously rather than being isolated to a subset.
