# ADR-001: Choice of Soroban / Stellar over Other Chains

**Date:** 2024-05-01  
**Status:** Accepted

## Context

ILN is an invoice liquidity protocol that requires trustless escrow, deterministic
settlement, and low transaction costs. The team evaluated several smart-contract
platforms before committing to a chain. The primary use-case targets freelancers
and SMEs who deal in USDC and need near-instant, cheap settlement — not
speculative DeFi users who are already on Ethereum.

Key requirements that drove the evaluation:

- **Native USDC support** — USDC is the settlement currency; bridged or wrapped
  versions introduce additional trust assumptions.
- **Low fees** — Freelancers submitting invoices and payers settling them cannot
  absorb $5–$50 gas fees per transaction.
- **Predictable execution costs** — Soroban's metered instruction model gives
  deterministic, pre-computable fees, which matters for UX.
- **Rust / WASM toolchain** — The team has Rust expertise and wanted a typed,
  memory-safe language with a mature ecosystem.
- **Regulatory posture** — Stellar's focus on payments and its existing
  relationships with regulated stablecoin issuers (Circle) reduces compliance
  friction.

## Decision

Build on **Stellar Soroban** using Rust-compiled WASM contracts.

## Alternatives Considered

| Alternative | Why rejected |
|-------------|--------------|
| **Ethereum (Solidity)** | Gas fees are prohibitive for small invoices; Solidity's type system is weaker than Rust; no native USDC (requires bridging or USDC ERC-20 with Circle trust). |
| **Solana (Anchor / native)** | Account model complexity increases audit surface; ecosystem tooling is less mature for financial contracts; USDC is available but Solana's history of outages is a reliability concern. |
| **Polygon / L2 rollups** | Cheaper than Ethereum mainnet but still require bridging USDC; adds bridge risk; finality guarantees vary by rollup design. |
| **Cosmos SDK (CosmWasm)** | Strong Rust support, but no native USDC; IBC bridging adds latency and trust assumptions; smaller LP/user base for invoice finance. |
| **Algorand (PyTeal / AVM)** | Native USDC (USDC ASA) is available, but the developer ecosystem is smaller; Python-based tooling is less type-safe; limited DeFi composability. |

## Consequences

**Positive:**
- Native USDC via Stellar's token interface — no bridge risk.
- Soroban fees are metered and predictable; a typical invoice lifecycle costs
  well under $0.01.
- Rust's ownership model and `checked_mul` / `checked_add` prevent entire
  classes of arithmetic bugs at compile time.
- Stellar's existing payment rails mean payers may already have Stellar wallets.
- Soroban's persistent/temporary/instance storage tiers allow fine-grained TTL
  management, reducing state bloat costs.

**Negative / Trade-offs:**
- Soroban is newer than EVM; the auditor pool and public tooling are smaller.
- Fewer composability integrations with existing DeFi protocols (no Uniswap,
  Aave, etc.) — limits future yield strategies for idle escrow funds.
- The `wasm32v1-none` target requires a specific Rust toolchain version; CI
  setup is more involved than a standard Rust project.
- Stellar's account model (sequence numbers, signers) adds onboarding friction
  for users unfamiliar with the ecosystem.
