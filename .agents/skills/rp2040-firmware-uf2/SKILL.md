---
name: rp2040-firmware-uf2
description: >-
  Use this skill when building, converting, or flashing the RP2040 Embassy Micro-VM
  firmware (firmware/rp2040-embassy), generating pre-built .uf2 firmware images for
  product distribution, or working on the USB BOOTSEL flasher (firmware_flasher.rs)
  and USB-CDC serial protocol commands.
---

# RP2040 Firmware Build & UF2 Packaging Skill

This skill provides the standard workflow for building the `uplc-rp2040-embassy` microcontroller firmware, producing `.uf2` images for zero-dependency USB flashing in **uPLC Studio**, and verifying the USB-CDC serial protocol.

## Architecture Overview

- **Firmware Crate**: [Cargo.toml](../../../firmware/rp2040-embassy/Cargo.toml) (`thumbv6m-none-eabi`, excluded from root workspace so desktop builds do not require embedded toolchains).
- **Shared Micro-VM**: [lib.rs](../../../crates/vm/src/lib.rs) (`#![no_std]` 32-bit fixed-width instruction VM shared between the desktop simulator and RP2040 MCU).
- **Desktop USB Flasher UI**: [firmware_flasher.rs](../../../crates/ui/src/firmware_flasher.rs) (flashes `.uf2` directly to mounted `RPI-RP2` USB Mass Storage drives and sends `CMD_ENTER_BOOTSEL = 0x0A` over USB-CDC).
- **Flash Memory Layout (2MB RP2040 Flash)**:
  - `0x000000`: Firmware binary (`~150 KB`)
  - `0x1FE000` (Sector 510): User Ladder Bytecode Program (`4 KB`, CRC16 verified)
  - `0x1FF000` (Sector 511): IEC 61131-3 `RETAIN` / Persistent variables (`M64..M95`, `D96..D127` / `%MW96..%MW127`)

## Step-by-Step Procedure

### 1. Build Release Firmware & Generate `.uf2`

Run the helper script [build_uf2.sh](./scripts/build_uf2.sh) from the repository root:

```bash
bash .agents/skills/rp2040-firmware-uf2/scripts/build_uf2.sh
```

What this script does:
1. Checks that the `thumbv6m-none-eabi` target and `flip-link` are installed.
2. Runs `cargo build --release` inside `firmware/rp2040-embassy`.
3. Converts `target/thumbv6m-none-eabi/release/uplc-rp2040-embassy` into `firmware/uplc-rp2040-embassy.uf2` (using `elf2uf2-rs` or `picotool`).

### 2. USB Flashing Workflow in Product Mode

End users do **not** compile firmware from source. Instead, **uPLC Studio** distributes the pre-built `firmware/uplc-rp2040-embassy.uf2` alongside the application binary:
1. If the RP2040 is running in normal PLC mode (`/dev/ttyACM*` or `COM*`), clicking **`⚡ Reboot PLC to BOOTSEL Mode`** sends `PlcSerialCommand::EnterBootsel` (`0x0A`).
2. The MCU saves `RETAIN` variables to Flash Sector 511 and invokes `embassy_rp::rom_data::reset_to_usb_boot(0, 0)`.
3. Once the `RPI-RP2` drive (`INFO_UF2.TXT`) is mounted, clicking **`🔥 Flash Firmware via USB`** copies the `.uf2` file directly to `RPI-RP2/uplc_firmware.uf2`.

### 3. Validation Steps

After modifying `crates/vm`, `crates/core/src/bytecode.rs`, or `crates/ui/src/firmware_flasher.rs`, always run:

```bash
cargo test --workspace
```
