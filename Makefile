.PHONY: build test fuzz changelog soroban-optimize spec

build:
	cargo build --target wasm32v1-none --release

# Generate the contract ABI/spec JSON (Issue #111).
# Every `pub fn` in the `#[contractimpl]` block is exported into the Soroban
# spec automatically. When the `stellar` CLI and a built WASM are available we
# emit the canonical embedded spec; otherwise we fall back to a toolchain-free
# source generator that produces an equivalent JSON. Output: docs/contract-spec.json
spec:
	@if command -v stellar >/dev/null 2>&1 && [ -f target/wasm32v1-none/release/invoice_liquidity.wasm ]; then \
		stellar contract inspect --wasm target/wasm32v1-none/release/invoice_liquidity.wasm --output json > docs/contract-spec.json && \
		echo "✅ spec from 'stellar contract inspect' -> docs/contract-spec.json"; \
	else \
		echo "ℹ️  stellar CLI/WASM not found; generating spec from source"; \
		npx --yes tsx scripts/gen-spec.ts; \
	fi

soroban-optimize:
	cargo build --release --target wasm32-unknown-unknown

test:
	cargo test

fuzz:
	cargo test -p iln_fuzz

# Generate CHANGELOG.md from conventional commits using git-cliff.
# Install: cargo install git-cliff
# Usage:
#   make changelog          # update for unreleased commits
#   make changelog TAG=v1.0.0  # generate up to a specific tag
changelog:
	git cliff $(if $(TAG),--tag $(TAG)) --output CHANGELOG.md
