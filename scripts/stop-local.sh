#!/bin/bash
# Stop the local Stellar node and clean up Docker resources
#
# Usage: ./scripts/stop-local.sh

set -euo pipefail

GREEN='\033[0;32m'
NC='\033[0m'

echo "Stopping local Stellar node..."
docker compose down

echo -e "${GREEN}✅ Local Stellar node stopped${NC}"
echo ""
echo "To start again: docker compose up -d stellar"
echo "To remove all data: docker compose down -v"
