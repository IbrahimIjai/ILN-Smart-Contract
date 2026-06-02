#!/bin/bash
# Deploy all ILN contracts to Stellar testnet and emit a summary file.

set -euo pipefail

NETWORK="testnet"
SOURCE="deployer"
SUMMARY_FILE="deploy-summary.json"
ENV_FILE=".contracts-${NETWORK}.env"

if [[ -z "${STELLAR_TESTNET_DEPLOYER_SECRET:-}" ]]; then
  echo "Missing STELLAR_TESTNET_DEPLOYER_SECRET in environment." >&2
  exit 1
fi

# Ensure network config exists.
if ! stellar network ls | grep -q "^${NETWORK}$"; then
  stellar network add --global ${NETWORK} \
    --rpc-url https://soroban-testnet.stellar.org \
    --network-passphrase "Test SDF Network ; September 2015"
fi

# Ensure deployer key exists.
if ! stellar keys address "${SOURCE}" &> /dev/null; then
  stellar keys add --global "${SOURCE}" --secret-key "${STELLAR_TESTNET_DEPLOYER_SECRET}"
fi

# Build optimized WASM.
cargo build --target wasm32v1-none --release

CONTRACT_NAMES=(
  invoice_liquidity
  iln_governance
  iln_distribution
  reputation_bonus
)

declare -A CONTRACTS=(
  ["invoice_liquidity"]="target/wasm32v1-none/release/invoice_liquidity.wasm"
  ["iln_governance"]="target/wasm32v1-none/release/iln_governance.wasm"
  ["iln_distribution"]="target/wasm32v1-none/release/iln_distribution.wasm"
  ["reputation_bonus"]="target/wasm32v1-none/release/reputation_bonus.wasm"
)

declare -A CONTRACT_IDS

for contract_name in "${CONTRACT_NAMES[@]}"; do
  wasm_path="${CONTRACTS[$contract_name]}"

  if [[ ! -f "${wasm_path}" ]]; then
    echo "WASM not found: ${wasm_path}" >&2
    exit 1
  fi

  upload_output=$(stellar contract upload \
    --network "${NETWORK}" \
    --source "${SOURCE}" \
    --wasm "${wasm_path}" 2>&1)

  wasm_hash=$(echo "${upload_output}" | grep -oP 'WASM hash: \K[a-f0-9]+' || true)
  if [[ -z "${wasm_hash}" ]]; then
    echo "Failed to upload WASM for ${contract_name}" >&2
    echo "${upload_output}" >&2
    exit 1
  fi

  deploy_output=$(stellar contract deploy \
    --network "${NETWORK}" \
    --source "${SOURCE}" \
    --wasm-hash "${wasm_hash}" 2>&1)

  contract_id=$(echo "${deploy_output}" | grep -oP 'Contract ID: \K[A-Z0-9]+' || true)
  if [[ -z "${contract_id}" ]]; then
    echo "Failed to deploy ${contract_name}" >&2
    echo "${deploy_output}" >&2
    exit 1
  fi

  CONTRACT_IDS[${contract_name}]="${contract_id}"
  echo "${contract_name}=${contract_id}"
done

cat > "${ENV_FILE}" <<EOF
# Contract IDs for ${NETWORK} network
# Generated: $(date -u +"%Y-%m-%dT%H:%M:%SZ")

INVOICE_LIQUIDITY_ID=${CONTRACT_IDS[invoice_liquidity]}
ILN_GOVERNANCE_ID=${CONTRACT_IDS[iln_governance]}
ILN_DISTRIBUTION_ID=${CONTRACT_IDS[iln_distribution]}
REPUTATION_BONUS_ID=${CONTRACT_IDS[reputation_bonus]}
NETWORK=${NETWORK}
SOURCE=${SOURCE}
EOF

cat > "${SUMMARY_FILE}" <<EOF
{
  "network": "${NETWORK}",
  "invoice_liquidity": "${CONTRACT_IDS[invoice_liquidity]}",
  "iln_governance": "${CONTRACT_IDS[iln_governance]}",
  "iln_distribution": "${CONTRACT_IDS[iln_distribution]}",
  "reputation_bonus": "${CONTRACT_IDS[reputation_bonus]}"
}
EOF

echo "Summary written to ${SUMMARY_FILE}"
