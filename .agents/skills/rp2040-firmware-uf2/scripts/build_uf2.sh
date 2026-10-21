#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../" && pwd)"
FW_DIR="${REPO_ROOT}/firmware/rp2040-embassy"
ELF_OUT="${FW_DIR}/target/thumbv6m-none-eabi/release/uplc-rp2040-embassy"
UF2_OUT="${REPO_ROOT}/firmware/uplc-rp2040-embassy.uf2"

echo "==> [1/3] Checking Rust Cortex-M0+ target (thumbv6m-none-eabi)..."
if ! rustup target list --installed | grep -q "thumbv6m-none-eabi"; then
    rustup target add thumbv6m-none-eabi
fi

if ! command -v flip-link >/dev/null 2>&1; then
    echo "Installing flip-link..."
    cargo install flip-link
fi

echo "==> [2/3] Building RP2040 Embassy firmware (--release)..."
(cd "${FW_DIR}" && cargo build --release)

echo "==> [3/3] Converting ELF binary to UF2 image..."
if command -v elf2uf2-rs >/dev/null 2>&1; then
    elf2uf2-rs "${ELF_OUT}" "${UF2_OUT}"
    echo "✔ Generated UF2 firmware: ${UF2_OUT} ($(stat -c%s "${UF2_OUT}" 2>/dev/null || wc -c < "${UF2_OUT}") bytes)"
elif command -v picotool >/dev/null 2>&1; then
    picotool uf2 convert "${ELF_OUT}" -t elf "${UF2_OUT}"
    echo "✔ Generated UF2 firmware via picotool: ${UF2_OUT}"
else
    echo "⚠ Neither 'elf2uf2-rs' nor 'picotool' found. Install with: cargo install elf2uf2-rs --locked"
    echo "  ELF binary is available at: ${ELF_OUT}"
    exit 1
fi
