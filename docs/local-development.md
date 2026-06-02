# Local Development Guide for ILN Smart Contracts

This guide walks you through setting up a complete local development environment for the ILN smart contracts, including a local Stellar node via Docker for integration testing without depending on the testnet.

---

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Part 1: Rust & Stellar CLI Setup](#part-1-rust--stellar-cli-setup)
3. [Part 2: Local Stellar Node with Docker](#part-2-local-stellar-node-with-docker)
4. [Part 3: Building Contracts](#part-3-building-contracts)
5. [Part 4: Running Tests](#part-4-running-tests)
6. [Part 5: Deploying to Local Node](#part-5-deploying-to-local-node)
7. [Local Development Workflow](#local-development-workflow)
8. [Common Issues and Fixes](#common-issues-and-fixes)
9. [Helper Scripts](#helper-scripts)
10. [CI/CD Integration](#cicd-integration)

---

## Prerequisites

| Tool | Minimum version | Platform |
|------|----------------|----------|
| Rust | 1.74 | Any |
| Git | any recent | Any |
| Docker | 20.10+ | Linux, macOS, Windows (with WSL2) |
| Docker Compose | 2.0+ | Any |
| curl | any recent | Any |

> **Windows Users:** Install [WSL2](https://learn.microsoft.com/en-us/windows/wsl/install) and run all commands in an Ubuntu terminal.

---

## Part 1: Rust & Stellar CLI Setup

### 1.1 Install rustup

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

Verify:
```bash
rustc --version  # should be ≥ 1.74
cargo --version
```

### 1.2 Add the WASM target

Soroban contracts compile to `wasm32v1-none`:

```bash
rustup target add wasm32v1-none
```

Verify:
```bash
rustup target list --installed | grep wasm32v1
# Output: wasm32v1-none-unknown
```

### 1.3 Install Stellar CLI

```bash
cargo install --locked stellar-cli --features opt
stellar --version
```

If you already have an older version, re-run the command above to upgrade.

### 1.4 Clone the repository

```bash
git clone https://github.com/directorfloo/ILN-Smart-Contract.git
cd ILN-Smart-Contract
```

### 1.5 Verify the build

```bash
cargo build
```

This verifies that the dependency graph resolves correctly. No WASM is generated at this stage.

---

## Part 2: Local Stellar Node with Docker

This section sets up a local Stellar network using Docker, perfect for end-to-end integration testing.

### 2.1 Install Docker

**Linux:**
```bash
curl -fsSL https://get.docker.com -o get-docker.sh
sudo sh get-docker.sh
sudo usermod -aG docker $USER
newgrp docker
```

**macOS:**
```bash
# Using Homebrew
brew install docker
brew install docker-compose

# Or download Docker Desktop from https://www.docker.com/products/docker-desktop
```

**Windows (WSL2):**
1. Install WSL2: https://learn.microsoft.com/en-us/windows/wsl/install
2. Install Docker Desktop for Windows with WSL2 backend
3. Run all commands in WSL2 terminal

Verify:
```bash
docker --version
docker compose --version
```

### 2.2 Create docker-compose.yml for Local Stellar Network

Create the following file at the repository root:

```yaml
# docker-compose.yml
version: '3.8'

services:
  stellar:
    image: stellar/stellar-quickstart:latest
    container_name: iln-stellar-local
    ports:
      - "8000:8000"  # HTTP port
      - "11626:11626" # Peer port
    environment:
      - NETWORK_MODE=standalone
      - STELLAR_NETWORK_ID=local
      - STELLAR_CAPTIVE_CORE_CONFIG_PATH=/etc/stellar/captive-core.cfg
    volumes:
      - stellar-data:/opt/stellar
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8000/health"]
      interval: 5s
      timeout: 10s
      retries: 10
      start_period: 30s

volumes:
  stellar-data:

networks:
  default:
    name: iln-local-network
```

Save this as `docker-compose.yml` in the repository root.

### 2.3 Start the Local Stellar Node

```bash
docker compose up -d stellar
```

Wait for the container to be healthy:
```bash
docker compose logs -f stellar | grep "ready"
```

The local Stellar RPC will be available at `http://localhost:8000/`.

### 2.4 Verify the Local Node is Running

```bash
curl -X GET http://localhost:8000/ledger
```

Expected output (JSON ledger info):
```json
{
  "sequence": "1",
  "hash": "...",
  ...
}
```

---

## Part 3: Building Contracts

### 3.1 Build for Debug (Unit Tests)

```bash
cargo build
```

This compiles all workspace members in debug mode and verifies dependencies.

### 3.2 Build WASM for Deployment

```bash
cargo build --target wasm32v1-none --release
```

Or use the convenience alias:
```bash
cargo build-wasm
```

Output files appear in `target/wasm32v1-none/release/`:
- `invoice_liquidity.wasm`
- `iln_governance.wasm`
- `iln_distribution.wasm`
- `reputation_bonus.wasm`

### 3.3 Check Binary Sizes

```bash
ls -lh target/wasm32v1-none/release/*.wasm
```

Expected sizes: 10–80 KB per contract (due to `opt-level = "z"` in release profile).

---

## Part 4: Running Tests

### 4.1 Unit and Integration Tests

All tests run on your native architecture (no Wasm required) via `soroban-sdk` test utilities:

```bash
# Run all tests
cargo test

# Run tests for a specific contract
cargo test -p invoice_liquidity
cargo test -p iln_governance
cargo test -p reputation_bonus
cargo test -p iln_distribution
```

### 4.2 Useful Test Flags

```bash
# Show stdout (useful for proptest failure output)
cargo test -p invoice_liquidity -- --nocapture

# Filter by test name
cargo test -p invoice_liquidity test_update_max_discount

# Run a specific test module
cargo test -p invoice_liquidity tests_discount_invariants

# Skip slow property-based tests
cargo test -p invoice_liquidity -- --skip prop_

# Run with thread count = 1 (useful for debugging)
cargo test -p invoice_liquidity -- --test-threads=1 --nocapture
```

### 4.3 Fuzz Suite

The `iln_fuzz` crate uses `proptest` for property-based testing:

```bash
cargo test -p iln_fuzz
```

This runs thousands of random test cases and may take 1–2 minutes.

### 4.4 Benchmark Tests

Invoice Liquidity includes benchmark tests:

```bash
cargo test -p invoice_liquidity benchmark -- --nocapture
```

Results are compared against baseline:
```bash
bash scripts/check_benchmark_regression.sh
```

### 4.5 Mutation Testing (Optional)

Test mutation coverage with `cargo-mutants`:

```bash
cargo install cargo-mutants
cargo mutants --package invoice_liquidity
```

---

## Part 5: Deploying to Local Node

### 5.1 Configure Stellar CLI for Local Network

```bash
stellar network add \
  --global local \
  --rpc-url http://localhost:8000 \
  --network-passphrase "Standalone Network ; February 2021"
```

Verify:
```bash
stellar network ls
```

### 5.2 Create and Fund a Local Test Account

```bash
# Generate a key
stellar keys generate --global alice
stellar keys address alice

# Output: G...XXXX (save this)

# Fund the account (local node provides free XLM)
stellar account fund alice --network local
```

> The local Stellar network automatically funds new accounts with 10,000 XLM.

### 5.3 Build and Upload WASM

```bash
# Build the contract
cargo build-wasm

# Upload the WASM
WASM_HASH=$(stellar contract upload \
  --network local \
  --source alice \
  --wasm target/wasm32v1-none/release/invoice_liquidity.wasm | grep -oP 'WASM hash: \K\w+')

echo "WASM Hash: $WASM_HASH"
```

### 5.4 Deploy the Contract

```bash
CONTRACT_ID=$(stellar contract deploy \
  --network local \
  --source alice \
  --wasm-hash $WASM_HASH | grep -oP 'Contract ID: \K\w+')

echo "Contract ID: $CONTRACT_ID"
```

Save the contract ID for future invocations.

### 5.5 Initialize the Contract

First, create SAC (Stellar Asset Contract) tokens for testing:

```bash
# Create USDC SAC
USDC_SAC=$(stellar contract deploy \
  --network local \
  --source alice \
  --asset USDC:$(stellar keys address alice) | grep -oP 'Contract ID: \K\w+')

echo "USDC SAC: $USDC_SAC"
```

Then initialize the main contract:

```bash
stellar contract invoke \
  --network local \
  --source alice \
  --id $CONTRACT_ID \
  -- initialize \
  --admin $(stellar keys address alice) \
  --usdc_token "$USDC_SAC" \
  --xlm_sac "$(stellar keys address alice)"  # Using alice as mock for testing
```

### 5.6 Verify Contract on Local Node

```bash
stellar contract info \
  --network local \
  --id $CONTRACT_ID
```

---

## Local Development Workflow

### Typical Workflow Loop

1. **Make code changes**
   ```bash
   # Edit contract code
   vim contracts/invoice_liquidity/src/lib.rs
   ```

2. **Run unit tests**
   ```bash
   cargo test -p invoice_liquidity -- --nocapture
   ```

3. **Build WASM**
   ```bash
   cargo build-wasm
   ```

4. **Deploy to local node**
   ```bash
   stellar contract upload --network local --source alice \
     --wasm target/wasm32v1-none/release/invoice_liquidity.wasm
   stellar contract deploy --network local --source alice --wasm-hash $HASH
   ```

5. **Test on local chain**
   ```bash
   stellar contract invoke --network local --source alice \
     --id $CONTRACT_ID -- submit_invoice ...
   ```

### Quick Test Script

Create `scripts/local-test.sh`:

```bash
#!/bin/bash
set -euo pipefail

echo "=== Running unit tests ==="
cargo test -p invoice_liquidity -- --nocapture

echo "=== Building WASM ==="
cargo build-wasm

echo "=== All checks passed ==="
```

Run it:
```bash
chmod +x scripts/local-test.sh
./scripts/local-test.sh
```

---

## Common Issues and Fixes

### Issue: Docker daemon not running

**Error:**
```
Cannot connect to the Docker daemon at unix:///var/run/docker.sock. 
Is the docker daemon running?
```

**Fix:**
```bash
# Linux
sudo systemctl start docker
sudo usermod -aG docker $USER
newgrp docker

# macOS
open /Applications/Docker.app

# Windows (WSL2)
# Restart Docker Desktop
```

### Issue: Port 8000 already in use

**Error:**
```
Error starting container: bind: address already in use
```

**Fix:**
```bash
# Find the process using port 8000
lsof -i :8000

# Kill it
kill -9 <PID>

# Or use a different port in docker-compose.yml
# Change "8000:8000" to "8001:8000"
```

### Issue: "wasm32v1-none not found" when building

**Error:**
```
error: target 'wasm32v1-none-unknown' not installed
```

**Fix:**
```bash
rustup target add wasm32v1-none
rustup target list --installed
```

### Issue: Stellar CLI not found

**Error:**
```bash
stellar: command not found
```

**Fix:**
```bash
# Ensure cargo bin is in PATH
export PATH="$HOME/.cargo/bin:$PATH"

# Reinstall
cargo install --locked stellar-cli --features opt --force

# Verify
stellar --version
```

### Issue: Local node is not healthy

**Error:**
```
docker-compose: stella health check failed
```

**Fix:**
```bash
# Check logs
docker compose logs stellar

# Restart the container
docker compose restart stellar

# Wait for health
docker compose ps stellar
# Should show Status: Up (healthy)
```

### Issue: Contract deployment fails with "invalid contract"

**Error:**
```
Error: invalid contract (XDR error)
```

**Fix:**
1. Ensure WASM was built with correct target:
   ```bash
   cargo build --target wasm32v1-none --release
   file target/wasm32v1-none/release/invoice_liquidity.wasm
   # Should output: WebAssembly (wasm) binary module
   ```

2. Verify the WASM hash matches:
   ```bash
   stellar contract upload --network local --source alice \
     --wasm target/wasm32v1-none/release/invoice_liquidity.wasm
   ```

### Issue: Test timeout on property-based tests

**Error:**
```
test tests_discount_invariants::prop_discount_rate ... timeout
```

**Fix:**
```bash
# Increase test timeout
RUST_TEST_TIME_UNIT=10000 RUST_TEST_TIME_INTEGRATION=30000 cargo test

# Or skip slow tests during development
cargo test -- --skip prop_
```

### Issue: "Network not found" when invoking contract

**Error:**
```
Error: network "local" not found
```

**Fix:**
```bash
# Verify network is configured
stellar network ls

# Reconfigure if needed
stellar network remove local
stellar network add --global local \
  --rpc-url http://localhost:8000 \
  --network-passphrase "Standalone Network ; February 2021"
```

### Issue: Out of memory when building

**Error:**
```
error: could not compile 'invoice_liquidity' ... memory allocation failed
```

**Fix:**
```bash
# Clean build artifacts
cargo clean

# Increase swap (Linux)
sudo fallocate -l 4G /swapfile
sudo chmod 600 /swapfile
sudo mkswap /swapfile
sudo swapon /swapfile

# Try again with parallel jobs limited
cargo build -j 2
```

---

## Helper Scripts

### Setup Local Environment (One-Shot)

Create `scripts/setup-local-env.sh`:

```bash
#!/bin/bash
set -euo pipefail

echo "=== Setting up local development environment ==="

# Check prerequisites
echo "Checking Docker..."
if ! command -v docker &> /dev/null; then
  echo "❌ Docker not installed. Please install from https://www.docker.com"
  exit 1
fi

echo "Checking Stellar CLI..."
if ! command -v stellar &> /dev/null; then
  echo "❌ Stellar CLI not installed. Installing..."
  cargo install --locked stellar-cli --features opt
fi

echo "Checking Rust WASM target..."
if ! rustup target list --installed | grep -q wasm32v1-none; then
  echo "Adding WASM target..."
  rustup target add wasm32v1-none
fi

echo "Starting local Stellar node..."
docker compose up -d stellar

echo "Waiting for node to be healthy..."
for i in {1..30}; do
  if docker compose exec stellar curl -s http://localhost:8000/ledger > /dev/null 2>&1; then
    echo "✅ Stellar node is healthy"
    break
  fi
  echo "  Waiting... ($i/30)"
  sleep 2
done

echo "Configuring Stellar CLI..."
stellar network add --global local \
  --rpc-url http://localhost:8000 \
  --network-passphrase "Standalone Network ; February 2021" \
  --override || true

echo "Creating and funding test account..."
stellar keys generate --global alice || true
ALICE=$(stellar keys address alice)
echo "Account: $ALICE"

# Fund account
stellar account fund alice --network local || true

echo ""
echo "✅ Local development environment is ready!"
echo ""
echo "Next steps:"
echo "  1. Build contracts: cargo build-wasm"
echo "  2. Run tests: cargo test"
echo "  3. See docs/local-development.md for deployment steps"
```

Run it:
```bash
chmod +x scripts/setup-local-env.sh
./scripts/setup-local-env.sh
```

### Deploy All Contracts Locally

Create `scripts/deploy-local.sh`:

```bash
#!/bin/bash
set -euo pipefail

NETWORK="${1:-local}"
SOURCE="${2:-alice}"

echo "=== Deploying ILN contracts to $NETWORK ==="

# Build WASM
echo "Building contracts..."
cargo build --target wasm32v1-none --release

declare -A CONTRACTS=(
  ["invoice_liquidity"]="target/wasm32v1-none/release/invoice_liquidity.wasm"
  ["iln_governance"]="target/wasm32v1-none/release/iln_governance.wasm"
  ["iln_distribution"]="target/wasm32v1-none/release/iln_distribution.wasm"
  ["reputation_bonus"]="target/wasm32v1-none/release/reputation_bonus.wasm"
)

declare -A CONTRACT_IDS

for name in "${!CONTRACTS[@]}"; do
  wasm_path="${CONTRACTS[$name]}"
  
  if [[ ! -f "$wasm_path" ]]; then
    echo "❌ WASM not found: $wasm_path"
    exit 1
  fi
  
  echo ""
  echo "Deploying $name..."
  
  # Upload WASM
  WASM_HASH=$(stellar contract upload \
    --network "$NETWORK" \
    --source "$SOURCE" \
    --wasm "$wasm_path" 2>&1 | grep -oP 'WASM hash: \K\w+')
  
  # Deploy contract
  CONTRACT_ID=$(stellar contract deploy \
    --network "$NETWORK" \
    --source "$SOURCE" \
    --wasm-hash "$WASM_HASH" 2>&1 | grep -oP 'Contract ID: \K\w+')
  
  CONTRACT_IDS[$name]=$CONTRACT_ID
  echo "✅ $name deployed: $CONTRACT_ID"
done

echo ""
echo "=== Deployment Summary ==="
for name in "${!CONTRACT_IDS[@]}"; do
  echo "$name: ${CONTRACT_IDS[$name]}"
done

# Save to file for later use
cat > .contracts-local.env <<EOF
INVOICE_LIQUIDITY_ID=${CONTRACT_IDS[invoice_liquidity]}
ILN_GOVERNANCE_ID=${CONTRACT_IDS[iln_governance]}
ILN_DISTRIBUTION_ID=${CONTRACT_IDS[iln_distribution]}
REPUTATION_BONUS_ID=${CONTRACT_IDS[reputation_bonus]}
NETWORK=$NETWORK
EOF

echo ""
echo "Contract IDs saved to .contracts-local.env"
```

Run it:
```bash
chmod +x scripts/deploy-local.sh
./scripts/deploy-local.sh
```

### Stop Local Node

Create `scripts/stop-local.sh`:

```bash
#!/bin/bash
docker compose down
echo "✅ Local Stellar node stopped"
```

Run it:
```bash
chmod +x scripts/stop-local.sh
./scripts/stop-local.sh
```

---

## CI/CD Integration

### GitHub Actions Workflow

Create `.github/workflows/local-integration-tests.yml`:

```yaml
name: Local Integration Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    
    services:
      stellar:
        image: stellar/stellar-quickstart:latest
        options: >-
          --health-cmd "curl -f http://localhost:8000/ledger"
          --health-interval 5s
          --health-timeout 10s
          --health-retries 10
        ports:
          - 8000:8000

    steps:
      - uses: actions/checkout@v3
      
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: wasm32v1-none
      
      - name: Install Stellar CLI
        run: cargo install --locked stellar-cli --features opt
      
      - name: Configure local network
        run: |
          stellar network add --global local \
            --rpc-url http://localhost:8000 \
            --network-passphrase "Standalone Network ; February 2021" \
            --override
      
      - name: Run unit tests
        run: cargo test
      
      - name: Build WASM
        run: cargo build --target wasm32v1-none --release
      
      - name: Run fuzz tests
        run: cargo test -p iln_fuzz
      
      - name: Check benchmark regression
        run: bash scripts/check_benchmark_regression.sh
```

---

## Troubleshooting Tips

1. **Always check logs first:**
   ```bash
   docker compose logs stellar
   cargo test -- --nocapture
   ```

2. **Clean slate:**
   ```bash
   cargo clean
   docker compose down -v
   ./scripts/setup-local-env.sh
   ```

3. **Verify versions:**
   ```bash
   rustc --version
   cargo --version
   stellar --version
   docker --version
   ```

4. **Test one thing at a time:**
   ```bash
   # Don't test everything at once — isolate failures
   cargo test -p invoice_liquidity tests_discount_rate
   ```

5. **Use verbose output for debugging:**
   ```bash
   RUST_LOG=debug cargo test -- --nocapture --test-threads=1
   stellar contract invoke --network local ... --verbose
   ```

---

## Next Steps

- Review [Architecture.md](Architecture.md) for system design
- Read [Contract ABI](contract-abi.md) for available functions
- Check [SDK Integration Guide](sdk-integration.md) for TypeScript examples
- See [CONTRIBUTING.md](../CONTRIBUTING.md) for testing standards

---

## Support

For issues:
1. Check [Common Issues and Fixes](#common-issues-and-fixes) above
2. Review logs: `docker compose logs stellar` or `cargo test -- --nocapture`
3. Open an issue: https://github.com/directorfloo/ILN-Smart-Contract/issues
