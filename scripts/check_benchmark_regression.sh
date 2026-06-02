#!/usr/bin/env bash
# Compare benchmark output against baseline and warn on >10% regression.
# Exits 0 always (CI warning-only); prints ::warning:: lines for GitHub Actions.

set -euo pipefail

BASELINE_FILE="${1:-contracts/invoice_liquidity/benchmarks/baseline.json}"
REGRESSION_THRESHOLD="${BENCHMARK_REGRESSION_THRESHOLD:-10}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

if [[ ! -f "$ROOT/$BASELINE_FILE" ]]; then
  echo "::warning::Baseline file not found: $BASELINE_FILE"
  exit 0
fi

echo "Running benchmark tests..."
BENCHMARK_OUTPUT="$(cd "$ROOT/contracts/invoice_liquidity" && cargo test --target x86_64-unknown-linux-gnu benchmark -- --nocapture 2>&1)" || true

export BENCHMARK_OUTPUT
export BASELINE_FILE="$ROOT/$BASELINE_FILE"
export REGRESSION_THRESHOLD

python3 <<'PY'
import json
import os
import re

baseline_path = os.environ["BASELINE_FILE"]
threshold_pct = float(os.environ["REGRESSION_THRESHOLD"])
output = os.environ.get("BENCHMARK_OUTPUT", "")

with open(baseline_path) as f:
    baseline = json.load(f)["benchmarks"]

pattern = re.compile(r"BENCHMARK\s+(\w+)\s+cpu=(\d+)\s+mem=(\d+)")
measured = {m.group(1): {"cpu": int(m.group(2)), "mem": int(m.group(3))} for m in pattern.finditer(output)}

if not measured:
    print("::warning::No BENCHMARK lines found in test output")
    raise SystemExit(0)

print("| Function | CPU (measured) | CPU (baseline) | Mem (measured) | Mem (baseline) |")
print("| -------- | -------------- | -------------- | -------------- | -------------- |")

for name, base in baseline.items():
    current = measured.get(name)
    if not current:
        print(f"::warning::Missing benchmark measurement for {name}")
        continue

    cpu_base, mem_base = base["cpu"], base["mem"]
    cpu_cur, mem_cur = current["cpu"], current["mem"]
    cpu_pct = ((cpu_cur - cpu_base) / cpu_base) * 100 if cpu_base else 0
    mem_pct = ((mem_cur - mem_base) / mem_base) * 100 if mem_base else 0

    print(f"| {name} | {cpu_cur} | {cpu_base} | {mem_cur} | {mem_base} |")

    if cpu_pct > threshold_pct:
        print(
            f"::warning::Benchmark regression: {name} CPU instructions "
            f"increased {cpu_pct:.1f}% ({cpu_cur} vs baseline {cpu_base})"
        )
    if mem_pct > threshold_pct:
        print(
            f"::warning::Benchmark regression: {name} memory bytes "
            f"increased {mem_pct:.1f}% ({mem_cur} vs baseline {mem_base})"
        )
PY
