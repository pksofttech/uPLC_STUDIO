#!/usr/bin/env bash
# Build + upload uPLC STM32F103C6T6 firmware with ST-Link V2 (SWD)
#
# Usage:
#   ./flash.sh                 # build release + flash with probe-rs (default)
#   ./flash.sh run             # flash + attach RTT/defmt log viewer
#   ./flash.sh openocd         # flash with OpenOCD (stlink.cfg + stm32f1x.cfg)
#   ./flash.sh stflash         # flash with st-flash (stlink-tools) from .bin
#   ./flash.sh erase           # full chip erase (clears saved PLC program + RETAIN)
#   ./flash.sh bin             # only generate .bin / .hex into ./dist
#
# Wiring ST-Link V2 -> Blue Pill:  SWDIO->PA13(DIO)  SWCLK->PA14(CLK)  GND->GND  3.3V->3.3V
# Clone chips (CKS32/APM32/GD32): set EXTRA="--connect-under-reset" or CHIP=... if detection fails.
set -euo pipefail
cd "$(dirname "$0")"

CHIP="${CHIP:-STM32F103C6Tx}"
SPEED="${SPEED:-1800}"
EXTRA="${EXTRA:-}"
ELF=target/thumbv7m-none-eabi/release/uplc-stm32f103c6-embassy
MODE="${1:-flash}"

cargo build --release
arm-none-eabi-size "$ELF" 2>/dev/null || true

make_bin() {
    mkdir -p dist
    if command -v rust-objcopy >/dev/null; then OBJCOPY=rust-objcopy; else OBJCOPY=arm-none-eabi-objcopy; fi
    "$OBJCOPY" -O binary "$ELF" dist/uplc-stm32f103c6.bin
    "$OBJCOPY" -O ihex "$ELF" dist/uplc-stm32f103c6.hex
    echo "Generated dist/uplc-stm32f103c6.bin / .hex"
}

case "$MODE" in
    flash)   probe-rs download --chip "$CHIP" --protocol swd --speed "$SPEED" $EXTRA "$ELF"
             probe-rs reset --chip "$CHIP" --protocol swd $EXTRA ;;
    run)     probe-rs run --chip "$CHIP" --protocol swd --speed "$SPEED" $EXTRA "$ELF" ;;
    openocd) openocd -f interface/stlink.cfg -f target/stm32f1x.cfg \
                 -c "program $ELF verify reset exit" ;;
    stflash) make_bin
             st-flash --reset write dist/uplc-stm32f103c6.bin 0x08000000 ;;
    erase)   probe-rs erase --chip "$CHIP" --protocol swd $EXTRA ;;
    bin)     make_bin ;;
    *)       echo "unknown mode: $MODE"; exit 1 ;;
esac
