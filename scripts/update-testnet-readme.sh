#!/bin/bash
# Update README testnet contract IDs from .contracts-testnet.env

set -euo pipefail

ENV_FILE=".contracts-testnet.env"
README_FILE="README.md"

if [[ ! -f "${ENV_FILE}" ]]; then
  echo "Missing ${ENV_FILE}. Run deploy script first." >&2
  exit 1
fi

# shellcheck disable=SC1090
source "${ENV_FILE}"

if [[ -z "${INVOICE_LIQUIDITY_ID:-}" || -z "${ILN_GOVERNANCE_ID:-}" || -z "${ILN_DISTRIBUTION_ID:-}" || -z "${REPUTATION_BONUS_ID:-}" ]]; then
  echo "Missing one or more contract IDs in ${ENV_FILE}." >&2
  exit 1
fi

TESTNET_USDC_SAC="CBIELTK6YBZJU5UP2WWQEUCYKLPU6AUNZ2BQ4WWFEIE3USCIHMXQDAMA"

TABLE_CONTENT=$(cat <<EOF
| Resource | Contract ID | Notes |
|----------|-------------|-------|
| **`invoice_liquidity`** | `${INVOICE_LIQUIDITY_ID}` | Primary integration contract; used in [SDK examples](docs/sdk-integration.md) |
| **`iln_governance`** | `${ILN_GOVERNANCE_ID}` | Governance proposals and voting |
| **`iln_distribution`** | `${ILN_DISTRIBUTION_ID}` | Rewards distribution |
| **`reputation_bonus`** | `${REPUTATION_BONUS_ID}` | Reputation-based bonus rules |
| **Testnet USDC (SAC)** | `${TESTNET_USDC_SAC}` | Referenced in SDK integration guide |
EOF
)

export README_TESTNET_TABLE="${TABLE_CONTENT}"

python3 - <<'PY'
import os
import re
import sys

path = os.environ.get("README_FILE", "README.md")
start = "<!-- TESTNET_CONTRACT_IDS_START -->"
end = "<!-- TESTNET_CONTRACT_IDS_END -->"

with open(path, "r", encoding="utf-8") as handle:
    content = handle.read()

if start not in content or end not in content:
    print("Missing testnet contract markers in README.md", file=sys.stderr)
    sys.exit(1)

table = os.environ["README_TESTNET_TABLE"].rstrip()
replacement = f"{start}\n{table}\n{end}"

pattern = re.compile(rf"{re.escape(start)}.*?{re.escape(end)}", re.S)
updated = pattern.sub(replacement, content)

if updated == content:
    print("README already up to date")
else:
    with open(path, "w", encoding="utf-8") as handle:
        handle.write(updated)
    print("README updated")
PY
