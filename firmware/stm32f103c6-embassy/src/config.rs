//! # Hardware & System Configuration (`config.rs`) — STM32F103C6T6
//!
//! Centralized hardware mapping for the STM32F103C6T6 PLC Micro-VM firmware
//! (Cortex-M3 @ 72 MHz, 32 KB Flash, 10 KB SRAM, "Blue Pill"-style boards).
//!
//! Unlike the RP2040 firmware, the I/O pin mapping is **fixed at compile time**
//! (edit the `configure_hardware_io!` macro below). `CMD_GET_IO_CONFIG (0x0B)` /
//! `CMD_SET_IO_CONFIG (0x0C)` are answered with a NAK because the RP2040 `GPn`
//! numbering used by `HardwareIoConfig` does not apply to STM32 ports.
//!
//! Config PIN
//! X0 = PB0      Y0 = PB12
//! X1 = PB1      Y1 = PB13
//! X2 = PB10     Y2 = PB14
//! X3 = PB11     Y3 = PB15
//! X4 = PB5      Y4 = PA8
//! X5 = PB6      Y5 = PB8
//! X6 = PB7      Y6 = PB9
//! X7 = PA6      Y7 = PA7
//!
//! AI0 = PA0 (ADC1_IN0)   AI1 = PA4 (ADC1_IN4)   AI2 = PA5 (ADC1_IN5)   AI3 = On-chip temp sensor
//! Status LED   = PC13 (active-low, onboard LED of Blue Pill)
//! uPLC Studio  = USART1  PA9 (TX) / PA10 (RX)  115200 8N1  (USB-TTL adapter: CH340 / CP2102 / FT232)
//! RS-485       = USART2  PA2 (TX) / PA3  (RX)  + PA1 = DE/RE
//! ST-Link V2   = SWD     PA13 (SWDIO) / PA14 (SWCLK) / NRST (optional)
//!
//! Pins intentionally NOT used: PA11/PA12 (USB D-/D+ with 1.5k pull-up on Blue Pill),
//! PA13/PA14 (SWD - keep free so ST-Link always works), PA15/PB3/PB4 (JTAG after reset),
//! PB2 (BOOT1), PC14/PC15 (LSE crystal).

use embassy_stm32::adc::{Adc, AnyAdcChannel, Temperature};
use embassy_stm32::gpio::{Input, Output, Pull};
use embassy_stm32::peripherals::ADC1;

// =============================================================================
// 1. PLC Timing & Scan Cycle Parameters
// =============================================================================

/// PLC Scan Cycle Period in milliseconds (10ms = 100 Hz deterministic scan)
pub const SCAN_CYCLE_MS: u32 = 10;

/// Watchdog timeout limit in milliseconds. Scans exceeding this trip the SM_SCAN_OVERRUN flag.
pub const DEFAULT_WATCHDOG_TIMEOUT_MS: u32 = 100;

// =============================================================================
// 2. Clock Configuration
// =============================================================================

/// `true`  = 8 MHz external crystal (HSE) x9 -> 72 MHz SYSCLK (standard Blue Pill)
/// `false` = internal 8 MHz RC (HSI/2) x16 -> 64 MHz SYSCLK (boards without crystal)
pub const USE_HSE_8MHZ: bool = true;

// =============================================================================
// 3. Internal Flash Storage Layout (STM32F103C6: 32 x 1 KB pages)
//    Offsets are relative to FLASH_BASE (0x0800_0000). Keep in sync with memory.x
// =============================================================================

/// Base address of the memory-mapped internal Flash
pub const FLASH_BASE_ADDR: u32 = 0x0800_0000;
/// Flash page (erase) size on STM32F1 low/medium density devices
pub const FLASH_PAGE_SIZE: u32 = 1024;

/// RETAIN memory (`M64..M95`, `D96..D127`) — Page 29 (`0x0800_7400..0x0800_77FF`)
pub const FLASH_PERSISTENT_OFFSET: u32 = 0x7400;
pub const FLASH_PERSISTENT_MAGIC: u32 = 0x5045_5253; // "PERS" in ASCII

/// PLC Bytecode — Pages 30..31 (`0x0800_7800..0x0800_7FFF`, 12 B header + up to 256 x 4 B program)
/// `uplc-vm` is built with the `compact` feature (`MAX_INSTRUCTIONS = 256`) to fit 10 KB SRAM.
pub const FLASH_PLC_OFFSET: u32 = 0x7800;
pub const FLASH_PLC_PAGES: u32 = 2;
pub const FLASH_MAGIC: u32 = 0x5550_4C43; // "UPLC" in ASCII

