# Smart Contract Benchmarks

*Date:* 2026-05-30

These values are baseline metrics for core contract execution: CPU instructions and memory bytes consumed via Soroban's cost meter (`env.cost_estimate()`). CI compares each run against `contracts/invoice_liquidity/benchmarks/baseline.json` and emits a **warning** (not a failure) when either metric regresses by more than 10%.

## Baseline Execution Results

| Function       | CPU Instructions | Memory (bytes) |
| -------------- | ---------------- | -------------- |
| submit_invoice |           859421 |          26485 |
| fund_invoice   |          1041920 |          38190 |
| mark_paid      |           948123 |          35480 |

## Re-Running Locally

```bash
cd contracts/invoice_liquidity
cargo test --target x86_64-unknown-linux-gnu benchmark -- --nocapture
```

Each benchmark test prints machine-readable lines:

```
BENCHMARK submit_invoice cpu=859421 mem=26485
```

## CI Regression Check

```bash
bash scripts/check_benchmark_regression.sh
```

The script always exits 0. Regressions above the threshold are reported as `::warning::` annotations in GitHub Actions.

## Updating Baselines

After an intentional optimisation or contract change:

1. Run the benchmark suite locally with `--nocapture`.
2. Update `contracts/invoice_liquidity/benchmarks/baseline.json`.
3. Update the table in this document.
