#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../" && pwd)"
cd "${REPO_ROOT}"

VERSION="$(grep -m1 '^version\s*=' Cargo.toml | sed -E 's/.*"([^"]+)".*/\1/')"
TARGET="$(rustc -vV | sed -n 's|host: ||p')"
DIST_DIR="${REPO_ROOT}/dist"
STAGE_DIR="${DIST_DIR}/stage"
ARCHIVE_NAME="laduino-plc-v${VERSION}-${TARGET}.tar.gz"

echo "==> [1/4] Running full workspace test suite..."
cargo test --workspace

echo "==> [2/4] Building release desktop binary (laduino-plc v${VERSION} for ${TARGET})..."
cargo build --release --package laduino-plc

echo "==> [3/4] Preparing distribution directory..."
rm -rf "${STAGE_DIR}"
mkdir -p "${STAGE_DIR}/firmware" "${DIST_DIR}"

cp "target/release/laduino-plc" "${STAGE_DIR}/"
cp -r "example" "${STAGE_DIR}/"

if [ -f "firmware/uplc-rp2040-embassy.uf2" ]; then
    cp "firmware/uplc-rp2040-embassy.uf2" "${STAGE_DIR}/firmware/"
elif [ -f "firmware/rp2040-embassy/target/thumbv6m-none-eabi/release/uplc-rp2040-embassy.uf2" ]; then
    cp "firmware/rp2040-embassy/target/thumbv6m-none-eabi/release/uplc-rp2040-embassy.uf2" "${STAGE_DIR}/firmware/uplc-rp2040-embassy.uf2"
else
    echo "ℹ Note: No pre-built .uf2 found yet. Run .agents/skills/rp2040-firmware-uf2/scripts/build_uf2.sh to include it."
fi

echo "==> [4/4] Creating self_update release archive: dist/${ARCHIVE_NAME}..."
(cd "${STAGE_DIR}" && tar -czf "${DIST_DIR}/${ARCHIVE_NAME}" .)
rm -rf "${STAGE_DIR}"

SHA256="$(sha256sum "${DIST_DIR}/${ARCHIVE_NAME}" | awk '{print $1}')"
echo "✔ Created: dist/${ARCHIVE_NAME}"
echo "✔ SHA-256: ${SHA256}"
