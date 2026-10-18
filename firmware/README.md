# uPLC Firmware

This folder contains hardware runtime implementations and firmware packages for the **uPLC** ecosystem.

---

## Target Platforms

| Target                               | Framework                        |        Status        | Directory                                       |
| ------------------------------------ | -------------------------------- | :------------------: | ----------------------------------------------- |
| **Raspberry Pi Pico (RP2040)** | Rust Embassy (`no_std`, async) | ✅ Stable & Verified | [`rp2040-embassy`](./rp2040-embassy/README.md) |

---

## Quick Start (RP2040)

For complete pinouts, wiring diagrams, build instructions, and flashing options:
👉 Please refer to the **[RP2040 Embassy Firmware Guide](./rp2040-embassy/README.md)**.

### Quick Build & Flash Commands:

```bash
cd firmware/rp2040-embassy

# 1. Build
cargo build --release

# 2. Flash via Probe (SWD / CMSIS-DAP)
cargo run --release

# OR Flash via USB BOOTSEL (RPI-RP2 drive)
elf2uf2-rs -d -s target/thumbv6m-none-eabi/release/uplc-rp2040-embassy
```
