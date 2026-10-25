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
//! Y2 = GP7
//! Y3 = GP6
//! Y4 = GP2
//! Y5 = GP3
//! Y6 = -
//! Y7 = -

use embassy_rp::adc::{Adc, Blocking, Channel};
use embassy_rp::gpio::{AnyPin, Input, Level, Output, Pull};

// =============================================================================
// 1. PLC Timing & Scan Cycle Parameters
// =============================================================================

/// PLC Scan Cycle Period in milliseconds (10ms = 100 Hz deterministic scan)
pub const SCAN_CYCLE_MS: u32 = 10;

// =============================================================================
// 2. Flash Memory Storage Layout (RP2040 2MB SPI Flash)
// =============================================================================

/// Flash offset reserved for saving PLC Bytecode (Sector 510, 0x1FE000..0x1FEFFF)
pub const FLASH_PLC_OFFSET: u32 = 0x1FE000;
pub const FLASH_SIZE_BYTES: usize = 2 * 1024 * 1024;
pub const FLASH_MAGIC: u32 = 0x5550_4C43; // "UPLC" in ASCII

/// Flash offset reserved for saving IEC 61131-3 PERSISTENT / Retentive Variables (Sector 511, 0x1FF000..0x1FFFFF)
pub const FLASH_PERSISTENT_OFFSET: u32 = 0x1FF000;
pub const FLASH_PERSISTENT_MAGIC: u32 = 0x5045_5253; // "PERS" in ASCII

/// Flash offset reserved for saving Runtime Hardware I/O Pin Mapping (Sector 509, 0x1FD000..0x1FDFFF)
pub const FLASH_IO_CONFIG_OFFSET: u32 = 0x1FD000;
pub const FLASH_IO_CONFIG_MAGIC: u32 = 0x494F_4346; // "IOCF" in ASCII

// =============================================================================
// 3. Serial & Modbus RTU Communication Configuration
// =============================================================================

/// User UART0 baud rate for `UART_SEND` / `UART_RECV` function blocks (GP0 = TX, GP1 = RX, 115200 8N1)
pub const USER_UART_BAUD: u32 = 115_200;

/// Alias for UART0 baud rate (GP0 = TX, GP1 = RX)
#[allow(dead_code)]
pub const UART_BAUD_RATE: u32 = USER_UART_BAUD;

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
#[allow(dead_code)]
pub const STATUS_LED_RUN_HEARTBEAT: bool = true;

// =============================================================================
// 5. Hardware GPIO Input & Output Pin Mapping
// =============================================================================

/// Default pull resistor mode for digital inputs
/// - `Pull::Down`: Active-High (logic 1 when connected to 3.3V)
/// - `Pull::Up`: Active-Low (logic 0 when connected to GND)
#[allow(dead_code)]
pub const INPUT_PULL_MODE: Pull = Pull::Up;

/// Whether digital inputs are Active-Low (True = GND triggers logic 1, False = 3.3V triggers logic 1)
#[allow(dead_code)]
pub const INPUT_ACTIVE_LOW: bool = true;