// =============================================================================
// 4. Serial & Modbus RTU Communication Configuration
// =============================================================================

/// USART1 (PA9 TX / PA10 RX) baud rate for the uPLC Studio link (must match IDE setting, default 115200)
pub const COMM_BAUD_RATE: u32 = 115_200;

/// Inter-byte timeout: a partially received frame is discarded after this many ms of silence
pub const COMM_FRAME_TIMEOUT_MS: u64 = 200;

/// USART2 (PA2 TX / PA3 RX) default baud rate for Modbus RTU RS-485
pub const MODBUS_BAUD_RATE: u32 = 9_600;

/// Modbus RTU Response Timeout in milliseconds
pub const MODBUS_TIMEOUT_MS: u64 = 150;

// =============================================================================
// 5. PLC Status LED (PC13, active-low)
// =============================================================================

/// PLC System Operating Status for LED Indication
#[derive(Debug, Clone, Copy, PartialEq, Eq, defmt::Format)]
pub enum PlcStatus {
    Run,
    Stop,
    Error,
}

/// RUN pattern: `true` = 1 Hz heartbeat, `false` = solid ON
pub const STATUS_LED_RUN_HEARTBEAT: bool = true;

/// Blue Pill PC13 LED lights when the pin is driven LOW
pub const STATUS_LED_ACTIVE_LOW: bool = true;

// =============================================================================
// 6. Hardware GPIO Input & Output Pin Mapping
// =============================================================================

/// Pull resistor for digital inputs (`Pull::Up` + active-low = switch to GND turns input ON)
pub const INPUT_PULL_MODE: Pull = Pull::Up;

/// `true` = GND triggers logic 1 (sinking inputs / opto-couplers to GND)
pub const INPUT_ACTIVE_LOW: bool = true;

