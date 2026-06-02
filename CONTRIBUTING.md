# Contributing to ILN Smart Contracts

Thank you for contributing to the Invoice Liquidity Network!  
This guide covers everything you need to go from a fresh machine to an accepted pull request.

---

## Table of Contents

## 📝 Commit Messages

This project uses [Conventional Commits](https://www.conventionalcommits.org/) so that the changelog can be generated automatically with `make changelog`.

**Format:** `<type>(<optional scope>): <description>`

| Type | When to use |
|------|-------------|
| `feat` | A new feature or contract function |
| `fix` | A bug fix |
| `refactor` | Code change that neither fixes a bug nor adds a feature |
| `perf` | Performance improvement |
| `test` | Adding or updating tests |
| `docs` | Documentation only changes |
| `chore` | Build process, tooling, or dependency updates |

**Examples:**
```
feat(governance): add quorum requirement for proposal passing
fix: resolve discount rate validation overflow
docs: add architecture decision records
test: add fuzz suite for submit_invoice
chore: add CHANGELOG and git-cliff changelog automation
```

Breaking changes must include `BREAKING CHANGE:` in the commit footer:
```
feat!: rename fund_invoice to fund

BREAKING CHANGE: fund_invoice has been renamed to fund in the invoice_liquidity contract.
```


## 🧪 Testing
1. [Environment Setup](#1-environment-setup)
2. [Building the Contracts](#2-building-the-contracts)
3. [Running Tests](#3-running-tests)
4. [Code Style](#4-code-style)
5. [PR Requirements](#5-pr-requirements)
6. [Review Process](#6-review-process)
7. [Soroban-Specific Gotchas](#7-soroban-specific-gotchas)

---

## 1. Environment Setup

### Rust toolchain

```bash
# Install rustup if you don't have it
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"

# Minimum supported version: 1.74
rustup update stable

# Add the WASM target used by Soroban
rustup target add wasm32v1-none

# Add formatting and linting components
rustup component add rustfmt clippy
```

### Stellar CLI

```bash
cargo install --locked stellar-cli --features opt
stellar --version
```

> Re-run the install command to upgrade an existing installation.

### Clone the repo

```bash
git clone https://github.com/Invoice-Liquidity-Network/ILN-Smart-Contract.git
cd ILN-Smart-Contract
```

For a more detailed walkthrough (testnet account setup, troubleshooting) see
[`docs/developer-quickstart.md`](docs/developer-quickstart.md).

---

## 2. Building the Contracts

```bash
# Debug build (native, fast — used for tests)
cargo build

# Optimised WASM build (required before deployment)
cargo build --release --target wasm32-unknown-unknown

# Alternative Soroban-specific optimized build
# `cargo build-wasm` is a workspace alias defined in `.cargo/config.toml`.
# It builds optimized WASM to the Soroban-specific target:
# cargo build --target wasm32v1-none --release
# or use the Makefile shortcut:
# make soroban-optimize
```

WASM output lands in `target/wasm32v1-none/release/*.wasm`.

> The `build-wasm` alias is defined in `.cargo/config.toml`.  
> The release profile enables LTO and `opt-level = "z"` — typical output is 10–80 KB per contract.

---

## 3. Running Tests

### Unit and integration tests

```bash
# Entire workspace
cargo test

# Single contract
cargo test -p invoice_liquidity
cargo test -p iln_governance
cargo test -p iln_distribution
cargo test -p reputation_bonus

# Useful flags
cargo test -p invoice_liquidity -- --nocapture   # show stdout
cargo test -p invoice_liquidity test_name        # filter by name
```

Tests run on your native architecture via `soroban-sdk` test utilities — no WASM build needed.

### Fuzz / property-based tests

```bash
cargo test -p iln_fuzz
```

Property tests generate thousands of random cases and may take a few minutes.
To skip them during rapid iteration:

```bash
cargo test -p invoice_liquidity -- --skip prop_
# or limit case count
PROPTEST_CASES=100 cargo test -p invoice_liquidity
```

### Coverage

CI enforces **≥ 95 % line coverage** on `invoice_liquidity` using
[cargo-tarpaulin](https://github.com/xd009642/tarpaulin).  Run it locally
before pushing if you touch that crate:

```bash
cargo install cargo-tarpaulin --locked
cargo tarpaulin -p invoice_liquidity --fail-under 95
```

---

## 4. Code Style

### Formatting

All code must be formatted with `rustfmt` using the workspace defaults:

```bash
cargo fmt --all
```

CI will reject PRs with formatting differences.

### Linting

Zero Clippy warnings are required:

```bash
cargo clippy --all-targets -- -D warnings
```

Fix every warning before opening a PR.  Do not use `#[allow(...)]` to silence
warnings without a comment explaining why.

### General conventions

- Keep functions small and single-purpose.
- Prefer explicit error types over `unwrap` / `expect` in contract code.
- Document public functions with a `///` doc comment.
- Avoid introducing new dependencies without discussion in an issue first.

---

## 5. PR Requirements

### Branch naming

```
<type>/<short-description>
```

Examples: `feat/multi-token-support`, `fix/overflow-in-discount`, `docs/contributing`.

### Commit messages — Conventional Commits

Follow the [Conventional Commits](https://www.conventionalcommits.org/) spec:

```
<type>(<optional scope>): <short summary>

[optional body]

[optional footer — e.g. Closes #101]
```

Common types: `feat`, `fix`, `docs`, `refactor`, `test`, `chore`.

Example:

```
docs: add CONTRIBUTING.md for smart contract contributors

Covers env setup, build, test, style, PR process, and Soroban gotchas.

Closes #101
```

### Checklist before opening a PR

- [ ] `cargo fmt --all` — no diff
- [ ] `cargo clippy --all-targets -- -D warnings` — zero warnings
- [ ] `cargo test` — all tests pass
- [ ] `cargo test -p iln_fuzz` — fuzz suite passes
- [ ] Coverage ≥ 95 % if `invoice_liquidity` was modified
- [ ] New behaviour is covered by tests
- [ ] `cargo build-wasm` succeeds (required for any contract change)
- [ ] PR description explains *what* changed and *why*
- [ ] Related issue linked in the PR description or footer (`Closes #<n>`)

---

## 6. Review Process

1. Open a PR against `main` on the upstream repo
   (`Invoice-Liquidity-Network/ILN-Smart-Contract`).
2. CI runs automatically: `test → clippy → benchmarks → coverage`.
   All jobs must be green before review begins.
3. At least one maintainer approval is required to merge.
4. Address review comments with new commits (do not force-push during review).
5. A maintainer will squash-merge once approved.

---

## 7. Soroban-Specific Gotchas

### WASM target is `wasm32v1-none`, not `wasm32-unknown-unknown`

Soroban uses Wasm 2.0 with no WASI.  Always use:

```bash
rustup target add wasm32v1-none
cargo build --target wasm32v1-none --release
```

Using the old `wasm32-unknown-unknown` target will produce a binary that the
Stellar runtime rejects.

### `std` is not available in contract code

Contract crates use `#![no_std]`.  Use `soroban-sdk` types (`Vec`, `Map`,
`String`, …) instead of `std` equivalents.  The `std` crate is only available
in test code gated behind `#[cfg(test)]`.

### `cargo test` does not test the WASM binary

Unit tests run on native via the SDK's mock environment.  Always do a
`cargo build-wasm` before deploying to confirm the WASM compiles cleanly —
some `no_std` violations only surface at WASM compile time.

### Testnet deployment

See [`docs/developer-quickstart.md § 7`](docs/developer-quickstart.md) for the
full deploy workflow.  The live testnet contract ID is:

```
CD3TE3IAHM737P236XZL2OYU275ZKD6MN7YH7PYYAXYIGEH55OPEWYJC
```

### Benchmark regression guard

The `scripts/check_benchmark_regression.sh` script compares instruction counts
against stored baselines.  CI runs it as a warning-only step, but a large
regression in `invoice_liquidity` will be flagged during review.  Run it
locally after performance-sensitive changes:

```bash
bash scripts/check_benchmark_regression.sh
```

---

## Questions?

Open a [GitHub Discussion](https://github.com/Invoice-Liquidity-Network/ILN-Smart-Contract/discussions)
or comment on the relevant issue.
