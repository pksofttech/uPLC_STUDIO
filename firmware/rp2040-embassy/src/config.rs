//! # Hardware & System Configuration (`config.rs`)
//!
//! Centralized hardware mapping for RP2040 PLC Micro-VM firmware:
//! - Digital Inputs `X0..X7` (`%IX0.0..%IX0.7`)
//! - Digital Outputs `Y0..Y7` (`%QX0.0..%QX0.7`)
//! - Onboard Status / Demo LED (`GP25`)
//! - Scan Cycle Timing & Flash Memory Offsets
//!
//! Config PIN
//! X0 = GP10
//! X1 = GP11
//! X2 = GP12
//! X3 = GP13
//! X4 = -
//! X5 = -
//! X6 = -
//! X7 = -
//! Y0 = GP15
//! Y1 = GP14
//! Y2 = GP6
//! Y3 = GP7
//! Y4 = GP2
//! Y5 = GP3
//! Y6 = -
//! Y7 = -

use embassy_rp::gpio::{AnyPin, Input, Level, Output, Pull};

// =============================================================================
// 1. PLC Timing & Scan Cycle Parameters
// =============================================================================

/// PLC Scan Cycle Period in milliseconds (10ms = 100 Hz deterministic scan)
pub const SCAN_CYCLE_MS: u32 = 10;

// =============================================================================
// 2. Flash Memory Storage Layout (RP2040 2MB SPI Flash)
// =============================================================================

/// Flash offset reserved for saving PLC Bytecode (last 8KB of 2MB RP2040 Flash)
pub const FLASH_PLC_OFFSET: u32 = 0x1FE000;
pub const FLASH_SIZE_BYTES: usize = 2 * 1024 * 1024;
pub const FLASH_MAGIC: u32 = 0x5550_4C43; // "UPLC" in ASCII

// =============================================================================
// 3. Serial & Modbus RTU Communication Configuration
// =============================================================================

/// UART0 baud rate for programming & telemetry (GP0 = TX, GP1 = RX) [Optional/Unused]
#[allow(dead_code)]
pub const UART_BAUD_RATE: u32 = 115_200;

/// UART1 baud rate for Modbus RTU RS-485 (GP8 = TX, GP9 = RX, DE/RE = None)
pub const MODBUS_BAUD_RATE: u32 = 9_600;

/// Modbus RTU Response Timeout in milliseconds
pub const MODBUS_TIMEOUT_MS: u64 = 150;

// =============================================================================
// 4. PLC Status & Indicator LED Configuration (GP25)
// =============================================================================

/// PLC System Operating Status for LED Indication:
/// - `Run`: PLC program is active and executing scan cycles
/// - `Stop`: PLC is in STOP mode or no program loaded (Idle)
/// - `Error`: Hardware/Flash/Protocol fault or runtime exception
#[derive(Debug, Clone, Copy, PartialEq, Eq, defmt::Format)]
pub enum PlcStatus {
    Run,
    Stop,
    Error,
}

/// Status LED (GP25) Pattern in RUN mode:
/// - `true`: Heartbeat 1 Hz (500ms ON / 500ms OFF) - Indicates active CPU logic execution
/// - `false`: Solid ON - Continuous light (traditional PLC RUN LED)
pub const STATUS_LED_RUN_HEARTBEAT: bool = true;

// =============================================================================
// 5. Hardware GPIO Input & Output Pin Mapping
// =============================================================================

/// Default pull resistor mode for digital inputs
/// - `Pull::Down`: Active-High (logic 1 when connected to 3.3V)
/// - `Pull::Up`: Active-Low (logic 0 when connected to GND)
pub const INPUT_PULL_MODE: Pull = Pull::Up;

/// Whether digital inputs are Active-Low (True = GND triggers logic 1, False = 3.3V triggers logic 1)
pub const INPUT_ACTIVE_LOW: bool = true;

/// Bundled Hardware I/O resources passed to the PLC scan task
pub struct HardwareIo {
    pub inputs: [Option<Input<'static>>; 8],
    pub outputs: [Option<Output<'static>>; 8],
    pub status_led: Output<'static>,
}

/// Macro to instantiate and configure hardware inputs and outputs directly from RP2040 Peripherals `p`.
///
/// Edit the pin numbers below to customize pin mapping for your hardware board:
#[macro_export]
macro_rules! configure_hardware_io {
    ($p:expr) => {
        $crate::config::HardwareIo {
            // -----------------------------------------------------------------
            // Hardware Digital Inputs X0..X7 (%IX0.0 .. %IX0.7)
            // -----------------------------------------------------------------
            inputs: [
                Some(embassy_rp::gpio::Input::new(
                    $p.PIN_10,
                    $crate::config::INPUT_PULL_MODE,
                )), // X0: GP10
                Some(embassy_rp::gpio::Input::new(
                    $p.PIN_11,
                    $crate::config::INPUT_PULL_MODE,
                )), // X1: GP11
                Some(embassy_rp::gpio::Input::new(
                    $p.PIN_12,
                    $crate::config::INPUT_PULL_MODE,
                )), // X2: GP12
                Some(embassy_rp::gpio::Input::new(
                    $p.PIN_13,
                    $crate::config::INPUT_PULL_MODE,
                )), // X3: GP13
                None, // X4: -
                None, // X5: -
                None, // X6: -
                None, // X7: -
            ],

            // -----------------------------------------------------------------
            // Hardware Digital Outputs Y0..Y7 (%QX0.0 .. %QX0.7)
            // -----------------------------------------------------------------
            outputs: [
                Some(embassy_rp::gpio::Output::new(
                    $p.PIN_15,
                    embassy_rp::gpio::Level::Low,
                )), // Y0: GP15
                Some(embassy_rp::gpio::Output::new(
                    $p.PIN_14,
                    embassy_rp::gpio::Level::Low,
                )), // Y1: GP14
                Some(embassy_rp::gpio::Output::new(
                    $p.PIN_6,
                    embassy_rp::gpio::Level::Low,
                )), // Y2: GP6
                Some(embassy_rp::gpio::Output::new(
                    $p.PIN_7,
                    embassy_rp::gpio::Level::Low,
                )), // Y3: GP7
                Some(embassy_rp::gpio::Output::new(
                    $p.PIN_2,
                    embassy_rp::gpio::Level::Low,
                )), // Y4: GP2
                Some(embassy_rp::gpio::Output::new(
                    $p.PIN_3,
                    embassy_rp::gpio::Level::Low,
                )), // Y5: GP3
                None, // Y6: -
                None, // Y7: -
            ],

            // -----------------------------------------------------------------
            // Onboard PLC Status Indicator LED Pin (Raspberry Pi Pico GP25)
            // -----------------------------------------------------------------
            status_led: embassy_rp::gpio::Output::new($p.PIN_25, embassy_rp::gpio::Level::Low),
        }
    };
}

/// Dynamic function-based configuration using erased `AnyPin` tokens
#[allow(dead_code)]
pub fn configure_hardware_io_degraded(
    in_pins: [Option<AnyPin>; 8],
    out_pins: [Option<AnyPin>; 8],
    led_pin: AnyPin,
) -> HardwareIo {
    HardwareIo {
        inputs: in_pins.map(|p| p.map(|pin| Input::new(pin, INPUT_PULL_MODE))),
        outputs: out_pins.map(|p| p.map(|pin| Output::new(pin, Level::Low))),
        status_led: Output::new(led_pin, Level::Low),
    }
}
