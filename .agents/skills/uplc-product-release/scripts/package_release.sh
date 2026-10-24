#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../../" && pwd)"
cd "${REPO_ROOT}"

VERSION="$(grep -m1 '^version\s*=' Cargo.toml | sed -E 's/.*"([^"]+)".*/\1/')"
RELEASES_DIR="${REPO_ROOT}/releases"

echo "==> [1/4] Building RP2040 Micro-VM (.uf2) firmware..."
bash .agents/skills/rp2040-firmware-uf2/scripts/build_uf2.sh

echo "==> [2/4] Building Linux release binary (x86_64-unknown-linux-gnu)..."
cargo build --release --package laduino-plc

echo "==> [3/4] Building Windows release binary (x86_64-pc-windows-gnu)..."
if command -v cross >/dev/null 2>&1; then
    cross build --release --package laduino-plc --target x86_64-pc-windows-gnu
else
    cargo build --release --package laduino-plc --target x86_64-pc-windows-gnu
fi

echo "==> [4/4] Packaging releases into ${RELEASES_DIR}/..."
rm -rf "${RELEASES_DIR}"
mkdir -p "${RELEASES_DIR}/linux/firmware" \
         "${RELEASES_DIR}/windows/firmware" \
         "${RELEASES_DIR}/rp2040"

# 1. RP2040 Micro-VM (.uf2)
cp "firmware/uplc-rp2040-embassy.uf2" "${RELEASES_DIR}/uplc-rp2040-embassy.uf2"
cp "firmware/uplc-rp2040-embassy.uf2" "${RELEASES_DIR}/rp2040/uplc-rp2040-embassy.uf2"

# 2. Linux bundle & archive
cp "target/release/laduino-plc" "${RELEASES_DIR}/linux/laduino-plc"
chmod +x "${RELEASES_DIR}/linux/laduino-plc"
cp "firmware/uplc-rp2040-embassy.uf2" "${RELEASES_DIR}/linux/firmware/uplc-rp2040-embassy.uf2"
cp -r "example" "${RELEASES_DIR}/linux/"

LINUX_ARCHIVE="laduino-plc-v${VERSION}-x86_64-unknown-linux-gnu.tar.gz"
(cd "${RELEASES_DIR}/linux" && tar -czf "${RELEASES_DIR}/${LINUX_ARCHIVE}" laduino-plc firmware example)

# 3. Windows bundle & archives
cp "target/x86_64-pc-windows-gnu/release/laduino-plc.exe" "${RELEASES_DIR}/windows/laduino-plc.exe"
cp "firmware/uplc-rp2040-embassy.uf2" "${RELEASES_DIR}/windows/firmware/uplc-rp2040-embassy.uf2"
cp -r "example" "${RELEASES_DIR}/windows/"

WIN_GNU_ARCHIVE="laduino-plc-v${VERSION}-x86_64-pc-windows-gnu.zip"
WIN_MSVC_ARCHIVE="laduino-plc-v${VERSION}-x86_64-pc-windows-msvc.zip"
(cd "${RELEASES_DIR}/windows" && zip -qr "${RELEASES_DIR}/${WIN_GNU_ARCHIVE}" laduino-plc.exe firmware example)
cp "${RELEASES_DIR}/${WIN_GNU_ARCHIVE}" "${RELEASES_DIR}/${WIN_MSVC_ARCHIVE}"

# 4. Generate SHA-256 checksums
(cd "${RELEASES_DIR}" && sha256sum \
    "${LINUX_ARCHIVE}" \
    "${WIN_GNU_ARCHIVE}" \
    "${WIN_MSVC_ARCHIVE}" \
    "uplc-rp2040-embassy.uf2" > SHA256SUMS.txt)

echo "✔ Releases packaged successfully in ${RELEASES_DIR}/"
cat "${RELEASES_DIR}/SHA256SUMS.txt"
