# uPLC Firmware

This folder contains hardware runtime implementations and firmware packages for the **uPLC** ecosystem.

---

## Target Platforms

| Target | Architecture | Framework | Status | Documentation |
| :--- | :--- | :--- | :---: | :--- |
| **Raspberry Pi Pico (RP2040)** | Dual ARM Cortex-M0+ @ 125MHz | Rust Embassy (`no_std`, async) | ✅ Stable & Verified | [`rp2040-embassy`](./rp2040-embassy/README.md) |

---

## Architecture & Features

The uPLC firmware runtime provides industrial-grade PLC capabilities on bare-metal microcontrollers:

- **High-Speed Cyclic PLC Scan Loop**: Configurable cycle time (default 10 ms) with scan time metrics recorded in `%SD2`–`%SD4`.
- **Digital I/O Engine**: 8 opto-isolated or pull-up digital inputs (`GP0..GP7` / `%IX0.0..%IX0.7`) and 8 digital outputs (`GP8..GP15` / `%QX0.0..%QX0.7`).
- **Hardware Analog ADC**: 12-bit ADC inputs (`GP26..GP28` / `%IW0..%IW2`), expansion `%IW3`, and on-chip junction temperature sensor recorded in $0.1^\circ\text{C}$ (`%SD6`).
- **IEC 61131-3 Non-Volatile Persistence**: Dedicated SPI Flash storage (Sector 511) for retentive variables (`%MX64..%MX71` / `M64..M71` and `%MW128..%MW135` / `D128..D135`) with CRC16 integrity verification and wear-leveling preservation.
- **Special System Registers & Markers**: Standard Mitsubishi / IEC 61131-3 system bits (`%SM0..%SM21`) and registers (`%SD0..%SD6`) for first scan, clock pulses (10ms, 100ms, 1s, 1min), scan diagnostics, error codes, and temperature.
- **Micro-VM Instruction Set**: Full support for LD, LDI, OUT, SET, RST, AND, ANI, OR, ORI, TON, TOF, CTU, CTD, ADD, SUB, MUL, DIV, CMP, MOV.
- **Real-Time USB-CDC Protocol**: Live packet protocol for bytecode download, RUN/STOP control, live telemetry snapshot streaming, and variable forcing.

---

## Quick Start (RP2040)

For complete pinouts, wiring diagrams, memory map, and flashing options:
👉 Please refer to the **[RP2040 Embassy Firmware Guide](./rp2040-embassy/README.md)**.

### Quick Build & Flash Commands:

```bash
cd firmware/rp2040-embassy

# 1. Build release binary
cargo build --release

# 2. Flash via Probe (SWD / CMSIS-DAP)
cargo run --release

# OR Flash via USB BOOTSEL (Hold BOOTSEL button while plugging in USB)
elf2uf2-rs -d -s target/thumbv6m-none-eabi/release/uplc-rp2040-embassy
```