/// Bundled Hardware I/O resources passed to the PLC scan task
#[allow(dead_code)]
pub struct HardwareIo {
    pub inputs: [Option<Input<'static>>; 8],
    pub outputs: [Option<Output<'static>>; 8],
    pub status_led: Output<'static>,
}

/// Bundled Hardware ADC resources passed to the PLC scan task
pub struct HardwareAdc {
    pub adc: Adc<'static, Blocking>,
    pub ch_ai0: Channel<'static>,
    pub ch_ai1: Channel<'static>,
    pub ch_ai2: Channel<'static>,
    pub ch_ai3: Channel<'static>,
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
                    $p.PIN_7,
                    embassy_rp::gpio::Level::Low,
                )), // Y3: GP7
                Some(embassy_rp::gpio::Output::new(
                    $p.PIN_6,
                    embassy_rp::gpio::Level::Low,
                )), // Y2: GP6
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

/// Macro to instantiate and configure hardware ADC and analog channels directly from RP2040 Peripherals `p`.
///
/// - AI0 (%IW0): GP26 (Pin 31 / ADC0)
/// - AI1 (%IW1): GP27 (Pin 32 / ADC1)
/// - AI2 (%IW2): GP28 (Pin 34 / ADC2)
/// - AI3 (%IW3): Internal RP2040 Temperature Sensor (ADC_TEMP_SENSOR)
#[macro_export]
macro_rules! configure_hardware_adc {
    ($p:expr) => {
        $crate::config::HardwareAdc {
            adc: embassy_rp::adc::Adc::new_blocking($p.ADC, embassy_rp::adc::Config::default()),
            ch_ai0: embassy_rp::adc::Channel::new_pin($p.PIN_26, embassy_rp::gpio::Pull::None),
            ch_ai1: embassy_rp::adc::Channel::new_pin($p.PIN_27, embassy_rp::gpio::Pull::None),
            ch_ai2: embassy_rp::adc::Channel::new_pin($p.PIN_28, embassy_rp::gpio::Pull::None),
            ch_ai3: embassy_rp::adc::Channel::new_temp_sensor($p.ADC_TEMP_SENSOR),
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

/// Apply `HardwareIoConfig` directly to RP2040 `SIO`, `IO_BANK0`, and `PADS_BANK0` registers at runtime
/// without requiring firmware recompilation.
pub fn apply_runtime_io_config(
    prev: Option<&uplc_vm::HardwareIoConfig>,
    cfg: &uplc_vm::HardwareIoConfig,
) {
    // 1. Safely release any previously configured output pins to Hi-Z input with pull-down
    if let Some(old) = prev {
        for &pin in old.output_pins.iter().chain(core::iter::once(&old.status_led_pin)) {
            if pin < 30 && !uplc_vm::HardwareIoConfig::is_reserved_peripheral_pin(pin) {
                embassy_rp::pac::SIO.gpio_out(0).value_clr().write_value(1u32 << pin);
                embassy_rp::pac::SIO.gpio_oe(0).value_clr().write_value(1u32 << pin);
                embassy_rp::pac::PADS_BANK0.gpio(pin as usize).modify(|w| {
                    w.set_ie(true);
                    w.set_pue(false);
                    w.set_pde(true);
                });
            }
        }
    }

    let pull_up = cfg.input_pull_up();

    // 2. Configure Digital Input pins X0..X7
    for &pin in &cfg.input_pins {
        if pin < 30 && !uplc_vm::HardwareIoConfig::is_reserved_peripheral_pin(pin) {
            embassy_rp::pac::SIO.gpio_oe(0).value_clr().write_value(1u32 << pin);
            embassy_rp::pac::PADS_BANK0.gpio(pin as usize).modify(|w| {
                w.set_od(false);
                w.set_ie(true);
                w.set_pue(pull_up);
                w.set_pde(!pull_up);
            });
            embassy_rp::pac::IO_BANK0
                .gpio(pin as usize)
                .ctrl()
                .write(|w| w.set_funcsel(5)); // 5 = SIO function on RP2040
        }
    }

    // 3. Configure Digital Output pins Y0..Y7
    for &pin in &cfg.output_pins {
        if pin < 30 && !uplc_vm::HardwareIoConfig::is_reserved_peripheral_pin(pin) {
            embassy_rp::pac::SIO.gpio_out(0).value_clr().write_value(1u32 << pin);
            embassy_rp::pac::SIO.gpio_oe(0).value_set().write_value(1u32 << pin);
            embassy_rp::pac::PADS_BANK0.gpio(pin as usize).modify(|w| {
                w.set_od(false);
                w.set_ie(true);
                w.set_pue(false);
                w.set_pde(false);
            });
            embassy_rp::pac::IO_BANK0
                .gpio(pin as usize)
                .ctrl()
                .write(|w| w.set_funcsel(5));
        }
    }

    // 4. Configure Status LED pin
    let led_pin = cfg.status_led_pin;
    if led_pin < 30 && !uplc_vm::HardwareIoConfig::is_reserved_peripheral_pin(led_pin) {
        embassy_rp::pac::SIO.gpio_out(0).value_clr().write_value(1u32 << led_pin);
        embassy_rp::pac::SIO.gpio_oe(0).value_set().write_value(1u32 << led_pin);
        embassy_rp::pac::PADS_BANK0.gpio(led_pin as usize).modify(|w| {
            w.set_od(false);
            w.set_ie(true);
            w.set_pue(false);
            w.set_pde(false);
        });
        embassy_rp::pac::IO_BANK0
            .gpio(led_pin as usize)
            .ctrl()
            .write(|w| w.set_funcsel(5));
    }
}

/// Drive a dynamically mapped RP2040 GPIO output pin HIGH or LOW via `SIO`.
#[inline]
pub fn write_runtime_gpio_pin(pin: u8, high: bool) {
    if pin < 30 && !uplc_vm::HardwareIoConfig::is_reserved_peripheral_pin(pin) {
        if high {
            embassy_rp::pac::SIO.gpio_out(0).value_set().write_value(1u32 << pin);
        } else {
            embassy_rp::pac::SIO.gpio_out(0).value_clr().write_value(1u32 << pin);
        }
    }
}

// =============================================================================
// 6. Memory Partitioning & Latch Retention Area Configuration
//    RETAIN range: M64..M95 (bit_mem[0x80..0xA0]) & D96..D127 (%MW96..%MW127, word_mem[0x80..0xA0])
// =============================================================================

/// Start of retentive bit relays `M64..M95` (`%MX8.0..%MX11.7`) in `bit_mem`
#[allow(dead_code)]
pub const LATCH_BIT_START_ADDR: u8 = 0x80;
#[allow(dead_code)]
pub const LATCH_BIT_COUNT: u8 = 32;

/// Start of retentive data registers `D96..D127` (`%MW96..%MW127`) in `word_mem`
#[allow(dead_code)]
pub const LATCH_WORD_START_ADDR: u8 = 0x80;
#[allow(dead_code)]
pub const LATCH_WORD_COUNT: u8 = 32;

/// Minimum interval (in seconds) between automatic Flash saves when RETAIN memory is dirty (10 minutes)
pub const RETAIN_AUTOSAVE_MIN_INTERVAL_S: u64 = 600;

// =============================================================================
// 7. Special System Markers Configuration (%SM / M8000 Series)
// =============================================================================

/// Enable/disable automatic update of Special System Markers in Firmware scan loop
pub const ENABLE_SPECIAL_MARKERS: bool = true;

/// Special Bit Addresses (Mapped in bit_mem[0xA0..0xBF] / M96..M127)
pub const SM_ALWAYS_ON: u8 = 0xA0; // %SM0  (M96 / M8000) : Always ON (True) while PLC is in RUN
pub const SM_ALWAYS_OFF: u8 = 0xA1; // %SM1  (M97 / M8001) : Always OFF (False)
pub const SM_FIRST_SCAN: u8 = 0xA2; // %SM2  (M98 / M8002) : ON for first scan only after RUN/Boot
pub const SM_RETAIN_DIRTY: u8 = 0xA7; // %SM7  (M103 / M8007): RETAIN memory dirty flag (1 when M64..M95 or D96..D127 differs from Flash)
pub const SM_CLOCK_10MS: u8 = 0xAA; // %SM10 (M106 / M8011): 10ms clock pulse (5ms ON / 5ms OFF, 50 Hz)
pub const SM_CLOCK_100MS: u8 = 0xAB; // %SM11 (M107 / M8012): 100ms clock pulse (50ms ON / 50ms OFF, 5 Hz)
pub const SM_CLOCK_1S: u8 = 0xAC; // %SM12 (M108 / M8013): 1.0s clock pulse (500ms ON / 500ms OFF, 1 Hz)
pub const SM_CLOCK_1MIN: u8 = 0xAD; // %SM13 (M109 / M8014): 1-minute pulse
pub const SM_SYSTEM_ERROR: u8 = 0xB4; // %SM20 (M116 / M8004): General PLC / Hardware Error flag
pub const SM_SCAN_OVERRUN: u8 = 0xB5; // %SM21 (M117 / M8020): Scan Time Overrun flag (> Watchdog preset)

// =============================================================================
// 8. Special System Registers Configuration (%SD / D8000 Series)
// =============================================================================

/// Enable/disable automatic measurement and recording of diagnostic registers
pub const ENABLE_SPECIAL_REGISTERS: bool = true;

/// Special Word Addresses (Mapped in word_mem[0xA0..0xBF] / D128..D159)
pub const SD_WATCHDOG_PRESET: u8 = 0xA0; // %SD0  (D128 / D8000): Watchdog Preset in ms (Default 100ms)
pub const SD_CURRENT_SCAN_TIME_US: u8 = 0xA1; // %SD1  (D129 / D8010): Current scan execution time (microseconds)
pub const SD_MIN_SCAN_TIME_US: u8 = 0xA2; // %SD2  (D130 / D8011): Minimum scan execution time recorded (μs)
pub const SD_MAX_SCAN_TIME_US: u8 = 0xA3; // %SD3  (D131 / D8012): Maximum scan execution time recorded (μs)
pub const SD_SCAN_COUNT_LOW: u8 = 0xA4; // %SD4  (D132): Scan counter lower 32-bit word
pub const SD_SCAN_COUNT_HIGH: u8 = 0xA5; // %SD5  (D133): Scan counter upper 32-bit word
pub const SD_CPU_TEMP_C01: u8 = 0xA6; // %SD6  (D134 / D8006): On-chip CPU Temperature in 0.1 °C units (e.g. 275 = 27.5 °C)
pub const SD_RETAIN_SAVE_COUNT: u8 = 0xA7; // %SD7  (D135 / D8007): Total RETAIN Flash save count

/// Watchdog timeout limit in milliseconds. Scans exceeding this trip the SM_SCAN_OVERRUN flag.
pub const DEFAULT_WATCHDOG_TIMEOUT_MS: u32 = 100;

// =============================================================================
// 9. Hardware Analog Inputs Configuration (ADC0..ADC2 & On-Chip Temp Sensor)
// =============================================================================

/// Enable/disable automatic sampling of Analog Inputs in Firmware scan loop
pub const ENABLE_ANALOG_INPUTS: bool = true;

/// Analog input raw 12-bit maximum (0..4095)
pub const ADC_MAX_RAW: u16 = 4095;
/// Reference voltage in millivolts (3300 mV = 3.3V)
pub const ADC_VREF_MV: u32 = 3300;
