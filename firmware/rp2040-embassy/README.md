# uPLC RP2040 Embassy Firmware

High-performance, async `no_std` Micro-VM and PLC runtime for **Raspberry Pi Pico (RP2040)** powered by the [Embassy](https://embassy.dev) asynchronous framework.

---

## 📋 Features

- **Deterministic 10ms PLC Scan Cycle**: High-precision async `Ticker` executing ladder logic scan cycles at 100 Hz (configurable in `config.rs`).
- **Hardware Digital I/O**:
  - Digital Inputs `X0..X3` configured as **Active-Low with internal Pull-Up** (`GND` = ON/Logic 1).
  - Digital Outputs `Y0..Y5` driving 3.3V GPIOs (Relays, Solenoids, Optocouplers, SSR).
  - Industrial Multi-State Status LED on `GP25` (RUN Heartbeat, STOP Beacon, ERROR Strobe).
- **Hardware Analog ADC Inputs (%IW0..%IW3)**:
  - 12-bit Analog Sampling (0..4095 raw, 0.00..3.30V) sampled synchronously every scan cycle.
  - Channels `GP26` (ADC0), `GP27` (ADC1), `GP28` (ADC2).
  - On-chip RP2040 Silicon Temperature Sensor mapped to `%IW3` and calculated into `%SD6` in $0.1^\circ\text{C}$ units.
- **IEC 61131-3 PERSISTENT / Retentive Variables**:
  - Dedicated Flash Partition in Sector 511 (`0x1FF000..0x1FFFFF`).
  - Automatically preserves 32 Bit Relays (`M64..M95`) and 32 Word Registers (`D128..D159`) across power cuts with CRC16 validation.
  - Auto-saves on PLC STOP or via Studio serial command `CMD_PERSIST_SAVE (0x08)`.
- **Special System Markers (%SM) & Special Registers (%SD)**:
  - Mitsubishi FX / Delta DVP / IEC 61131-3 compatible system diagnostic registers.
  - Real-time scan time execution telemetry (`%SD1`: Current, `%SD2`: Min, `%SD3`: Max in µs).
  - Scan counters (`%SD4..%SD5`), Watchdog Timer (`%SD0`), and CPU Temp (`%SD6`).
  - Standard clock pulses: 10ms (`%SM10`), 100ms (`%SM11`), 1.0s (`%SM12`), 1min (`%SM13`).
- **Native USB CDC-ACM Virtual Serial Port**:
  - Enumerates automatically on PC (`/dev/ttyACM*` on Linux, `COM*` on Windows).
  - Enables **live bytecode hot-reloading**, **online status monitoring**, and **real-time Data Watch Table** directly from **uPLC Studio**.
- **Modbus RTU Master/Slave (RS-485)**:
  - Hardware UART1 on `GP8` (TX) and `GP9` (RX) @ 9600 baud with non-blocking DMA background dispatcher.

---

## 📌 Hardware Pinout Table

| PLC Address | RP2040 Pin | Physical Pin | Function / Type | Notes |
|:---:|:---:|:---:|:---|:---|
| **X0** | `GP10` | Pin 14 | Digital Input 0 | Pull-Up, Active-Low (Connect to GND to trigger) |
| **X1** | `GP11` | Pin 15 | Digital Input 1 | Pull-Up, Active-Low (Connect to GND to trigger) |
| **X2** | `GP12` | Pin 16 | Digital Input 2 | Pull-Up, Active-Low (Connect to GND to trigger) |
| **X3** | `GP13` | Pin 17 | Digital Input 3 | Pull-Up, Active-Low (Connect to GND to trigger) |
| **X4..X7** | — | — | Unassigned | Reserved for future expansion |
| **Y0** | `GP15` | Pin 20 | Digital Output 0 | 3.3V Push-Pull |
| **Y1** | `GP14` | Pin 19 | Digital Output 1 | 3.3V Push-Pull |
| **Y2** | `GP6` | Pin 9 | Digital Output 2 | 3.3V Push-Pull |
| **Y3** | `GP7` | Pin 10 | Digital Output 3 | 3.3V Push-Pull |
| **Y4** | `GP2` | Pin 4 | Digital Output 4 | 3.3V Push-Pull |
| **Y5** | `GP3` | Pin 5 | Digital Output 5 | 3.3V Push-Pull |
| **Y6..Y7** | — | — | Unassigned | Reserved for future expansion |
| **AI0** (`%IW0`) | `GP26` | Pin 31 | Analog Input 0 | 12-bit ADC0 (0..4095 raw = 0..3.3V) |
| **AI1** (`%IW1`) | `GP27` | Pin 32 | Analog Input 1 | 12-bit ADC1 (0..4095 raw = 0..3.3V) |
| **AI2** (`%IW2`) | `GP28` | Pin 34 | Analog Input 2 | 12-bit ADC2 (0..4095 raw = 0..3.3V) |
| **AI3** (`%IW3`) | `Internal` | ADC Ch 4 | On-Chip Temp | Raw ADC count from RP2040 Internal Temp Sensor |
| **STATUS LED** | `GP25` | Internal | Status Indicator | Onboard Green LED (RUN / STOP / ERROR) |
| **Modbus TX** | `GP8` | Pin 11 | UART1 TX | Modbus RTU RS-485 (9600 baud) |
| **Modbus RX** | `GP9` | Pin 12 | UART1 RX | Modbus RTU RS-485 (9600 baud) |
| **USB** | `USB D+/D-` | Micro-USB | CDC-ACM Serial | Programming, Live Telemetry & Watch Table |

---

### 💡 PLC Status LED Indicators (GP25)

The onboard green LED (`GP25`) functions as a dedicated industrial PLC status indicator:

| สถานะ PLC | รูปแบบการกระพริบ (Blink Pattern) | ความถี่ / คาบเวลา | คำอธิบาย |
|:---:|:---:|:---:|:---|
| **🟢 RUN** | **Heartbeat (กระพริบเป็นจังหวะ)** หรือ **Solid ON** | 1 Hz (ติด 500ms, ดับ 500ms) | PLC กำลังรันโปรแกรม Ladder และสแกน I/O ตามปกติ (10ms scan loop alive) |
| **🟡 STOP** | **Slow Beacon (กระพริบสั้นทุก 2 วินาที)** | 0.5 Hz (ติด 60ms, ดับ 1940ms) | PLC อยู่ในสถานะ STOP หรือยังไม่มีโปรแกรมในหน่วยความจำ (Standby) |
| **🔴 ERROR** | **Fast Strobe (กระพริบเร็วถี่)** | 5 Hz (ติด 100ms, ดับ 100ms) | เกิดข้อผิดพลาด เช่น Flash บันทึกไม่สำเร็จ, CRC Packet Mismatch, Watchdog Overrun |

> [!NOTE]
> Pin mappings, pull resistors, Active-Low/High polarity, and LED patterns can be customized in [`src/config.rs`](src/config.rs).

---

## 📟 Special System Registers & Markers Architecture

uPLC Runtime provides built-in system markers and telemetry registers compatible with IEC 61131-3 and traditional PLC architectures:

### 1. Special System Markers (%SM / M8000 Series)

| IEC Address | Legacy Name | VM Bit Addr | Function / Description |
|:---:|:---:|:---:|:---|
| **`%SM0`** | `M8000` / `M96` | `0xA0` | **Always ON**: Continuously TRUE while PLC is in RUN mode |
| **`%SM1`** | `M8001` / `M97` | `0xA1` | **Always OFF**: Continuously FALSE |
| **`%SM2`** | `M8002` / `M98` | `0xA2` | **First Scan Pulse**: ON for exactly 1 scan cycle after RUN/Boot |
| **`%SM10`** | `M8011` / `M106` | `0xAA` | **10ms Clock Pulse**: 5ms ON / 5ms OFF (50 Hz) |
| **`%SM11`** | `M8012` / `M107` | `0xAB` | **100ms Clock Pulse**: 50ms ON / 50ms OFF (5 Hz) |
| **`%SM12`** | `M8013` / `M108` | `0xAC` | **1.0s Clock Pulse**: 500ms ON / 500ms OFF (1 Hz Blinker) |
| **`%SM13`** | `M8014` / `M109` | `0xAD` | **1-Minute Clock Pulse**: 30s ON / 30s OFF |
| **`%SM20`** | `M8004` / `M116` | `0xB4` | **System Error Flag**: Trips on runtime or flash errors |
| **`%SM21`** | `M8020` / `M117` | `0xB5` | **Watchdog Scan Overrun**: ON if scan time exceeds Watchdog preset |

### 2. Special System Registers (%SD / D8000 Series)

| IEC Address | Legacy Name | VM Word Addr | Function / Description | Unit / Format |
|:---:|:---:|:---:|:---|:---:|
| **`%SD0`** | `D8000` / `D128` | `0xA0` | **Watchdog Timer Preset** (Default: 100 ms) | `ms` |
| **`%SD1`** | `D8010` / `D129` | `0xA1` | **Current Scan Execution Time** | `µs` |
| **`%SD2`** | `D8011` / `D130` | `0xA2` | **Minimum Recorded Scan Execution Time** | `µs` |
| **`%SD3`** | `D8012` / `D131` | `0xA3` | **Maximum Recorded Scan Execution Time** | `µs` |
| **`%SD4`** | `D8004` / `D132` | `0xA4` | **Scan Counter (Lower 16 bits)** | Count |
| **`%SD5`** | `D8005` / `D133` | `0xA5` | **Scan Counter (Upper 16 bits)** | Count |
| **`%SD6`** | `D8006` / `D134` | `0xA6` | **RP2040 On-Chip CPU Junction Temperature** | $0.1^\circ\text{C}$ (e.g. 265 = 26.5 °C) |

---

## 💾 Flash Storage & Retentive Memory Layout

RP2040 2MB SPI Flash is partitioned into dedicated, wear-isolated sectors:

```
0x000000 ┌──────────────────────────────────────────────┐
         │ Firmware Binary (Cortex-M0+ & Embassy)       │
         │ ~150 KB (Flash base)                         │
         ├──────────────────────────────────────────────┤
         │ ... Unused Flash Storage ...                 │
0x1FE000 ├──────────────────────────────────────────────┤
         │ Sector 510: Bytecode Program Storage (4 KB)  │  <-- Hot-reloaded via USB
0x1FF000 ├──────────────────────────────────────────────┤
         │ Sector 511: IEC PERSISTENT Data Storage (4KB)│  <-- M64..M95, D128..D159
0x1FFFFF └──────────────────────────────────────────────┘
```

- **Bytecode Storage (Sector 510)**: Stores compiled 32-bit Micro-VM bytecode with CRC16 verification.
- **PERSISTENT Data Storage (Sector 511)**:
  - 32 Retentive Bit Relays: `M64..M95` (`%MX8.0..%MX11.7`)
  - 32 Retentive Word Registers: `D128..D159` (`%MW128..%MW159`)
  - Stored with 8-byte header (`PERS` Magic + Payload Length + CRC16).
  - Automatically restored on boot and committed to Flash upon entering STOP mode or receiving `CMD_PERSIST_SAVE (0x08)`.

---

## 🛠️ Prerequisites

Before building or flashing, ensure the following tools are installed:

### 1. ARM Cortex-M0+ Rust Target
```bash
rustup target add thumbv6m-none-eabi
```

### 2. Stack Overflow Protection Linker (`flip-link`)
```bash
cargo install flip-link
```

### 3. Flashing Tools (Choose One or Both)
- **Option A: Hardware Debug Probe (`probe-rs`)** *(Recommended for development & RTT logs)*:
  ```bash
  cargo install probe-rs-tools --locked
  ```
- **Option B: USB BOOTSEL Mode (`elf2uf2-rs`)** *(No debugger required)*:
  ```bash
  cargo install elf2uf2-rs --locked
  ```
  *(Or install `picotool` via package manager: `sudo apt install picotool`)*

---

## 🔨 How to Build

Navigate to the firmware directory:

```bash
cd firmware/rp2040-embassy
```

### Build Release Binary
```bash
cargo build --release
```

The resulting ELF binary will be generated at:
```
target/thumbv6m-none-eabi/release/uplc-rp2040-embassy
```

---

## ⚡ How to Flash

### Method 1: Flash via Debug Probe (CMSIS-DAP / Raspberry Pi Debug Probe)

Connect SWD pins: `SWDIO`, `SWCLK`, and `GND` from your debug probe to the Pico debug header.

Run directly via Cargo (configured in `.cargo/config.toml`):
```bash
cargo run --release
```

Or manually using `probe-rs`:
```bash
probe-rs run --chip RP2040 --protocol swd target/thumbv6m-none-eabi/release/uplc-rp2040-embassy
```

> [!TIP]
> This method will automatically flash the MCU and attach to **RTT** (`defmt`) to stream real-time debug and telemetry logs to your terminal.

---

### Method 2: Flash via USB BOOTSEL Mode (No Debug Probe Needed)

1. Unplug the Raspberry Pi Pico.
2. Hold down the white **BOOTSEL** button on the Pico board while plugging the USB cable into your computer.
3. Release the **BOOTSEL** button. The Pico will mount as a mass-storage drive named `RPI-RP2`.
4. Flash using `elf2uf2-rs`:
   ```bash
   elf2uf2-rs -d -s target/thumbv6m-none-eabi/release/uplc-rp2040-embassy
   ```
   *(The `-d` flag auto-detects the mounted drive, flashes the firmware, and automatically resets the Pico).*

#### Alternative with `picotool`:
```bash
picotool load -x target/thumbv6m-none-eabi/release/uplc-rp2040-embassy
```

---

## 💻 Connecting with uPLC Studio Desktop

Once flashed, the Pico runs the uPLC Micro-VM autonomously.

1. Connect the Pico to your computer via USB (native USB CDC port).
2. Check the enumerated port:
   - **Linux**: `/dev/ttyACM0` or `/dev/ttyACM1`
   - **Windows**: `COM3`, `COM4`, etc.
3. Open **uPLC Studio**:
   ```bash
   cargo run -p uplc-studio
   ```
4. In uPLC Studio:
   - Select the detected port in the bottom **Status Bar** or **Settings**.
   - Click **"⚡ Update to PLC"** (`Ctrl+U`) to download your ladder logic directly to the running RP2040 over USB.
   - Click **"Online Monitor"** to observe real-time power flow and live I/O states on the ladder diagram!
   - Open **`📊 Watch`** in the right panel to monitor and force data registers and view real-time ADC and CPU temperature telemetry.
