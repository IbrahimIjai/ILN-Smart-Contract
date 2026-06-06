# WASM Binary Size

This document records the optimized WASM build command for ILN smart contracts and the current size guidance for the generated binaries.

## Optimized build command

```bash
cargo build --release --target wasm32-unknown-unknown
```

The repository also exposes a Makefile shortcut:

```bash
make soroban-optimize
```

## Current WASM size guidance

- Optimized Soroban contract binaries are expected to be small, typically in the range of 10 KB to 80 KB per contract when built with the release profile.
- Actual size should be verified after building with the command above.

## Verification

After building, check the size of the generated WASM file:

```bash
wc -c target/wasm32-unknown-unknown/release/<contract>.wasm
```

or

```bash
stat -c%s target/wasm32-unknown-unknown/release/<contract>.wasm
```
