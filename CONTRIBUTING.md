# Contributing to Invoice Liquidity Network Smart Contracts

Thank you for your interest in contributing to ILN-Smart-Contract!

## 🧪 Testing

### Running Unit and Integration Tests

To run the standard unit and integration tests for the governance, distribution, and reputation contracts:

```bash
cargo test
```

### 🧬 Running the Fuzz Suite

We use property-based testing and fuzzing via `proptest` to check contract safety and robustness under random inputs.

To run the fuzz tests:

```bash
cargo test -p iln_fuzz
```

The fuzz suite tests `submit_invoice()` with randomized parameters (amount, discount rate, due date, and randomized account/contract address payloads) and verifies that the contract never panics and handles invalid inputs gracefully.
