# uPLC Studio — STM32F103C6T6 Embassy Micro-VM Firmware

Firmware for **STM32F103C6T6** (Cortex-M3 @ 72 MHz, **32 KB Flash / 10 KB SRAM**, Blue Pill–style boards).
It runs the same `uplc-vm` engine and the same serial protocol as `firmware/rp2040-embassy`, but connects to
uPLC Studio over **USART1 (PA9/PA10)** through a USB-TTL adapter. You upload it with an **ST-Link V2**.

## Resource budget (release build)

| Resource | Used | Available |
|---|---|---|
| Flash (code) | ~26.9 KB | 29 KB (pages 0..28) |
| Static RAM (.data + .bss + RTT) | ~7.4 KB | 10 KB |
| Stack (flip-link, overflow -> HardFault) | ~2.8 KB | — |
| PLC program | **256 instructions max** (`uplc-vm` `compact` feature) | RP2040: 512 |

## Pinout

| Function | Pin |
|---|---|
| X0..X7 (`%IX0.0..0.7`, pull-up, active-low) | PB0, PB1, PB10, PB11, PB5, PB6, PB7, PA6 |
| Y0..Y7 (`%QX0.0..0.7`) | PB12, PB13, PB14, PB15, PA8, PB8, PB9, PA7 |
| AI0..AI2 (`%IW0..2`, 12-bit) | PA0, PA4, PA5 |
| AI3 (`%IW3`) + `%SD6` (0.1 °C) | On-chip temperature sensor |
| Status LED (RUN / STOP / ERROR) | PC13 (active-low) |
| **uPLC Studio link** USART1 115200 8N1 | **PA9 = TX, PA10 = RX** |
| RS-485 Modbus RTU USART2 9600 8N1 | PA2 = TX, PA3 = RX, PA1 = DE/RE |
| ST-Link V2 (SWD) | PA13 = SWDIO, PA14 = SWCLK |

To change the mapping, edit `configure_hardware_io!` / `configure_hardware_adc!` in [`src/config.rs`](src/config.rs).
The mapping is fixed when you compile, so `CMD 0x0B/0x0C` (read/write I/O config) get a NAK reply.

## Flash layout

| Address | Size | Content |
|---|---|---|
| `0x0800_0000` | 29 KB | Firmware |
| `0x0800_7400` | 1 KB (page 29) | RETAIN `M64..M95`, `D96..D127` (`PERS`) |
| `0x0800_7800` | 2 KB (pages 30–31) | PLC bytecode (`UPLC` header + CRC16) |

`probe-rs download` erases only the pages it writes, so re-flashing the firmware **keeps** the saved PLC program and RETAIN data.
To wipe everything, run `./flash.sh erase`.

## Build & upload (ST-Link V2)

Wiring: ST-Link `SWDIO → PA13`, `SWCLK → PA14`, `GND → GND`, `3.3V → 3.3V`. Leave BOOT0 = 0.

```bash
rustup target add thumbv7m-none-eabi
cargo install probe-rs-tools flip-link   # once

cd firmware/stm32f103c6-embassy
./flash.sh            # build + flash (probe-rs)
./flash.sh run        # flash + live defmt/RTT log   (same as: cargo run --release)
./flash.sh openocd    # alternative: OpenOCD
./flash.sh stflash    # alternative: st-flash (.bin)
./flash.sh bin        # produce dist/uplc-stm32f103c6.bin/.hex only
```

**Clone chips** (CKS32 / APM32 / GD32 / "C6T6" with a different IDCODE): try
`EXTRA="--connect-under-reset" ./flash.sh` and hold NRST if needed. If no 8 MHz crystal is fitted,
set `USE_HSE_8MHZ = false` in `config.rs` (uses HSI → 64 MHz).

## Connecting uPLC Studio

1. Connect a USB-TTL adapter (CH340 / CP2102 / FT232, **3.3 V logic**): `TX → PA10`, `RX → PA9`, `GND → GND`.
2. **Do not** wire DTR/RTS to NRST. uPLC Studio asserts DTR when it opens the port.
3. In uPLC Studio, select the adapter port (`/dev/ttyUSB0` / `COMx`) at **115200** baud.
4. Keep ladder programs **≤ 256 instructions**. Larger downloads are rejected with `[0xAA, 0xFF, 0x01]`.

## Differences from RP2040 firmware

- No USB-CDC: the IDE link is USART1, which saves about 8–10 KB of Flash.
- `CMD_ENTER_BOOTSEL (0x0A)` saves RETAIN data and does a **system reset** (STM32 has no BOOTSEL mode). Update firmware with ST-Link.
- No user UART: `UART_SEND` jobs finish right away and `UART_RECV` never receives data. Only two USARTs are available, and both are already in use.
- RETAIN auto-save runs at most every 10 min. A Flash page erase pauses the CPU for about 20–40 ms on STM32F1.
