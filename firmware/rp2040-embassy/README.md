# uPLC RP2040 Embassy Firmware

High-performance, async `no_std` Micro-VM and PLC runtime for **Raspberry Pi Pico (RP2040)** powered by the [Embassy](https://embassy.dev) asynchronous framework.

---

## 📋 Features

- **Deterministic 10ms PLC Scan Cycle**: High-precision async `Ticker` executing ladder logic scan cycles at 100 Hz.
- **Hardware I/O Pin Mapping**:
  - Digital Inputs `X0..X3` configured as **Active-Low with internal Pull-Up** (`GND` = ON/Logic 1).
  - Digital Outputs `Y0..Y5` driving 3.3V GPIOs.
  - Onboard Status LED (`GP25`) mirrors output `Y0` for instant visual feedback.
- **Native USB CDC-ACM Virtual Serial Port**:
  - Enumerates automatically on PC (e.g., `/dev/ttyACM*` on Linux, `COM*` on Windows).
  - Enables **live bytecode hot-reloading**, **online status monitoring**, and **telemetry** directly from the **uPLC Studio** desktop app.
- **On-Chip Flash Persistence**:
  - Bytecode uploaded from uPLC Studio is automatically committed to the top 8 KB of the RP2040 SPI Flash (`0x1FE000`).
  - Programs persist across power cycles and reboots.
- **Modbus RTU Master/Slave (RS-485)**:
  - Hardware UART1 on `GP8` (TX) and `GP9` (RX) @ 9600 baud.

---

## 📌 Hardware Pinout Table

| PLC Address | RP2040 Pin | Physical Pin | Function / Type | Notes |
|:---:|:---:|:---:|:---|:---|
| **X0** | `GP10` | Pin 14 | Digital Input | Pull-Up, Active-Low (Connect to GND to trigger) |
| **X1** | `GP11` | Pin 15 | Digital Input | Pull-Up, Active-Low (Connect to GND to trigger) |
| **X2** | `GP12` | Pin 16 | Digital Input | Pull-Up, Active-Low (Connect to GND to trigger) |
| **X3** | `GP13` | Pin 17 | Digital Input | Pull-Up, Active-Low (Connect to GND to trigger) |
| **X4..X7** | — | — | Unassigned | Disabled |
| **Y0** | `GP15` | Pin 20 | Digital Output | 3.3V Push-Pull |
| **Y1** | `GP14` | Pin 19 | Digital Output | 3.3V Push-Pull |
| **Y2** | `GP6` | Pin 9 | Digital Output | 3.3V Push-Pull |
| **Y3** | `GP7` | Pin 10 | Digital Output | 3.3V Push-Pull |
| **Y4** | `GP2` | Pin 4 | Digital Output | 3.3V Push-Pull |
| **Y5** | `GP3` | Pin 5 | Digital Output | 3.3V Push-Pull |
| **Y6..Y7** | — | — | Unassigned | Disabled |
| **STATUS LED** | `GP25` | Internal | PLC Status Indicator | Onboard Green LED (RUN / STOP / ERROR) |
| **Modbus TX** | `GP8` | Pin 11 | UART1 TX | Modbus RTU RS-485 (9600 baud) |
| **Modbus RX** | `GP9` | Pin 12 | UART1 RX | Modbus RTU RS-485 (9600 baud) |
| **USB** | `USB D+/D-` | Micro-USB | CDC-ACM Serial | Programming & Live Telemetry |

---

### 💡 PLC Status LED Indicators (GP25)

The onboard green LED (`GP25`) functions as a dedicated industrial PLC status indicator:

| สถานะ PLC | รูปแบบการกระพริบ (Blink Pattern) | ความถี่ / คาบเวลา | คำอธิบาย |
|:---:|:---:|:---:|:---|
| **🟢 RUN** | **Heartbeat (กระพริบช้าเป็นจังหวะ)** หรือ **Solid ON** | 1 Hz (ติด 500ms, ดับ 500ms) | PLC กำลังรันโปรแกรม Ladder และสแกน I/O ตามปกติ (10ms scan loop alive) |
| **🟡 STOP** | **Slow Beacon (กระพริบสั้นทุก 2 วินาที)** | 0.5 Hz (ติด 60ms, ดับ 1940ms) | PLC อยู่ในสถานะ STOP หรือยังไม่มีโปรแกรมในหน่วยความจำ (Standby) |
| **🔴 ERROR** | **Fast Strobe (กระพริบเร็วถี่)** | 5 Hz (ติด 100ms, ดับ 100ms) | เกิดข้อผิดพลาด เช่น Flash บันทึกไม่สำเร็จ, CRC Packet Mismatch, Watchdog |

> [!NOTE]
> Pin configurations and RUN LED mode can be customized in [`src/config.rs`](src/config.rs).

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
