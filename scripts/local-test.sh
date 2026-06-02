#!/bin/bash
# Quick local testing script
# Runs unit tests, builds WASM, and checks benchmark regression
#
# Usage: ./scripts/local-test.sh

set -euo pipefail

GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m'

echo "=== Running local test suite ==="
echo ""

# Run unit tests
echo "Running unit tests..."
if cargo test -p invoice_liquidity -- --nocapture; then
  echo -e "${GREEN}✓ Unit tests passed${NC}"
else
  echo -e "${RED}✗ Unit tests failed${NC}"
  exit 1
fi

echo ""

# Build WASM
echo "Building WASM contracts..."
if cargo build --target wasm32v1-none --release; then
  echo -e "${GREEN}✓ WASM build successful${NC}"
else
  echo -e "${RED}✗ WASM build failed${NC}"
  exit 1
fi

echo ""

# Check benchmark regression
echo "Checking benchmark regression..."
if bash scripts/check_benchmark_regression.sh; then
  echo -e "${GREEN}✓ Benchmark checks passed${NC}"
else
  echo -e "${YELLOW}⚠ Benchmark check warnings (see above)${NC}"
fi

echo ""
echo -e "${GREEN}✅ All local tests completed!${NC}"
