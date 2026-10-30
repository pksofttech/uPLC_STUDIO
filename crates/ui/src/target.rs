//! PLC hardware target profiles (`RP2040` USB-CDC vs `STM32F103C6T6` USART1).
//!
//! The target is derived from `LadderProgram::hardware` (Hardware menu), so it is saved with
//! the project file and in the app settings. Every target-specific limit, pin label, and
//! piece of UI text lives here so the rest of the UI can stay generic.

/// Hardware-menu name of the STM32F103C6T6 Micro-VM firmware (`firmware/stm32f103c6-embassy`)
pub const STM32F103C6_TARGET_NAME: &str = "STM32F103C6T6 Micro-VM (USART1 / ST-Link)";

/// Hardware-menu name of the RP2040 Micro-VM firmware (`firmware/rp2040-embassy`)
pub const RP2040_TARGET_NAME: &str = "RP2040 Micro-VM (Custom Firmware)";

/// Fixed (compile-time) STM32F103C6T6 digital input pins `X0..X7` (see firmware `src/config.rs`)
pub const STM32F103C6_INPUT_PINS: [&str; 8] = ["PB0", "PB1", "PB10", "PB11", "PB5", "PB6", "PB7", "PA6"];
/// Fixed (compile-time) STM32F103C6T6 digital output pins `Y0..Y7`
pub const STM32F103C6_OUTPUT_PINS: [&str; 8] = ["PB12", "PB13", "PB14", "PB15", "PA8", "PB8", "PB9", "PA7"];
/// STM32F103C6T6 analog inputs `AI0..AI3` (`%IW0..%IW3`)
pub const STM32F103C6_ANALOG_PINS: [(&str, &str); 4] = [
    ("%IW0", "PA0 (ADC1_IN0)"),
    ("%IW1", "PA4 (ADC1_IN4)"),
    ("%IW2", "PA5 (ADC1_IN5)"),
    ("%IW3", "On-chip Temp Sensor"),
];

/// Micro-VM firmware family selected by the project's hardware target
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlcTarget {
    /// RP2040 (Raspberry Pi Pico): USB-CDC link, runtime-configurable GPIO, 512 instructions
    Rp2040,
    /// STM32F103C6T6 (Blue Pill): USART1 PA9/PA10 link, fixed pin map, 256 instructions
    Stm32f103c6,
}

impl PlcTarget {
    /// Classify a `LadderProgram::hardware` string. Anything that is not the STM32F103C6
    /// Micro-VM (including the legacy "OpenPLC STM32" entry) falls back to RP2040 behaviour.
    pub fn from_hardware(hardware: &str) -> Self {
        let lower = hardware.to_ascii_lowercase();
        if lower.contains("stm32f103c6") || (lower.contains("stm32f103") && lower.contains("micro-vm")) {
            Self::Stm32f103c6
        } else {
            Self::Rp2040
        }
    }

    /// Short MCU name for labels
    pub const fn mcu_name(self) -> &'static str {
        match self {
            Self::Rp2040 => "RP2040",
            Self::Stm32f103c6 => "STM32F103C6T6",
        }
    }

    /// Program memory capacity of the firmware (`uplc-vm` `MAX_INSTRUCTIONS`, `compact` on STM32)
    pub const fn max_instructions(self) -> usize {
        match self {
            Self::Rp2040 => 512,
            Self::Stm32f103c6 => 256,
        }
    }

    /// Flash address where the firmware stores downloaded bytecode
    pub const fn program_flash_location(self) -> &'static str {
        match self {
            Self::Rp2040 => "Flash 0x1FE000",
            Self::Stm32f103c6 => "Flash 0x0800_7800",
        }
    }

    /// Physical link between uPLC Studio and the PLC
    pub const fn link_description(self) -> &'static str {
        match self {
            Self::Rp2040 => "Native USB-CDC (/dev/ttyACM* / COMx)",
            Self::Stm32f103c6 => "USART1 PA9=TX / PA10=RX via 3.3V USB-TTL (CH340 / CP2102 / FT232), 115200 8N1",
        }
    }

    /// Default serial baud rate expected by the firmware
    pub const fn default_baud(self) -> u32 {
        115_200
    }

    /// `true` if the firmware supports `CMD_GET/SET_IO_CONFIG (0x0B/0x0C)` runtime pin mapping
    pub const fn supports_runtime_io_config(self) -> bool {
        matches!(self, Self::Rp2040)
    }

    /// `true` if `CMD_ENTER_BOOTSEL (0x0A)` reboots into a USB mass-storage (UF2) bootloader
    pub const fn supports_usb_bootsel(self) -> bool {
        matches!(self, Self::Rp2040)
    }

    /// Hardware pin label for digital input `X{idx}` on fixed-map targets (`None` = runtime config)
    pub fn fixed_input_pin(self, idx: usize) -> Option<&'static str> {
        match self {
            Self::Rp2040 => None,
            Self::Stm32f103c6 => STM32F103C6_INPUT_PINS.get(idx).copied(),
        }
    }

    /// Hardware pin label for digital output `Y{idx}` on fixed-map targets (`None` = runtime config)
    pub fn fixed_output_pin(self, idx: usize) -> Option<&'static str> {
        match self {
            Self::Rp2040 => None,
            Self::Stm32f103c6 => STM32F103C6_OUTPUT_PINS.get(idx).copied(),
        }
    }

    /// Status message after `CMD_ENTER_BOOTSEL (0x0A)` was sent
    pub const fn bootsel_status(self) -> &'static str {
        match self {
            Self::Rp2040 => "RP2040 is mounting as RPI-RP2 drive",
            Self::Stm32f103c6 => "STM32 saved RETAIN & reset (update firmware with ST-Link V2)",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_targets() {
        assert_eq!(PlcTarget::from_hardware(STM32F103C6_TARGET_NAME), PlcTarget::Stm32f103c6);
        assert_eq!(PlcTarget::from_hardware("stm32f103c6t6"), PlcTarget::Stm32f103c6);
        assert_eq!(PlcTarget::from_hardware(RP2040_TARGET_NAME), PlcTarget::Rp2040);
        assert_eq!(PlcTarget::from_hardware("OpenPLC STM32 (IEC 61131-3)"), PlcTarget::Rp2040);
        assert_eq!(PlcTarget::from_hardware("Arduino NANO"), PlcTarget::Rp2040);
    }

    #[test]
    fn stm32_limits_and_pins() {
        let t = PlcTarget::Stm32f103c6;
        assert_eq!(t.max_instructions(), 256);
        assert!(!t.supports_runtime_io_config());
        assert_eq!(t.fixed_input_pin(0), Some("PB0"));
        assert_eq!(t.fixed_output_pin(7), Some("PA7"));
        assert_eq!(PlcTarget::Rp2040.fixed_input_pin(0), None);
    }
}