/// Bundled Hardware I/O resources passed to the PLC scan task
pub struct HardwareIo {
    pub inputs: [Input<'static>; 8],
    pub outputs: [Output<'static>; 8],
    pub status_led: Output<'static>,
}

/// Bundled Hardware ADC resources passed to the PLC scan task
pub struct HardwareAdc {
    pub adc: Adc<'static, ADC1>,
    /// `AI0..AI2` external analog channels (`%IW0..%IW2`)
    pub ai: [AnyAdcChannel<ADC1>; 3],
    /// `AI3` on-chip temperature sensor (`%IW3`, also converted to `%SD6`)
    pub temp: Temperature,
}

/// Instantiate digital I/O from STM32 peripherals `p`. Edit pins here to customize your board.
#[macro_export]
macro_rules! configure_hardware_io {
    ($p:expr) => {{
        use embassy_stm32::gpio::{Input, Level, Output, Speed};
        let pull = $crate::config::INPUT_PULL_MODE;
        $crate::config::HardwareIo {
            // Digital Inputs X0..X7 (%IX0.0 .. %IX0.7)
            inputs: [
                Input::new($p.PB0, pull),  // X0
                Input::new($p.PB1, pull),  // X1
                Input::new($p.PB10, pull), // X2
                Input::new($p.PB11, pull), // X3
                Input::new($p.PB5, pull),  // X4
                Input::new($p.PB6, pull),  // X5
                Input::new($p.PB7, pull),  // X6
                Input::new($p.PA6, pull),  // X7
            ],
            // Digital Outputs Y0..Y7 (%QX0.0 .. %QX0.7) — start LOW (safe state)
            outputs: [
                Output::new($p.PB12, Level::Low, Speed::Low), // Y0
                Output::new($p.PB13, Level::Low, Speed::Low), // Y1
                Output::new($p.PB14, Level::Low, Speed::Low), // Y2
                Output::new($p.PB15, Level::Low, Speed::Low), // Y3
                Output::new($p.PA8, Level::Low, Speed::Low),  // Y4
                Output::new($p.PB8, Level::Low, Speed::Low),  // Y5
                Output::new($p.PB9, Level::Low, Speed::Low),  // Y6
                Output::new($p.PA7, Level::Low, Speed::Low),  // Y7
            ],
            // Onboard Status LED (PC13, active-low -> start HIGH = OFF)
            status_led: Output::new($p.PC13, Level::High, Speed::Low),
        }
    }};
}

/// Instantiate ADC1 + analog channels (`AI0=PA0`, `AI1=PA4`, `AI2=PA5`, `AI3=Temp sensor`)
#[macro_export]
macro_rules! configure_hardware_adc {
    ($p:expr) => {{
        use embassy_stm32::adc::{Adc, AdcChannel, SampleTime};
        use embassy_stm32::interrupt::InterruptExt;
        let mut adc = Adc::new($p.ADC1);
        // embassy-stm32 0.2 F1 `Adc::new()` does NOT enable the ADC1_2 NVIC line, so the async
        // `read().await` would never be woken (EOC interrupt) and the scan task would hang forever.
        embassy_stm32::interrupt::ADC1_2.unpend();
        // SAFETY: the bound `adc::InterruptHandler<ADC1>` (see `bind_interrupts!` in main.rs) services it.
        unsafe { embassy_stm32::interrupt::ADC1_2.enable() };
        // 239.5 cycles @ 12 MHz ADCCLK ≈ 20 µs (temp sensor needs >= 17.1 µs)
        adc.set_sample_time(SampleTime::CYCLES239_5);
        let temp = adc.enable_temperature();
        $crate::config::HardwareAdc {
            adc,
            ai: [
                $p.PA0.degrade_adc(), // AI0 (%IW0)
                $p.PA4.degrade_adc(), // AI1 (%IW1)
                $p.PA5.degrade_adc(), // AI2 (%IW2)
            ],
            temp,
        }
    }};
}

// =============================================================================
// 7. Memory Partitioning & Latch Retention Area
//    RETAIN range: M64..M95 (bit_mem[0x80..0xA0]) & D96..D127 (word_mem[0x80..0xA0])
// =============================================================================

pub const LATCH_BIT_START_ADDR: u8 = 0x80;
pub const LATCH_WORD_START_ADDR: u8 = 0x80;

/// Minimum interval (seconds) between automatic Flash saves when RETAIN memory is dirty (10 minutes).
/// STM32F1 Flash endurance is 10k cycles per page -> keep this conservative.
pub const RETAIN_AUTOSAVE_MIN_INTERVAL_S: u64 = 600;

// =============================================================================
// 8. Special System Markers (%SM) — bit_mem[0xA0..0xBF] / M96..M127
// =============================================================================

pub const ENABLE_SPECIAL_MARKERS: bool = true;

pub const SM_ALWAYS_ON: u8 = 0xA0; // %SM0
pub const SM_ALWAYS_OFF: u8 = 0xA1; // %SM1
pub const SM_FIRST_SCAN: u8 = 0xA2; // %SM2
pub const SM_RETAIN_DIRTY: u8 = 0xA7; // %SM7
pub const SM_CLOCK_10MS: u8 = 0xAA; // %SM10
pub const SM_CLOCK_100MS: u8 = 0xAB; // %SM11
pub const SM_CLOCK_1S: u8 = 0xAC; // %SM12
pub const SM_CLOCK_1MIN: u8 = 0xAD; // %SM13
pub const SM_SYSTEM_ERROR: u8 = 0xB4; // %SM20
pub const SM_SCAN_OVERRUN: u8 = 0xB5; // %SM21

// =============================================================================
// 9. Special System Registers (%SD) — word_mem[0xA0..0xBF] / D128..D159
// =============================================================================

pub const ENABLE_SPECIAL_REGISTERS: bool = true;

pub const SD_WATCHDOG_PRESET: u8 = 0xA0; // %SD0
pub const SD_CURRENT_SCAN_TIME_US: u8 = 0xA1; // %SD1
pub const SD_MIN_SCAN_TIME_US: u8 = 0xA2; // %SD2
pub const SD_MAX_SCAN_TIME_US: u8 = 0xA3; // %SD3
pub const SD_SCAN_COUNT_LOW: u8 = 0xA4; // %SD4
pub const SD_SCAN_COUNT_HIGH: u8 = 0xA5; // %SD5
pub const SD_CPU_TEMP_C01: u8 = 0xA6; // %SD6 (0.1 °C)
pub const SD_RETAIN_SAVE_COUNT: u8 = 0xA7; // %SD7

// =============================================================================
// 10. Analog Inputs
// =============================================================================

pub const ENABLE_ANALOG_INPUTS: bool = true;
/// Reference voltage in millivolts (VDDA = 3.3V)
pub const ADC_VREF_MV: u32 = 3300;
