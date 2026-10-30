#![no_std]

//! # uPLC Micro-VM (`uplc-vm`)
//!
//! Pure `#![no_std]` Accumulator-based Virtual Machine for executing 32-bit fixed-width
//! IEC 61131-3 Instruction List (IL) bytecode on RP2040 (Embassy) and other microcontrollers.
//!
//! ## Unified Memory Space
//! - `bit_mem[256]`:
//!   - `0x00..=0x1F`: Digital Inputs `X0..X31` (`%IX0.0..%IX3.7`)
//!   - `0x20..=0x3F`: Digital Outputs `Y0..Y31` (`%QX0.0..%QX3.7`)
//!   - `0x40..=0xBF`: Internal Relays `M0..M127` (`%MX0.0..%MX15.7`)
//!   - `0xC0..=0xDF`: Timer Done Bits `T0..T31`
//!   - `0xE0..=0xFF`: Counter Done Bits `C0..C31`
//! - `word_mem[256]`:
//!   - `0x00..=0x0F`: Analog Inputs `AI0..AI15` (`%IW0..%IW15`)
//!   - `0x10..=0x1F`: Analog Outputs `AO0..AO15` (`%QW0..%QW15`)
//!   - `0x20..=0xBF`: Data Registers `D0..D159` (`%MW0..%MW159`)
//!   - `0xC0..=0xDF`: Timer Current Values `T0..T31` (ms)
//!   - `0xE0..=0xFF`: Counter Current Values `C0..C31`

/// Maximum number of 32-bit instructions in program memory
/// (`512` by default, `256` with the `compact` feature for 10 KB SRAM parts like STM32F103C6T6).
#[cfg(not(feature = "compact"))]
pub const MAX_INSTRUCTIONS: usize = 512;
#[cfg(feature = "compact")]
pub const MAX_INSTRUCTIONS: usize = 256;
pub const MAX_STACK_DEPTH: usize = 16;
pub const PACKET_SOF: u8 = 0xAA;

pub mod modbus;
pub use modbus::*;
pub mod fb;
pub use fb::*;

pub const CMD_DOWNLOAD_PROGRAM: u8 = 0x01;
pub const CMD_RUN_STOP: u8 = 0x02;
pub const CMD_MONITOR_STATUS: u8 = 0x03;
pub const CMD_STATUS_RESPONSE: u8 = 0x83;
pub const CMD_UPLOAD_PROGRAM: u8 = 0x04;
pub const CMD_UPLOAD_PROGRAM_RESPONSE: u8 = 0x84;
pub const CMD_STEP: u8 = 0x05;
pub const CMD_FORCE_IO: u8 = 0x06;
pub const CMD_FORCE_IO_RESPONSE: u8 = 0x86;
pub const CMD_WRITE_VALUE: u8 = 0x07;
pub const CMD_WRITE_VALUE_RESPONSE: u8 = 0x87;
pub const CMD_PERSIST_SAVE: u8 = 0x08;
pub const CMD_PERSIST_SAVE_RESPONSE: u8 = 0x88;
pub const CMD_PERSIST_CLEAR: u8 = 0x09;
pub const CMD_PERSIST_CLEAR_RESPONSE: u8 = 0x89;
pub const CMD_ENTER_BOOTSEL: u8 = 0x0A;
pub const CMD_ENTER_BOOTSEL_RESPONSE: u8 = 0x8A;
pub const CMD_GET_IO_CONFIG: u8 = 0x0B;
pub const CMD_GET_IO_CONFIG_RESPONSE: u8 = 0x8B;
pub const CMD_SET_IO_CONFIG: u8 = 0x0C;
pub const CMD_SET_IO_CONFIG_RESPONSE: u8 = 0x8C;

pub const PIN_UNASSIGNED: u8 = 0xFF;
pub const LEGACY_IO_CONFIG_PAYLOAD_LEN: usize = 18;
pub const IO_CONFIG_PAYLOAD_LEN: usize = 20;
pub const IO_CONFIG_PACKET_LEN: usize = 5 + IO_CONFIG_PAYLOAD_LEN; // 25 bytes

pub const IO_FLAG_INPUT_PULL_UP: u8 = 0x01;
pub const IO_FLAG_INPUT_ACTIVE_LOW: u8 = 0x02;
pub const IO_FLAG_STATUS_LED_HEARTBEAT: u8 = 0x04;

/// Bit 4 of `rs485_config`: `0` = UART1 on `GP8(TX)/GP9(RX)`, `1` = UART1 on `GP4(TX)/GP5(RX)`
pub const RS485_FLAG_ALT_PINS_GP4_GP5: u8 = 0x10;

/// Runtime-configurable Hardware GPIO & RS-485 Pin Mapping for RP2040 Micro-VM (`20 bytes` payload)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct HardwareIoConfig {
    /// GPIO pin number (`0..=29`) for Digital Inputs `X0..X7` (`%IX0.0..%IX0.7`), or `0xFF` (`PIN_UNASSIGNED`)
    pub input_pins: [u8; 8],
    /// GPIO pin number (`0..=29`) for Digital Outputs `Y0..Y7` (`%QX0.0..%QX0.7`), or `0xFF` (`PIN_UNASSIGNED`)
    pub output_pins: [u8; 8],
    /// GPIO pin number (`0..=29`) for PLC Status LED, or `0xFF` (`PIN_UNASSIGNED`)
    pub status_led_pin: u8,
    /// Bitflags:
    /// - Bit 0 (`0x01`): `input_pull_up` (`1` = Pull-Up, `0` = Pull-Down)
    /// - Bit 1 (`0x02`): `input_active_low` (`1` = Active-Low / GND triggers ON, `0` = Active-High / 3.3V triggers ON)
    /// - Bit 2 (`0x04`): `status_led_heartbeat` (`1` = 1 Hz Heartbeat in RUN, `0` = Solid ON in RUN)
    pub flags: u8,
    /// GPIO pin number (`0..=29`) for RS-485 `DE/RE` direction control, or `0xFF` (`PIN_UNASSIGNED` = Auto-direction)
    pub rs485_de_re_pin: u8,
    /// RS-485 UART1 configuration byte:
    /// - Bits `0..=3`: Baud rate code (`0`=9600, `1`=19200, `2`=38400, `3`=57600, `4`=115200)
    /// - Bit `4` (`0x10`): TX/RX pin pair (`0` = `GP8(TX)/GP9(RX)`, `1` = `GP4(TX)/GP5(RX)`)
    pub rs485_config: u8,
}

impl Default for HardwareIoConfig {
    fn default() -> Self {
        Self::default_rp2040()
    }
}

impl HardwareIoConfig {
    /// Factory default RP2040 GPIO mapping matching `firmware/rp2040-embassy/src/config.rs`
    pub const fn default_rp2040() -> Self {
        Self {
            input_pins: [
                10,             // X0: GP10
                11,             // X1: GP11
                12,             // X2: GP12
                13,             // X3: GP13
                PIN_UNASSIGNED, // X4: -
                PIN_UNASSIGNED, // X5: -
                PIN_UNASSIGNED, // X6: -
                PIN_UNASSIGNED, // X7: -
            ],
            output_pins: [
                15,             // Y0: GP15
                14,             // Y1: GP14
                7,              // Y2: GP7
                6,              // Y3: GP6
                2,              // Y4: GP2
                3,              // Y5: GP3
                PIN_UNASSIGNED, // Y6: -
                PIN_UNASSIGNED, // Y7: -
            ],
            status_led_pin: 25, // GP25
            flags: IO_FLAG_INPUT_PULL_UP
                | IO_FLAG_INPUT_ACTIVE_LOW
                | IO_FLAG_STATUS_LED_HEARTBEAT,
            rs485_de_re_pin: PIN_UNASSIGNED, // Auto-direction (None)
            rs485_config: 0,                 // 9600 bps on GP8(TX)/GP9(RX)
        }
    }

    #[inline]
    pub const fn input_pull_up(&self) -> bool {
        (self.flags & IO_FLAG_INPUT_PULL_UP) != 0
    }

    #[inline]
    pub fn set_input_pull_up(&mut self, pull_up: bool) {
        if pull_up {
            self.flags |= IO_FLAG_INPUT_PULL_UP;
        } else {
            self.flags &= !IO_FLAG_INPUT_PULL_UP;
        }
    }

    #[inline]
    pub const fn input_active_low(&self) -> bool {
        (self.flags & IO_FLAG_INPUT_ACTIVE_LOW) != 0
    }

    #[inline]
    pub fn set_input_active_low(&mut self, active_low: bool) {
        if active_low {
            self.flags |= IO_FLAG_INPUT_ACTIVE_LOW;
        } else {
            self.flags &= !IO_FLAG_INPUT_ACTIVE_LOW;
        }
    }

    #[inline]
    pub const fn status_led_heartbeat(&self) -> bool {
        (self.flags & IO_FLAG_STATUS_LED_HEARTBEAT) != 0
    }

    #[inline]
    pub fn set_status_led_heartbeat(&mut self, heartbeat: bool) {
        if heartbeat {
            self.flags |= IO_FLAG_STATUS_LED_HEARTBEAT;
        } else {
            self.flags &= !IO_FLAG_STATUS_LED_HEARTBEAT;
        }
    }

    /// `true` if RS-485 UART1 uses `GP4(TX)/GP5(RX)` instead of default `GP8(TX)/GP9(RX)`
    #[inline]
    pub const fn rs485_use_alt_pins_gp4_gp5(&self) -> bool {
        (self.rs485_config & RS485_FLAG_ALT_PINS_GP4_GP5) != 0
    }

    #[inline]
    pub fn set_rs485_use_alt_pins_gp4_gp5(&mut self, alt_gp4_gp5: bool) {
        if alt_gp4_gp5 {
            self.rs485_config |= RS485_FLAG_ALT_PINS_GP4_GP5;
        } else {
            self.rs485_config &= !RS485_FLAG_ALT_PINS_GP4_GP5;
        }
    }

    /// Return RS-485 UART1 `(tx_pin, rx_pin)` pair (`(8, 9)` or `(4, 5)`)
    #[inline]
    pub const fn rs485_tx_rx_pins(&self) -> (u8, u8) {
        if self.rs485_use_alt_pins_gp4_gp5() {
            (4, 5)
        } else {
            (8, 9)
        }
    }

    /// Baud rate code `0..=4` (`0`=9600, `1`=19200, `2`=38400, `3`=57600, `4`=115200)
    #[inline]
    pub const fn rs485_baud_code(&self) -> u8 {
        self.rs485_config & 0x0F
    }

    #[inline]
    pub fn set_rs485_baud_code(&mut self, code: u8) {
        self.rs485_config = (self.rs485_config & !0x0F) | (code.min(4) & 0x0F);
    }

    /// Return RS-485 Modbus RTU baud rate in bps (`9600`, `19200`, `38400`, `57600`, or `115200`)
    #[inline]
    pub const fn rs485_baud_rate(&self) -> u32 {
        match self.rs485_baud_code() {
            1 => 19_200,
            2 => 38_400,
            3 => 57_600,
            4 => 115_200,
            _ => 9_600,
        }
    }

    #[inline]
    pub fn set_rs485_baud_rate(&mut self, baud: u32) {
        let code = match baud {
            19_200 => 1,
            38_400 => 2,
            57_600 => 3,
            115_200 => 4,
            _ => 0,
        };
        self.set_rs485_baud_code(code);
    }

    /// Returns `true` if `pin` is dedicated to UART0 (`GP0..1`), default UART1 (`GP8..9`), or ADC (`GP26..28`).
    #[inline]
    pub const fn is_reserved_peripheral_pin(pin: u8) -> bool {
        matches!(pin, 0 | 1 | 8 | 9 | 26 | 27 | 28)
    }

    /// Returns `true` if `pin` is reserved by fixed peripherals or the currently active RS-485 UART1 TX/RX pair.
    #[inline]
    pub const fn is_pin_reserved_for_config(&self, pin: u8) -> bool {
        let (tx, rx) = self.rs485_tx_rx_pins();
        matches!(pin, 0 | 1 | 26 | 27 | 28) || pin == tx || pin == rx
    }

    /// Sanitize pin assignments so out-of-range, reserved, or duplicate GPIO pins are safely cleared to `PIN_UNASSIGNED`.
    pub fn sanitize(&mut self) {
        let (mb_tx, mb_rx) = self.rs485_tx_rx_pins();
        let mut used_mask: u32 = (1u32 << 0)
            | (1u32 << 1)
            | (1u32 << 26)
            | (1u32 << 27)
            | (1u32 << 28)
            | (1u32 << mb_tx)
            | (1u32 << mb_rx);

        if self.rs485_de_re_pin >= 30 || (used_mask & (1u32 << self.rs485_de_re_pin)) != 0 {
            self.rs485_de_re_pin = PIN_UNASSIGNED;
        } else {
            used_mask |= 1u32 << self.rs485_de_re_pin;
        }

        for p in &mut self.input_pins {
            if *p >= 30 || (used_mask & (1u32 << *p)) != 0 {
                *p = PIN_UNASSIGNED;
            } else {
                used_mask |= 1u32 << *p;
            }
        }
        for p in &mut self.output_pins {
            if *p >= 30 || (used_mask & (1u32 << *p)) != 0 {
                *p = PIN_UNASSIGNED;
            } else {
                used_mask |= 1u32 << *p;
            }
        }
        if self.status_led_pin >= 30 || (used_mask & (1u32 << self.status_led_pin)) != 0 {
            self.status_led_pin = PIN_UNASSIGNED;
        }
        self.flags &= 0x07;
        let baud_code = (self.rs485_config & 0x0F).min(4);
        let alt_flag = self.rs485_config & RS485_FLAG_ALT_PINS_GP4_GP5;
        self.rs485_config = baud_code | alt_flag;
    }

    #[inline]
    pub const fn to_bytes(&self) -> [u8; IO_CONFIG_PAYLOAD_LEN] {
        [
            self.input_pins[0],
            self.input_pins[1],
            self.input_pins[2],
            self.input_pins[3],
            self.input_pins[4],
            self.input_pins[5],
            self.input_pins[6],
            self.input_pins[7],
            self.output_pins[0],
            self.output_pins[1],
            self.output_pins[2],
            self.output_pins[3],
            self.output_pins[4],
            self.output_pins[5],
            self.output_pins[6],
            self.output_pins[7],
            self.status_led_pin,
            self.flags,
            self.rs485_de_re_pin,
            self.rs485_config,
        ]
    }

    pub fn from_bytes(b: &[u8]) -> Option<Self> {
        if b.len() < LEGACY_IO_CONFIG_PAYLOAD_LEN {
            return None;
        }
        let (rs485_de_re_pin, rs485_config) = if b.len() >= IO_CONFIG_PAYLOAD_LEN {
            (b[18], b[19])
        } else {
            (PIN_UNASSIGNED, 0)
        };
        let mut cfg = Self {
            input_pins: [b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]],
            output_pins: [b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]],
            status_led_pin: b[16],
            flags: b[17],
            rs485_de_re_pin,
            rs485_config,
        };
        cfg.sanitize();
        Some(cfg)
    }

    /// Encode a complete `CMD_GET_IO_CONFIG_RESPONSE (0x8B)` frame (`25 bytes`) into `out_buf`.
    pub fn write_response_packet(&self, out_buf: &mut [u8]) -> Result<usize, &'static str> {
        if out_buf.len() < IO_CONFIG_PACKET_LEN {
            return Err("Buffer too small for IO config response packet");
        }
        out_buf[0] = PACKET_SOF;
        out_buf[1] = CMD_GET_IO_CONFIG_RESPONSE;
        let len_bytes = (IO_CONFIG_PAYLOAD_LEN as u16).to_le_bytes();
        out_buf[2] = len_bytes[0];
        out_buf[3] = len_bytes[1];
        out_buf[4..4 + IO_CONFIG_PAYLOAD_LEN].copy_from_slice(&self.to_bytes());
        out_buf[IO_CONFIG_PACKET_LEN - 1] =
            MicroVm::compute_crc8(&out_buf[1..IO_CONFIG_PACKET_LEN - 1]);
        Ok(IO_CONFIG_PACKET_LEN)
    }
}

/// RETAIN area (Flash-latched): `M64..M95` = `bit_mem[0x80..0xA0]`, `D96..D127` = `word_mem[0x80..0xA0]`
pub const RETAIN_BIT_START: usize = 0x80;
pub const RETAIN_WORD_START: usize = 0x80;
pub const RETAIN_LEN: usize = 32;

/// Online Monitoring Status Packet Lengths (includes 2B prog_len + 2B prog_crc16 signature)
pub const STATUS_SNAPSHOT_PAYLOAD_LEN: usize = 1 + 4 + 1 + 4 + 256 + 64 + 64 + 64 + 2 + 2; // 462 bytes
pub const STATUS_PACKET_LEN: usize = 5 + STATUS_SNAPSHOT_PAYLOAD_LEN; // 467 bytes
pub const MAX_PROGRAM_PACKET_LEN: usize = 5 + (MAX_INSTRUCTIONS * 4); // 2053 bytes

/// 32-bit Fixed-Width VM Instruction (`4 Bytes`)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(C)]
pub struct VmInstruction {
    pub op: u8,
    pub addr: u8,
    pub imm: u16,
}

impl VmInstruction {
    #[inline]
    pub const fn from_le_bytes(b: [u8; 4]) -> Self {
        Self {
            op: b[0],
            addr: b[1],
            imm: u16::from_le_bytes([b[2], b[3]]),
        }
    }

    #[inline]
    pub const fn to_le_bytes(self) -> [u8; 4] {
        let [lo, hi] = self.imm.to_le_bytes();
        [self.op, self.addr, lo, hi]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct StackItem {
    pending_op: u8,
    saved_acc: bool,
}

/// Core `#![no_std]` PLC Virtual Machine State
pub struct MicroVm {
    pub program: [VmInstruction; MAX_INSTRUCTIONS],
    pub prog_len: u16,
    pub prog_crc: u16,
    pub running: bool,
    pub scan_count: u32,
    pub acc_bool: bool,
    pub acc_word: i32,

    /// Unified Bit Memory (`X`, `Y`, `M`, `T-bit`, `C-bit`)
    pub bit_mem: [u8; 256],
    /// Force Mask for Bit Memory: 1 = Forced (overridden), 0 = Normal
    pub force_bit_mask: [u8; 256],
    /// Forced Bit Values: 1 = Force ON, 0 = Force OFF
    pub force_bit_val: [u8; 256],
    /// Previous scan bit state for rising/falling edge detection (`LDP`, `LDF`, `R_TRIG`, etc.)
    pub prev_bit_mem: [u8; 256],
    /// Unified Word Memory (`AI`, `AO`, `D`, `T-cv`, `C-cv`)
    pub word_mem: [i32; 256],
    /// Previous rung input state for counters (`CTU`, `CTD`, `CTUD`)
    pub ctu_prev: [u8; 32],
    /// Previous rung input state for `STP` / `STF` / `R_TRIG` / `F_TRIG`
    pub edge_prev: [u8; 256],
    /// IEC 61131-3 MB_MASTER Function Block instances
    pub mb_master: [ModbusMasterState; MAX_MB_MASTER_INSTANCES],
    /// Outgoing Modbus Job dispatched to background RTU task
    pub mb_out_job: Option<ModbusMasterJob>,
    /// Incoming completion result from background RTU task
    pub mb_in_result: Option<ModbusJobResult>,
    /// Incremental Job ID tracker
    pub next_job_id: u8,
    /// PID function block instances (`PID`, opcode 0x72)
    pub pid: [PidState; MAX_PID_INSTANCES],
    /// Outgoing UART job for the firmware UART task (taken like `mb_out_job`)
    pub uart_tx_out: Option<UartTx>,
    /// `true` while a UART job is queued / being transmitted (firmware clears it after sending)
    pub uart_tx_busy: bool,
    /// Incoming UART byte placed by the firmware (consumed by `URX`, 1 byte per scan)
    pub uart_rx_in: Option<u8>,
    /// Pending UartSend requests (rising edge latched while the UART is busy)
    pub uart_tx_req: [bool; MAX_UART_INSTANCES],
    /// Previous EN state per UartSend instance (rising-edge detection)
    pub uart_tx_prev: [bool; MAX_UART_INSTANCES],
    /// Previous EN state per edge slot for edge-triggered function blocks (Shift / ShiftReg)
    pub fb_edge_prev: [u8; MAX_FB_EDGE_SLOTS],
    /// Runtime-configurable Hardware I/O Pin Mapping (`X0..X7`, `Y0..Y7`, `status_led`, `flags`)
    pub io_config: HardwareIoConfig,
}

impl Default for MicroVm {
    fn default() -> Self {
        Self::new()
    }
}

impl MicroVm {
    pub const fn new() -> Self {
        Self {
            program: [VmInstruction {
                op: 0xFF,
                addr: 0,
                imm: 0,
            }; MAX_INSTRUCTIONS],
            prog_len: 0,
            prog_crc: 0,
            running: true,
            scan_count: 0,
            acc_bool: false,
            acc_word: 0,
            bit_mem: [0; 256],
            force_bit_mask: [0; 256],
            force_bit_val: [0; 256],
            prev_bit_mem: [0; 256],
            word_mem: [0; 256],
            ctu_prev: [0; 32],
            edge_prev: [0; 256],
            mb_master: [ModbusMasterState {
                prev_req: false,
                busy: false,
                done: false,
                error: false,
                status: 0,
                job_id: 0,
            }; MAX_MB_MASTER_INSTANCES],
            mb_out_job: None,
            mb_in_result: None,
            next_job_id: 0,
            pid: [PidState::new(); MAX_PID_INSTANCES],
            uart_tx_out: None,
            uart_tx_busy: false,
            uart_rx_in: None,
            uart_tx_req: [false; MAX_UART_INSTANCES],
            uart_tx_prev: [false; MAX_UART_INSTANCES],
            fb_edge_prev: [0; MAX_FB_EDGE_SLOTS],
            io_config: HardwareIoConfig::default_rp2040(),
        }
    }

    /// Force a digital bit (Input X, Output Y, or Internal Relay M) to ON (true) or OFF (false)
    pub fn force_bit(&mut self, addr: u8, val: bool) {
        let idx = addr as usize;
        self.force_bit_mask[idx] = 1;
        self.force_bit_val[idx] = if val { 1 } else { 0 };
        self.bit_mem[idx] = if val { 1 } else { 0 };
    }

    /// Release force on a digital bit, returning control to physical inputs or ladder logic
    pub fn unforce_bit(&mut self, addr: u8) {
        let idx = addr as usize;
        self.force_bit_mask[idx] = 0;
        self.force_bit_val[idx] = 0;
    }

    /// Release all active forces across all 256 bit memory addresses
    pub fn clear_all_forces(&mut self) {
        self.force_bit_mask = [0; 256];
        self.force_bit_val = [0; 256];
    }

    /// Returns Some(true) if forced ON, Some(false) if forced OFF, or None if not forced
    pub fn is_bit_forced(&self, addr: u8) -> Option<bool> {
        let idx = addr as usize;
        if self.force_bit_mask[idx] != 0 {
            Some(self.force_bit_val[idx] != 0)
        } else {
            None
        }
    }

    /// Direct write a 32-bit integer value into Word Memory (%MW / Data Register)
    pub fn write_word(&mut self, addr: u8, val: i32) {
        self.word_mem[addr as usize] = val;
    }

    /// Reset all outputs, internal relays, timers, counters, and communication jobs (Safe Shutdown)
    pub fn reset_memory(&mut self) {
        self.bit_mem = [0; 256];
        self.prev_bit_mem = [0; 256];
        self.word_mem = [0; 256];
        self.ctu_prev = [0; 32];
        self.edge_prev = [0; 256];
        self.mb_master = [ModbusMasterState::default(); MAX_MB_MASTER_INSTANCES];
        self.mb_out_job = None;
        self.mb_in_result = None;
        self.reset_fb_state();
        self.acc_bool = false;
        self.acc_word = 0;
        self.scan_count = 0;
    }

    /// Like [`reset_memory`](Self::reset_memory) but keeps the RETAIN area
    /// (`M64..M95` = `bit_mem[0x80..0xA0]`, `D96..D127` = `word_mem[0x80..0xA0]`). Used at boot
    /// after the retained values have been restored from Flash.
    pub fn reset_memory_keep_retain(&mut self) {
        let mut bits = [0u8; RETAIN_LEN];
        let mut words = [0i32; RETAIN_LEN];
        bits.copy_from_slice(&self.bit_mem[RETAIN_BIT_START..RETAIN_BIT_START + RETAIN_LEN]);
        words.copy_from_slice(&self.word_mem[RETAIN_WORD_START..RETAIN_WORD_START + RETAIN_LEN]);
        self.reset_memory();
        self.bit_mem[RETAIN_BIT_START..RETAIN_BIT_START + RETAIN_LEN].copy_from_slice(&bits);
        self.prev_bit_mem[RETAIN_BIT_START..RETAIN_BIT_START + RETAIN_LEN].copy_from_slice(&bits);
        self.word_mem[RETAIN_WORD_START..RETAIN_WORD_START + RETAIN_LEN].copy_from_slice(&words);
    }

    /// Reset PID / UART / edge state of the feature-level-2 function blocks
    fn reset_fb_state(&mut self) {
        self.pid = [PidState::new(); MAX_PID_INSTANCES];
        self.uart_tx_out = None;
        self.uart_tx_busy = false;
        self.uart_rx_in = None;
        self.uart_tx_req = [false; MAX_UART_INSTANCES];
        self.uart_tx_prev = [false; MAX_UART_INSTANCES];
        self.fb_edge_prev = [0; MAX_FB_EDGE_SLOTS];
    }

    /// Decode one `OPND` word `[0x6F, kind, imm_lo, imm_hi]` into its current value:
    /// kind 0 = `word_mem[imm_lo]`, kind 1 = sign-extended i16 constant, kind 2 = `bit_mem[imm_lo]`.
    pub fn read_opnd(&self, w: [u8; 4]) -> i32 {
        self.read_opnd_ins(VmInstruction::from_le_bytes(w))
    }

    #[inline]
    fn read_opnd_ins(&self, w: VmInstruction) -> i32 {
        match w.addr {
            OPND_KIND_WORD => self.word_mem[(w.imm & 0xFF) as usize],
            OPND_KIND_CONST => w.imm as i16 as i32,
            OPND_KIND_BIT => i32::from(self.bit_mem[(w.imm & 0xFF) as usize] != 0),
            _ => 0,
        }
    }

    /// Value of the operand word at `idx` (0 if out of range or not an `OPND` word)
    #[inline]
    fn opnd_at(&self, idx: usize, max_pc: usize) -> i32 {
        if idx < max_pc && self.program[idx].op == OP_OPND {
            self.read_opnd_ins(self.program[idx])
        } else {
            0
        }
    }

    /// Rising-edge detection on an edge slot; always records the new state
    #[inline]
    fn fb_rising_edge(&mut self, slot: usize, en: bool) -> bool {
        let slot = slot % MAX_FB_EDGE_SLOTS;
        let prev = self.fb_edge_prev[slot] != 0;
        self.fb_edge_prev[slot] = u8::from(en);
        en && !prev
    }

    /// Load raw 32-bit aligned bytecode payload (`4 * N` bytes) into VM program memory
    pub fn load_bytecode(&mut self, payload: &[u8]) -> Result<u16, &'static str> {
        if payload.len() % 4 != 0 {
            return Err("Payload length must be a multiple of 4 bytes");
        }
        let count = payload.len() / 4;
        if count > MAX_INSTRUCTIONS {
            return Err("Program exceeds MAX_INSTRUCTIONS");
        }

        for (i, chunk) in payload.chunks_exact(4).enumerate() {
            self.program[i] = VmInstruction::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        }
        self.prog_len = count as u16;
        self.prog_crc = Self::compute_crc16(payload);
        // Clear outputs Y0..Y31 on program reload for safety
        for y in 0x20..=0x3F {
            self.bit_mem[y] = 0;
        }
        Ok(self.prog_len)
    }

    #[inline]
    pub fn set_digital_input(&mut self, x_index: u8, state: bool) {
        if x_index < 32 {
            let idx = x_index as usize;
            if self.force_bit_mask[idx] != 0 {
                self.bit_mem[idx] = self.force_bit_val[idx];
            } else {
                self.bit_mem[idx] = u8::from(state);
            }
        }
    }

    #[inline]
    pub fn get_digital_output(&self, y_index: u8) -> bool {
        if y_index < 32 {
            let idx = (0x20 + y_index) as usize;
            if self.force_bit_mask[idx] != 0 {
                self.force_bit_val[idx] != 0
            } else {
                self.bit_mem[idx] != 0
            }
        } else {
            false
        }
    }

    /// Execute 1 full PLC Scan Cycle with elapsed time `dt_ms` (in milliseconds)
    pub fn step_scan(&mut self, dt_ms: u32) {
        if !self.running || self.prog_len == 0 {
            return;
        }

        // 1. Clear 1-scan pulses (DONE and ERROR) for Modbus Master FBs
        for inst in &mut self.mb_master {
            inst.done = false;
            inst.error = false;
        }

        // 2. Consume incoming Modbus communication results from background task
        if let Some(res) = self.mb_in_result.take() {
            let inst = (res.instance_id as usize).min(MAX_MB_MASTER_INSTANCES - 1);
            if self.mb_master[inst].busy && self.mb_master[inst].job_id == res.job_id {
                self.mb_master[inst].busy = false;
                if res.success {
                    self.mb_master[inst].done = true;
                    self.mb_master[inst].error = false;
                    self.mb_master[inst].status = MB_ERR_OK;
                } else {
                    self.mb_master[inst].done = false;
                    self.mb_master[inst].error = true;
                    self.mb_master[inst].status = res.error_code;
                }
            }
        }

        let mut acc = false;
        let mut mc_active = true;
        let mut stack = [StackItem::default(); MAX_STACK_DEPTH];
        let mut sp: usize = 0;
        let mut pc: usize = 0;
        let max_pc = (self.prog_len as usize).min(MAX_INSTRUCTIONS);
        let mut loop_guard: usize = 0;

        while pc < max_pc && loop_guard < MAX_INSTRUCTIONS * 2 {
            loop_guard += 1;
            let ins = self.program[pc];
            let addr = ins.addr as usize;
            let bit_val = self.bit_mem[addr] != 0;
            let prev_bit_val = self.prev_bit_mem[addr] != 0;

            match ins.op {
                0x00 => {} // NOP
                0x01 => acc = bit_val && mc_active,                         // LD
                0x02 => acc = (!bit_val) && mc_active,                      // LDN
                0x03 => acc = acc && bit_val,                               // AND
                0x04 => acc = acc && !bit_val,                              // ANDN
                0x05 => acc = acc || (bit_val && mc_active),                // OR
                0x06 => acc = acc || ((!bit_val) && mc_active),             // ORN
                0x07 => self.bit_mem[addr] = u8::from(acc && mc_active),    // ST
                0x08 => self.bit_mem[addr] = u8::from((!acc) && mc_active), // STN
                0x09 => {
                    if acc && mc_active {
                        self.bit_mem[addr] = 1; // S (Set Latch)
                    }
                }
                0x0A => {
                    if acc && mc_active {
                        self.bit_mem[addr] = 0; // R (Reset Unlatch)
                        if addr >= 0xC0 {
                            self.word_mem[addr] = 0;
                        }
                    }
                }
                0x0B => acc = (bit_val && !prev_bit_val) && mc_active,  // LDP
                0x0C => acc = (!bit_val && prev_bit_val) && mc_active,  // LDF
                0x0D => acc = acc && (bit_val && !prev_bit_val),        // ANDP
                0x0E => acc = acc && (!bit_val && prev_bit_val),        // ANDF
                0x0F => acc = acc || ((bit_val && !prev_bit_val) && mc_active), // ORP
                0x13 => acc = acc || ((!bit_val && prev_bit_val) && mc_active), // ORF

                0x10 => {
                    // AND(
                    if sp < MAX_STACK_DEPTH {
                        stack[sp] = StackItem {
                            pending_op: 0x03,
                            saved_acc: acc,
                        };
                        sp += 1;
                    }
                }
                0x11 => {
                    // OR(
                    if sp < MAX_STACK_DEPTH {
                        stack[sp] = StackItem {
                            pending_op: 0x05,
                            saved_acc: acc,
                        };
                        sp += 1;
                    }
                }
                0x12 => {
                    // )
                    if sp > 0 {
                        sp -= 1;
                        let item = stack[sp];
                        if item.pending_op == 0x03 {
                            acc = item.saved_acc && acc;
                        } else {
                            acc = item.saved_acc || acc;
                        }
                    }
                }

                0x14 | 0x16 => {
                    // STP / R_TRIG
                    let prev = self.edge_prev[addr] != 0;
                    let pulse = acc && !prev;
                    self.edge_prev[addr] = u8::from(acc);
                    if ins.op == 0x14 {
                        self.bit_mem[addr] = u8::from(pulse && mc_active);
                    } else {
                        acc = pulse && mc_active;
                    }
                }
                0x15 | 0x17 => {
                    // STF / F_TRIG
                    let prev = self.edge_prev[addr] != 0;
                    let pulse = !acc && prev;
                    self.edge_prev[addr] = u8::from(acc);
                    if ins.op == 0x15 {
                        self.bit_mem[addr] = u8::from(pulse && mc_active);
                    } else {
                        acc = pulse && mc_active;
                    }
                }
                0x18 => {
                    // SR (Set Dominant)
                    if acc && mc_active {
                        self.bit_mem[addr] = 1;
                    }
                    acc = self.bit_mem[addr] != 0;
                }
                0x19 => {
                    // RS (Reset Dominant)
                    if acc && mc_active {
                        self.bit_mem[addr] = 1;
                    }
                    acc = self.bit_mem[addr] != 0;
                }
                0x1A => {
                    // MC (Master Control Relay)
                    mc_active = acc;
                }

                0x20 => {
                    // TON (Timer On-Delay)
                    let pt = ins.imm as i32;
                    if acc && mc_active {
                        let next_cv = self.word_mem[addr].saturating_add(dt_ms as i32);
                        if next_cv >= pt {
                            self.word_mem[addr] = pt;
                            self.bit_mem[addr] = 1;
                        } else {
                            self.word_mem[addr] = next_cv;
                            self.bit_mem[addr] = 0;
                        }
                        self.acc_word = self.word_mem[addr];
                    } else {
                        self.word_mem[addr] = 0;
                        self.bit_mem[addr] = 0;
                    }
                    acc = self.bit_mem[addr] != 0;
                }
                0x22 => {
                    // TOF (Timer Off-Delay)
                    let pt = ins.imm as i32;
                    if acc && mc_active {
                        self.word_mem[addr] = 0;
                        self.bit_mem[addr] = 1;
                    } else if self.bit_mem[addr] != 0 {
                        let next_cv = self.word_mem[addr].saturating_add(dt_ms as i32);
                        if next_cv >= pt {
                            self.word_mem[addr] = pt;
                            self.bit_mem[addr] = 0;
                        } else {
                            self.word_mem[addr] = next_cv;
                        }
                        self.acc_word = self.word_mem[addr];
                    }
                    acc = self.bit_mem[addr] != 0;
                }
                0x23 => {
                    // TP (Pulse Timer)
                    let pt = ins.imm as i32;
                    if acc && mc_active && self.word_mem[addr] == 0 && self.bit_mem[addr] == 0 {
                        self.bit_mem[addr] = 1;
                        self.word_mem[addr] = dt_ms as i32;
                        self.acc_word = self.word_mem[addr];
                    } else if self.bit_mem[addr] != 0 {
                        let next_cv = self.word_mem[addr].saturating_add(dt_ms as i32);
                        if next_cv >= pt {
                            self.word_mem[addr] = pt;
                            self.bit_mem[addr] = 0;
                        } else {
                            self.word_mem[addr] = next_cv;
                        }
                        self.acc_word = self.word_mem[addr];
                    } else if !acc {
                        self.word_mem[addr] = 0;
                    }
                    acc = self.bit_mem[addr] != 0;
                }
                0x21 | 0x25 => {
                    // CTU / CTUD (Counter Up)
                    let c_idx = addr.saturating_sub(0xE0) & 0x1F;
                    let prev_cu = self.ctu_prev[c_idx] != 0;
                    if acc && mc_active && !prev_cu {
                        self.word_mem[addr] = self.word_mem[addr].saturating_add(1);
                    }
                    self.bit_mem[addr] = u8::from(self.word_mem[addr] >= ins.imm as i32);
                    self.ctu_prev[c_idx] = u8::from(acc && mc_active);
                    self.acc_word = self.word_mem[addr];
                    acc = self.bit_mem[addr] != 0;
                }
                0x24 => {
                    // CTD (Counter Down)
                    let prev_cd = self.edge_prev[addr] != 0;
                    if acc && mc_active && !prev_cd {
                        self.word_mem[addr] = (self.word_mem[addr] - 1).max(0);
                        self.bit_mem[addr] = u8::from(self.word_mem[addr] <= 0);
                    } else if self.word_mem[addr] > 0 && self.word_mem[addr] < ins.imm as i32 {
                        self.bit_mem[addr] = 0;
                    }
                    self.edge_prev[addr] = u8::from(acc && mc_active);
                    self.acc_word = self.word_mem[addr];
                    acc = self.bit_mem[addr] != 0;
                }

                // Comparisons
                0x30 => {
                    self.acc_word = self.word_mem[addr];
                    acc = acc && (self.word_mem[addr] == ins.imm as i32); // EQ
                }
                0x31 => {
                    self.acc_word = self.word_mem[addr];
                    acc = acc && (self.word_mem[addr] != ins.imm as i32); // NE
                }
                0x32 => {
                    self.acc_word = self.word_mem[addr];
                    acc = acc && (self.word_mem[addr] > ins.imm as i32);  // GT
                }
                0x33 => {
                    self.acc_word = self.word_mem[addr];
                    acc = acc && (self.word_mem[addr] < ins.imm as i32);  // LT
                }
                0x34 => {
                    self.acc_word = self.word_mem[addr];
                    acc = acc && (self.word_mem[addr] >= ins.imm as i32); // GE
                }
                0x35 => {
                    self.acc_word = self.word_mem[addr];
                    acc = acc && (self.word_mem[addr] <= ins.imm as i32); // LE
                }

                // Word Move & Math
                0x40 => {
                    if acc && mc_active {
                        self.word_mem[addr] = ins.imm as i32;
                        self.acc_word = self.word_mem[addr];
                    }
                }
                0x41 => {
                    if acc && mc_active {
                        self.word_mem[addr] = self.word_mem[addr].saturating_add(ins.imm as i32);
                        self.acc_word = self.word_mem[addr];
                    }
                }
                0x42 => {
                    if acc && mc_active {
                        self.word_mem[addr] = self.word_mem[addr].saturating_sub(ins.imm as i32);
                        self.acc_word = self.word_mem[addr];
                    }
                }
                0x43 => {
                    if acc && mc_active {
                        self.word_mem[addr] = self.word_mem[addr].saturating_mul(ins.imm as i32);
                        self.acc_word = self.word_mem[addr];
                    }
                }
                0x44 => {
                    if acc && mc_active && ins.imm != 0 {
                        self.word_mem[addr] /= ins.imm as i32;
                        self.acc_word = self.word_mem[addr];
                    }
                }
                0x45 => {
                    if acc && mc_active && ins.imm != 0 {
                        self.word_mem[addr] %= ins.imm as i32;
                        self.acc_word = self.word_mem[addr];
                    }
                }

                // Conditional Jump
                0x50 => {
                    if acc && (ins.imm as usize) < max_pc {
                        pc = ins.imm as usize;
                        continue;
                    }
                }
                0x51 => {} // LBL (Jump Target)

                // Modbus Master Communication Function Block (IEC 61131-3)
                0x60 => {
                    // MB_MASTER:
                    // addr: instance_id (0..7)
                    // imm: (slave_id as u16) | ((func as u16) << 8)
                    let inst_id = (addr as usize).min(MAX_MB_MASTER_INSTANCES - 1);
                    let slave_id = (ins.imm & 0xFF) as u8;
                    let func = ((ins.imm >> 8) & 0xFF) as u8;

                    let p1 = if pc + 1 < max_pc { self.program[pc + 1] } else { VmInstruction::default() };
                    let p2 = if pc + 2 < max_pc { self.program[pc + 2] } else { VmInstruction::default() };
                    pc += 2;

                    let rising_req = acc && !self.mb_master[inst_id].prev_req && mc_active;
                    self.mb_master[inst_id].prev_req = acc;

                    if rising_req {
                        if self.mb_master[inst_id].busy {
                            self.mb_master[inst_id].error = true;
                            self.mb_master[inst_id].status = MB_ERR_BUSY;
                        } else {
                            self.next_job_id = self.next_job_id.wrapping_add(1);
                            if self.next_job_id == 0 {
                                self.next_job_id = 1;
                            }
                            let j_id = self.next_job_id;

                            self.mb_master[inst_id].busy = true;
                            self.mb_master[inst_id].done = false;
                            self.mb_master[inst_id].error = false;
                            self.mb_master[inst_id].status = 0;
                            self.mb_master[inst_id].job_id = j_id;

                            let is_write = (p2.addr & 0x01) != 0
                                || matches!(
                                    func,
                                    MB_FC_WRITE_SINGLE_COIL
                                        | MB_FC_WRITE_SINGLE_REGISTER
                                        | MB_FC_WRITE_MULTIPLE_REGISTERS
                                );

                            self.mb_out_job = Some(ModbusMasterJob {
                                job_id: j_id,
                                instance_id: inst_id as u8,
                                slave_id,
                                function_code: func,
                                reg_addr: p1.imm,
                                count: p2.imm,
                                buffer_addr: p1.addr,
                                is_write,
                            });
                        }
                    }
                    acc = self.mb_master[inst_id].done;
                }
                0x61 | 0x62 => {} // MB_PARAM1, MB_PARAM2 (consumed by 0x60)
                0x63 => {
                    // MB_STATUS: Query FB output flags (imm: 0=DONE, 1=BUSY, 2=ERROR, 3=STATUS)
                    let inst_id = (addr as usize).min(MAX_MB_MASTER_INSTANCES - 1);
                    match ins.imm {
                        0 => acc = self.mb_master[inst_id].done,
                        1 => acc = self.mb_master[inst_id].busy,
                        2 => acc = self.mb_master[inst_id].error,
                        3 => self.acc_word = self.mb_master[inst_id].status as i32,
                        _ => {}
                    }
                }

                // ---------------- Feature level 2: OPND-based function blocks ----------------
                OP_MOVW => {
                    // MOVW dst, A
                    if acc && mc_active {
                        let a = self.opnd_at(pc + 1, max_pc);
                        self.word_mem[addr] = a;
                        self.acc_word = a;
                    }
                    pc += 1;
                }
                OP_WAND | OP_WOR | OP_WXOR | OP_WNOT | OP_SHL | OP_SHR => {
                    // dst := A op B (imm bit 15 = rising-edge trigger on slot imm & 0xFF)
                    let en = acc && mc_active;
                    let fire = if (ins.op == OP_SHL || ins.op == OP_SHR) && (ins.imm & SHIFT_EDGE_FLAG) != 0 {
                        self.fb_rising_edge((ins.imm & 0xFF) as usize, en)
                    } else {
                        en
                    };
                    if fire {
                        let a = self.opnd_at(pc + 1, max_pc);
                        let b = self.opnd_at(pc + 2, max_pc);
                        let r = bitwise(ins.op, a, b);
                        self.word_mem[addr] = r;
                        self.acc_word = r;
                    }
                    pc += 2;
                }
                OP_SHREG => {
                    // SHREG base, imm = len | (edge_slot << 8); IN operand
                    let en = acc && mc_active;
                    let len = ((ins.imm & 0xFF) as usize).clamp(1, 32);
                    let slot = (ins.imm >> 8) as usize;
                    if self.fb_rising_edge(slot, en) {
                        let input = self.opnd_at(pc + 1, max_pc);
                        let last = (addr + len - 1).min(255);
                        let mut i = last;
                        while i > addr {
                            self.word_mem[i] = self.word_mem[i - 1];
                            i -= 1;
                        }
                        self.word_mem[addr] = input;
                        self.acc_word = input;
                    }
                    pc += 1;
                }
                OP_SHBIT => {
                    // SHBIT word, imm = edge_slot; BIT operand: w = (w << 1) | bit
                    let en = acc && mc_active;
                    if self.fb_rising_edge((ins.imm & 0xFF) as usize, en) {
                        let bit = self.opnd_at(pc + 1, max_pc);
                        self.word_mem[addr] = shift_in_bit(self.word_mem[addr], bit);
                        self.acc_word = self.word_mem[addr];
                    }
                    pc += 1;
                }
                OP_SCALE => {
                    // SCALE dst; IN, in_min, in_max, out_min, out_max
                    if acc && mc_active {
                        let v = scale(
                            self.opnd_at(pc + 1, max_pc),
                            self.opnd_at(pc + 2, max_pc),
                            self.opnd_at(pc + 3, max_pc),
                            self.opnd_at(pc + 4, max_pc),
                            self.opnd_at(pc + 5, max_pc),
                        );
                        self.word_mem[addr] = v;
                        self.acc_word = v;
                    }
                    pc += 5;
                }
                OP_LIMIT => {
                    // LIMIT dst; IN, MIN, MAX
                    if acc && mc_active {
                        let v = limit(
                            self.opnd_at(pc + 1, max_pc),
                            self.opnd_at(pc + 2, max_pc),
                            self.opnd_at(pc + 3, max_pc),
                        );
                        self.word_mem[addr] = v;
                        self.acc_word = v;
                    }
                    pc += 3;
                }
                OP_PID => {
                    // PID dst(CV), imm = instance; PV, SP, Kp, Ki, Kd, OutMax
                    let inst = (ins.imm as usize) % MAX_PID_INSTANCES;
                    if acc && mc_active {
                        let pv = self.opnd_at(pc + 1, max_pc);
                        let sp_v = self.opnd_at(pc + 2, max_pc);
                        let kp = self.opnd_at(pc + 3, max_pc);
                        let ki = self.opnd_at(pc + 4, max_pc);
                        let kd = self.opnd_at(pc + 5, max_pc);
                        let out_max = self.opnd_at(pc + 6, max_pc);
                        let cv = self.pid[inst].step(pv, sp_v, kp, ki, kd, out_max, dt_ms);
                        self.word_mem[addr] = cv;
                        self.acc_word = cv;
                    } else {
                        self.pid[inst].reset();
                    }
                    pc += 6;
                }
                OP_UTX => {
                    // UTX inst, imm = mode; IN operand. Rising edge latches a request; ENO = sent/idle
                    let inst = addr % MAX_UART_INSTANCES;
                    let en = acc && mc_active;
                    if en && !self.uart_tx_prev[inst] {
                        self.uart_tx_req[inst] = true;
                    }
                    self.uart_tx_prev[inst] = en;
                    if self.uart_tx_req[inst] && !self.uart_tx_busy && self.uart_tx_out.is_none() {
                        let value = self.opnd_at(pc + 1, max_pc);
                        self.uart_tx_out = Some(UartTx {
                            instance: inst as u8,
                            mode: (ins.imm & 0xFF) as u8,
                            value,
                        });
                        self.uart_tx_busy = true;
                        self.uart_tx_req[inst] = false;
                    }
                    acc = en && !self.uart_tx_req[inst] && !self.uart_tx_busy;
                    pc += 1;
                }
                OP_URX => {
                    // URX dst: ENO = 1 only in the scan a byte was received
                    let mut got = false;
                    if acc && mc_active {
                        if let Some(b) = self.uart_rx_in.take() {
                            self.word_mem[addr] = b as i32;
                            self.acc_word = b as i32;
                            got = true;
                        }
                    }
                    acc = got;
                }
                OP_OPND => {} // Operand word (consumed by the preceding header opcode)

                0xFF => break, // END
                _ => {}
            }

            pc += 1;
        }

        self.acc_bool = acc;
        // Re-assert forced values for outputs and internal relays after ladder logic execution
        for i in 0x20..256 {
            if self.force_bit_mask[i] != 0 {
                self.bit_mem[i] = self.force_bit_val[i];
            }
        }
        self.prev_bit_mem = self.bit_mem;
        self.scan_count = self.scan_count.wrapping_add(1);
    }

    /// Write complete Online Monitoring Status Packet Frame into `out` buffer:
    /// `[0xAA] [0x83] [LEN_LO] [LEN_HI] [PAYLOAD (458 bytes)] [CRC8]`
    /// Returns total bytes written (STATUS_PACKET_LEN = 463 bytes)
    pub fn write_status_packet(&self, out: &mut [u8]) -> Result<usize, &'static str> {
        if out.len() < STATUS_PACKET_LEN {
            return Err("Buffer too small for status packet");
        }
        out[0] = PACKET_SOF;
        out[1] = CMD_STATUS_RESPONSE; // 0x83
        let payload_len = STATUS_SNAPSHOT_PAYLOAD_LEN as u16;
        let [l0, l1] = payload_len.to_le_bytes();
        out[2] = l0;
        out[3] = l1;

        let mut idx = 4;
        out[idx] = if self.running { 1 } else { 0 };
        idx += 1;

        out[idx..idx + 4].copy_from_slice(&self.scan_count.to_le_bytes());
        idx += 4;

        // acc_bool byte: bit0 = acc_bool, high nibble = FW_FEATURE_LEVEL (legacy firmware sends 0/1)
        out[idx] = (if self.acc_bool { 1 } else { 0 }) | (FW_FEATURE_LEVEL << 4);
        idx += 1;

        out[idx..idx + 4].copy_from_slice(&self.acc_word.to_le_bytes());
        idx += 4;

        out[idx..idx + 256].copy_from_slice(&self.bit_mem);
        // Encode %SD7 (word_mem[0xA7], RETAIN Flash save count) in bit 1 of bit_mem[0xA0..0xB0]
        let sd7 = self.word_mem[0xA7] as u16;
        for b in 0..16 {
            out[idx + 0xA0 + b] = (out[idx + 0xA0 + b] & 0x01) | (((sd7 >> b) as u8 & 0x01) << 1);
        }
        idx += 256;

        // Timers T0..T15 current values (word_mem[0xC0..0xD0])
        for i in 0..16 {
            let cv = (self.word_mem[0xC0 + i].max(0)) as u32;
            out[idx..idx + 4].copy_from_slice(&cv.to_le_bytes());
            idx += 4;
        }

        // Counters C0..C15 current values (word_mem[0xE0..0xF0])
        for i in 0..16 {
            let cv = self.word_mem[0xE0 + i];
            out[idx..idx + 4].copy_from_slice(&cv.to_le_bytes());
            idx += 4;
        }

        // Data Registers D0..D15 / %MW0..%MW15 (word_mem[0x20..0x30]) with fallback to AI0..AI15 (word_mem[0..16])
        for i in 0..16 {
            let d_val = self.word_mem[0x20 + i];
            let val = if d_val != 0 { d_val } else { self.word_mem[i] };
            out[idx..idx + 4].copy_from_slice(&val.to_le_bytes());
            idx += 4;
        }

        // Program Signature: prog_len (u16 LE) + prog_crc (u16 LE)
        out[idx..idx + 2].copy_from_slice(&self.prog_len.to_le_bytes());
        idx += 2;
        out[idx..idx + 2].copy_from_slice(&self.prog_crc.to_le_bytes());
        idx += 2;

        let crc = Self::compute_crc8(&out[1..idx]);
        out[idx] = crc;
        idx += 1;
        Ok(idx)
    }

    /// Construct a complete `CMD_UPLOAD_PROGRAM_RESPONSE (0x84)` Serial Packet Frame containing
    /// the full currently loaded 32-bit bytecode program (`prog_len * 4` bytes).
    ///
    /// Frame layout:
    /// - `[0x00]`: `PACKET_SOF` (`0xAA`)
    /// - `[0x01]`: `CMD_UPLOAD_PROGRAM_RESPONSE` (`0x84`)
    /// - `[0x02..0x04]`: Payload length in bytes (`prog_len * 4`, 16-bit LE)
    /// - `[0x04..4 + N]`: Raw instructions (`[op, addr, imm_lo, imm_hi] * prog_len`)
    /// - `[4 + N]`: CRC-8 checksum over bytes `1..4 + N`
    pub fn write_program_packet(&self, out: &mut [u8]) -> Result<usize, &'static str> {
        let payload_len = (self.prog_len as usize) * 4;
        let total_len = 5 + payload_len;
        if out.len() < total_len {
            return Err("Buffer too small for program packet");
        }

        out[0] = PACKET_SOF;
        out[1] = CMD_UPLOAD_PROGRAM_RESPONSE;
        let [len_lo, len_hi] = (payload_len as u16).to_le_bytes();
        out[2] = len_lo;
        out[3] = len_hi;

        let mut idx = 4;
        for i in 0..(self.prog_len as usize) {
            let ins_bytes = self.program[i].to_le_bytes();
            out[idx..idx + 4].copy_from_slice(&ins_bytes);
            idx += 4;
        }

        let crc = Self::compute_crc8(&out[1..idx]);
        out[idx] = crc;
        idx += 1;
        Ok(idx)
    }

    /// Compute CRC-8 (polynomial 0x07) over `data`
    pub fn compute_crc8(data: &[u8]) -> u8 {
        let mut crc: u8 = 0x00;
        for &byte in data {
            crc ^= byte;
            for _ in 0..8 {
                if (crc & 0x80) != 0 {
                    crc = (crc << 1) ^ 0x07;
                } else {
                    crc <<= 1;
                }
            }
        }
        crc
    }

    /// Compute CRC-16/CCITT-FALSE (polynomial 0x1021, init 0xFFFF) over `data` for Bytecode Signature Verification
    pub fn compute_crc16(data: &[u8]) -> u16 {
        let mut crc: u16 = 0xFFFF;
        for &byte in data {
            crc ^= (byte as u16) << 8;
            for _ in 0..8 {
                if (crc & 0x8000) != 0 {
                    crc = (crc << 1) ^ 0x1021;
                } else {
                    crc <<= 1;
                }
            }
        }
        crc
    }

    /// Parse and apply a complete Serial Packet Frame received from uPLC Studio:
    /// `[0xAA] [CMD] [LEN_LO] [LEN_HI] [PAYLOAD...] [CRC8]`
    pub fn process_serial_packet(&mut self, frame: &[u8]) -> Result<u8, &'static str> {
        if frame.len() < 5 {
            return Err("Frame too short");
        }
        if frame[0] != PACKET_SOF {
            return Err("Invalid SOF byte (expected 0xAA)");
        }
        let cmd = frame[1];
        let payload_len = u16::from_le_bytes([frame[2], frame[3]]) as usize;
        if frame.len() < 5 + payload_len {
            return Err("Incomplete frame payload");
        }
        let payload = &frame[4..4 + payload_len];
        let expected_crc = frame[4 + payload_len];
        let actual_crc = Self::compute_crc8(&frame[1..4 + payload_len]);
        if actual_crc != expected_crc {
            return Err("CRC8 checksum mismatch");
        }

        match cmd {
            CMD_DOWNLOAD_PROGRAM => {
                self.load_bytecode(payload)?;
                self.running = true;
                Ok(CMD_DOWNLOAD_PROGRAM)
            }
            CMD_RUN_STOP => {
                if payload.len() == 1 {
                    self.running = payload[0] != 0;
                } else {
                    self.running = !self.running;
                }
                if !self.running {
                    // Safe output shutdown on STOP
                    for y in 0x20..=0x3F {
                        self.bit_mem[y] = 0;
                    }
                }
                Ok(CMD_RUN_STOP)
            }
            CMD_MONITOR_STATUS => Ok(CMD_MONITOR_STATUS),
            CMD_UPLOAD_PROGRAM => Ok(CMD_UPLOAD_PROGRAM),
            CMD_STEP => {
                if self.prog_len > 0 {
                    let was_running = self.running;
                    self.running = true;
                    self.step_scan(10);
                    self.running = was_running;
                }
                Ok(CMD_STEP)
            }
            CMD_FORCE_IO => {
                // Payload: [ADDR: 1B, ACTION: 1B]
                // ACTION: 0 = Unforce, 1 = Force ON, 2 = Force OFF, 0xFF = Clear All Forces
                if payload.len() >= 2 {
                    let addr = payload[0];
                    let action = payload[1];
                    match action {
                        0 => self.unforce_bit(addr),
                        1 => self.force_bit(addr, true),
                        2 => self.force_bit(addr, false),
                        0xFF => self.clear_all_forces(),
                        _ => return Err("Invalid force action"),
                    }
                    Ok(CMD_FORCE_IO)
                } else if payload.len() == 1 && payload[0] == 0xFF {
                    self.clear_all_forces();
                    Ok(CMD_FORCE_IO)
                } else {
                    Err("Invalid CMD_FORCE_IO payload")
                }
            }
            CMD_WRITE_VALUE => {
                // Payload: [TARGET: 1B (0=Bit, 1=Word), ADDR: 1B, VAL: 4B LE]
                if payload.len() >= 6 {
                    let target_type = payload[0];
                    let addr = payload[1];
                    let val = i32::from_le_bytes([payload[2], payload[3], payload[4], payload[5]]);
                    if target_type == 0 {
                        self.bit_mem[addr as usize] = if val != 0 { 1 } else { 0 };
                    } else {
                        self.word_mem[addr as usize] = val;
                    }
                    Ok(CMD_WRITE_VALUE)
                } else {
                    Err("Invalid CMD_WRITE_VALUE payload")
                }
            }
            CMD_PERSIST_SAVE => Ok(CMD_PERSIST_SAVE),
            CMD_PERSIST_CLEAR => Ok(CMD_PERSIST_CLEAR),
            CMD_ENTER_BOOTSEL => {
                self.running = false;
                for y in 0x20..=0x3F {
                    self.bit_mem[y] = 0;
                }
                Ok(CMD_ENTER_BOOTSEL)
            }
            CMD_GET_IO_CONFIG => Ok(CMD_GET_IO_CONFIG),
            CMD_SET_IO_CONFIG => {
                if let Some(cfg) = HardwareIoConfig::from_bytes(payload) {
                    self.io_config = cfg;
                    Ok(CMD_SET_IO_CONFIG)
                } else {
                    Err("Invalid CMD_SET_IO_CONFIG payload")
                }
            }
            _ => Err("Unknown command code"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vm_self_holding_and_ton() {
        let mut vm = MicroVm::new();
        // Same 8 instructions (32 bytes) from BytecodeCompiler test:
        // 0: LD X0         (01 00 00 00)
        // 1: OR Y0         (05 20 00 00)
        // 2: ANDN X1       (04 01 00 00)
        // 3: ST Y0         (07 20 00 00)
        // 4: TON T0, K2000 (20 C0 D0 07)
        // 5: LD T0         (01 C0 00 00)
        // 6: ST Y1         (07 21 00 00)
        // 7: END           (FF 00 00 00)
        let raw: [u8; 32] = [
            0x01, 0x00, 0x00, 0x00,
            0x05, 0x20, 0x00, 0x00,
            0x04, 0x01, 0x00, 0x00,
            0x07, 0x20, 0x00, 0x00,
            0x20, 0xC0, 0xD0, 0x07,
            0x01, 0xC0, 0x00, 0x00,
            0x07, 0x21, 0x00, 0x00,
            0xFF, 0x00, 0x00, 0x00,
        ];
        vm.load_bytecode(&raw).unwrap();

        // 1. Press Start X0 = 1
        vm.set_digital_input(0, true);
        vm.step_scan(10);
        assert!(vm.get_digital_output(0), "Y0 should latch ON");
        assert!(!vm.get_digital_output(1), "Y1 should still be OFF (10ms < 2000ms)");

        // 2. Release Start X0 = 0 (Self-holding keeps Y0 ON)
        vm.set_digital_input(0, false);
        vm.step_scan(1000);
        assert!(vm.get_digital_output(0), "Y0 remains latched");
        assert!(!vm.get_digital_output(1), "Y1 still OFF at 1010ms");

        // 3. Advance another 1000ms -> Total 2010ms >= 2000ms -> T0 Done -> Y1 ON!
        vm.step_scan(1000);
        assert!(vm.get_digital_output(0));
        assert!(vm.get_digital_output(1), "Y1 should turn ON after TON T0 reaches 2000ms");

        // 4. Press Stop X1 = 1 -> Unlatches Y0 and resets T0 & Y1
        vm.set_digital_input(1, true);
        vm.step_scan(10);
        assert!(!vm.get_digital_output(0), "Y0 should turn OFF");
        assert!(!vm.get_digital_output(1), "Y1 should turn OFF");
    }

    #[test]
    fn test_vm_demo_led_blink_oscillator() {
        let mut vm = MicroVm::new();
        // 14-instruction Demo LED Blinker from example/08_rp2040_demo_led_blink.il:
        let raw: [u8; 56] = [
            0x02, 0x01, 0x00, 0x00, // 0: LDN  X1
            0x04, 0x41, 0x00, 0x00, // 1: ANDN M1
            0x20, 0xC0, 0xF4, 0x01, // 2: TON  T0, K500
            0x07, 0x40, 0x00, 0x00, // 3: ST   M0
            0x01, 0x40, 0x00, 0x00, // 4: LD   M0
            0x20, 0xC1, 0xF4, 0x01, // 5: TON  T1, K500
            0x07, 0x41, 0x00, 0x00, // 6: ST   M1
            0x02, 0x01, 0x00, 0x00, // 7: LDN  X1
            0x04, 0x40, 0x00, 0x00, // 8: ANDN M0
            0x05, 0x00, 0x00, 0x00, // 9: OR   X0
            0x07, 0x20, 0x00, 0x00, // 10: ST  Y0 (LED_PIN GP25)
            0x01, 0x40, 0x00, 0x00, // 11: LD  M0
            0x07, 0x21, 0x00, 0x00, // 12: ST  Y1 (LED_ALT GP11)
            0xFF, 0x00, 0x00, 0x00, // 13: END
        ];
        vm.load_bytecode(&raw).unwrap();

        // Phase 1 (0..490ms): Y0 (LED_PIN) is ON, Y1 is OFF
        for _ in 0..49 {
            vm.step_scan(10);
            assert!(vm.get_digital_output(0), "LED_PIN (Y0) should be ON during first 500ms");
            assert!(!vm.get_digital_output(1), "LED_ALT (Y1) should be OFF during first 500ms");
        }

        // Phase 2 (500..980ms): T0 reaches 500ms -> M0=1 -> Y0 (LED_PIN) turns OFF, Y1 turns ON
        for _ in 0..49 {
            vm.step_scan(10);
            assert!(!vm.get_digital_output(0), "LED_PIN (Y0) should be OFF during second 500ms");
            assert!(vm.get_digital_output(1), "LED_ALT (Y1) should be ON during second 500ms");
        }

        // At 990ms (50th scan of T1): T1 reaches 500ms -> M1=1
        vm.step_scan(10);
        // Next scan (1000ms): M1=1 resets T0 -> M0=0 -> Y0 (LED_PIN) turns back ON!
        vm.step_scan(10);
        assert!(vm.get_digital_output(0), "LED_PIN (Y0) should restart ON phase at 1000ms");
        assert!(!vm.get_digital_output(1), "LED_ALT (Y1) should return OFF at 1000ms");
    }

    #[test]
    fn test_vm_serial_run_and_stop_commands() {
        let mut vm = MicroVm::new();
        // Simple program: LDN X1 -> ST Y0 -> END
        let raw: [u8; 12] = [
            0x02, 0x01, 0x00, 0x00, // LDN X1
            0x07, 0x20, 0x00, 0x00, // ST Y0
            0xFF, 0x00, 0x00, 0x00, // END
        ];
        vm.load_bytecode(&raw).unwrap();
        vm.step_scan(1);
        assert!(vm.running);
        assert!(vm.get_digital_output(0), "Y0 should be ON while running");

        // Build STOP packet: SOF=0xAA, CMD=0x02, LEN=1 (0x01, 0x00), PAYLOAD=[0x00], CRC8
        let mut stop_pkt = [0xAA, 0x02, 0x01, 0x00, 0x00, 0x00];
        stop_pkt[5] = MicroVm::compute_crc8(&stop_pkt[1..5]);
        assert_eq!(vm.process_serial_packet(&stop_pkt), Ok(CMD_RUN_STOP));
        assert!(!vm.running, "VM should be in STOP state");
        assert!(!vm.get_digital_output(0), "Y0 should be cleared immediately on STOP");

        // Build RUN packet: SOF=0xAA, CMD=0x02, LEN=1 (0x01, 0x00), PAYLOAD=[0x01], CRC8
        let mut run_pkt = [0xAA, 0x02, 0x01, 0x00, 0x01, 0x00];
        run_pkt[5] = MicroVm::compute_crc8(&run_pkt[1..5]);
        assert_eq!(vm.process_serial_packet(&run_pkt), Ok(CMD_RUN_STOP));
        assert!(vm.running, "VM should return to RUN state");
        vm.step_scan(1);
        assert!(vm.get_digital_output(0), "Y0 should be ON again after resuming RUN");

        // Build ENTER_BOOTSEL packet: SOF=0xAA, CMD=0x0A, LEN=0, CRC8
        let mut bootsel_pkt = [0xAA, CMD_ENTER_BOOTSEL, 0x00, 0x00, 0x00];
        bootsel_pkt[4] = MicroVm::compute_crc8(&bootsel_pkt[1..4]);
        assert_eq!(vm.process_serial_packet(&bootsel_pkt), Ok(CMD_ENTER_BOOTSEL));
        assert!(!vm.running, "VM should safely stop before entering BOOTSEL");
        assert!(!vm.get_digital_output(0), "Outputs should be turned OFF before entering BOOTSEL");
    }

    #[test]
    fn test_vm_status_snapshot_packet() {
        let mut vm = MicroVm::new();
        vm.running = true;
        vm.scan_count = 1420;
        vm.set_digital_input(0, true);  // X0
        vm.bit_mem[0x20] = 1;          // Y0
        vm.bit_mem[0x40] = 1;          // M0
        vm.bit_mem[0xC0] = 1;          // T0 Done
        vm.word_mem[0xC0] = 500;       // T0 CV = 500ms
        vm.acc_bool = true;
        vm.acc_word = 42;

        let mut buf = [0u8; STATUS_PACKET_LEN];
        let bytes_written = vm.write_status_packet(&mut buf).unwrap();
        assert_eq!(bytes_written, STATUS_PACKET_LEN);
        assert_eq!(buf[0], PACKET_SOF);
        assert_eq!(buf[1], CMD_STATUS_RESPONSE); // 0x83
        let len = u16::from_le_bytes([buf[2], buf[3]]) as usize;
        assert_eq!(len, STATUS_SNAPSHOT_PAYLOAD_LEN);
        assert_eq!(buf[4], 1); // running = true
        let scan = u32::from_le_bytes([buf[5], buf[6], buf[7], buf[8]]);
        assert_eq!(scan, 1420);
        assert_eq!(buf[9] & 0x01, 1); // acc_bool = true (bit0)
        assert_eq!(buf[9] >> 4, FW_FEATURE_LEVEL); // feature level in high nibble
        let acc_w = i32::from_le_bytes([buf[10], buf[11], buf[12], buf[13]]);
        assert_eq!(acc_w, 42);
        // Verify bit_mem
        assert_eq!(buf[14], 1); // X0
        assert_eq!(buf[14 + 0x20], 1); // Y0
        assert_eq!(buf[14 + 0x40], 1); // M0
        assert_eq!(buf[14 + 0xC0], 1); // T0 bit
        // Verify CRC8 matches computed CRC8
        let expected_crc = buf[STATUS_PACKET_LEN - 1];
        let actual_crc = MicroVm::compute_crc8(&buf[1..STATUS_PACKET_LEN - 1]);
        assert_eq!(expected_crc, actual_crc);
    }

    #[test]
    fn test_vm_upload_program_packet() {
        let mut vm = MicroVm::new();
        let raw: [u8; 16] = [
            0x01, 0x00, 0x00, 0x00, // LD X0
            0x03, 0x01, 0x00, 0x00, // AND X1
            0x07, 0x20, 0x00, 0x00, // ST Y0
            0xFF, 0x00, 0x00, 0x00, // END
        ];
        vm.load_bytecode(&raw).unwrap();

        // 1. Process Upload Request: SOF=0xAA, CMD=0x04, LEN=0, CRC8
        let mut req = [PACKET_SOF, CMD_UPLOAD_PROGRAM, 0x00, 0x00, 0x00];
        req[4] = MicroVm::compute_crc8(&req[1..4]);
        assert_eq!(vm.process_serial_packet(&req), Ok(CMD_UPLOAD_PROGRAM));

        // 2. Write Program Packet (Response 0x84)
        let mut tx_buf = [0u8; 64];
        let written = vm.write_program_packet(&mut tx_buf).unwrap();
        assert_eq!(written, 5 + 16); // 21 bytes
        assert_eq!(tx_buf[0], PACKET_SOF);
        assert_eq!(tx_buf[1], CMD_UPLOAD_PROGRAM_RESPONSE);
        assert_eq!(u16::from_le_bytes([tx_buf[2], tx_buf[3]]), 16);
        assert_eq!(&tx_buf[4..20], &raw);
        assert_eq!(tx_buf[20], MicroVm::compute_crc8(&tx_buf[1..20]));
    }

    #[test]
    fn test_modbus_rtu_framing_and_crc() {
        // Standard Modbus test vector: Read 10 holding registers from addr 0, slave ID 1
        let req_job = ModbusMasterJob {
            job_id: 1,
            instance_id: 0,
            slave_id: 1,
            function_code: MB_FC_READ_HOLDING_REGISTERS,
            reg_addr: 0x0000,
            count: 10,
            buffer_addr: 0x20, // D0
            is_write: false,
        };
        let word_mem = [0i32; 256];
        let bit_mem = [0u8; 256];
        let mut frame_buf = [0u8; 32];
        let frame_len = encode_rtu_request(&req_job, &word_mem, &bit_mem, &mut frame_buf).unwrap();

        assert_eq!(frame_len, 8);
        assert_eq!(&frame_buf[..6], &[0x01, 0x03, 0x00, 0x00, 0x00, 0x0A]);
        // Standard Modbus CRC for [01 03 00 00 00 0A] is 0xC5CD (Little Endian: CD, C5)
        let crc = compute_crc16(&frame_buf[..6]);
        assert_eq!(u16::from_le_bytes([frame_buf[6], frame_buf[7]]), crc);

        // Decode simulated response: 2 registers (D0=1234, D1=5678)
        let mut sim_resp = [
            0x01, 0x03, 0x04,
            0x04, 0xD2, // 1234
            0x16, 0x2E, // 5678
            0x00, 0x00, // CRC placeholder
        ];
        let resp_crc = compute_crc16(&sim_resp[..7]);
        let [rc_lo, rc_hi] = resp_crc.to_le_bytes();
        sim_resp[7] = rc_lo;
        sim_resp[8] = rc_hi;

        let mut read_job = req_job;
        read_job.count = 2;
        let mut target_words = [0i32; 256];
        let mut target_bits = [0u8; 256];

        let decode_res = decode_rtu_response(&read_job, &sim_resp, &mut target_words, &mut target_bits);
        assert!(decode_res.is_ok());
        assert_eq!(target_words[0x20], 1234); // D0
        assert_eq!(target_words[0x21], 5678); // D1
    }

    #[test]
    fn test_vm_mb_master_execution() {
        let mut vm = MicroVm::new();
        // Bytecode:
        // 0: LD X0 (0x01, addr 0)
        // 1: MB_MASTER inst 0, slave 1, func 3 (0x60, addr 0, imm 0x0301)
        // 2: MB_PARAM1 buffer D0 (0x20), reg_addr 100 (0x61, addr 0x20, imm 100)
        // 3: MB_PARAM2 flags 0, count 2 (0x62, addr 0, imm 2)
        // 4: MB_STATUS inst 0, check BUSY (0x63, addr 0, imm 1)
        // 5: ST M1 (0x07, addr 0x41)
        // 6: END (0xFF)
        let prog = [
            VmInstruction { op: 0x01, addr: 0x00, imm: 0 },
            VmInstruction { op: 0x60, addr: 0x00, imm: 0x0301 },
            VmInstruction { op: 0x61, addr: 0x20, imm: 100 },
            VmInstruction { op: 0x62, addr: 0x00, imm: 2 },
            VmInstruction { op: 0x63, addr: 0x00, imm: 1 },
            VmInstruction { op: 0x07, addr: 0x41, imm: 0 },
            VmInstruction { op: 0xFF, addr: 0x00, imm: 0 },
        ];
        for (i, ins) in prog.iter().enumerate() {
            vm.program[i] = *ins;
        }
        vm.prog_len = prog.len() as u16;

        // Scan 1: X0 is OFF -> No trigger
        vm.step_scan(10);
        assert!(!vm.mb_master[0].busy);
        assert!(vm.mb_out_job.is_none());

        // Scan 2: X0 turns ON -> Rising edge triggers MB_MASTER
        vm.set_digital_input(0, true);
        vm.step_scan(10);
        assert!(vm.mb_master[0].busy);
        assert!(vm.bit_mem[0x41] != 0, "M1 (BUSY status) should be ON");
        assert!(vm.mb_out_job.is_some());
        let job = vm.mb_out_job.take().unwrap();
        assert_eq!(job.slave_id, 1);
        assert_eq!(job.function_code, 3);
        assert_eq!(job.reg_addr, 100);
        assert_eq!(job.count, 2);
        assert_eq!(job.buffer_addr, 0x20);

        // Scan 3: Still waiting for response -> busy remains true
        vm.step_scan(10);
        assert!(vm.mb_master[0].busy);

        // Response arrives from background task
        vm.word_mem[0x20] = 999;
        vm.word_mem[0x21] = 888;
        vm.mb_in_result = Some(ModbusJobResult {
            job_id: job.job_id,
            instance_id: 0,
            success: true,
            error_code: MB_ERR_OK,
        });

        // Scan 4: Consumes result -> BUSY turns OFF, DONE turns ON (1-scan pulse)
        vm.step_scan(10);
        assert!(!vm.mb_master[0].busy);
        assert!(vm.mb_master[0].done);
        assert_eq!(vm.mb_master[0].status, MB_ERR_OK);

        // Scan 5: Next scan -> DONE turns OFF automatically
        vm.step_scan(10);
        assert!(!vm.mb_master[0].done);
        assert_eq!(vm.word_mem[0x20], 999);
        assert_eq!(vm.word_mem[0x21], 888);
    }

    #[test]
    fn test_vm_force_io_and_write_value() {
        let mut vm = MicroVm::new();
        // Simple program: LD X0; ST Y0
        let prog: [u8; 8] = [
            0x01, 0x00, 0x00, 0x00, // LD X0
            0x07, 0x20, 0x00, 0x00, // ST Y0
        ];
        vm.load_bytecode(&prog).unwrap();

        // Normal execution: X0=0 -> Y0=0
        vm.set_digital_input(0, false);
        vm.step_scan(10);
        assert!(!vm.get_digital_output(0));

        // 1. Force X0 ON: even though physical input is set to false, force overrides to true
        vm.force_bit(0, true);
        assert_eq!(vm.is_bit_forced(0), Some(true));
        vm.set_digital_input(0, false);
        vm.step_scan(10);
        assert!(vm.get_digital_output(0), "Y0 should be ON because X0 was forced ON");

        // 2. Force Y0 OFF: even though ladder logic evaluates to ON, forced output overrides to false
        vm.force_bit(0x20, false);
        assert_eq!(vm.is_bit_forced(0x20), Some(false));
        vm.step_scan(10);
        assert!(!vm.get_digital_output(0), "Y0 should be OFF because Y0 was forced OFF");

        // 3. Packet protocol test: Unforce Y0 via CMD_FORCE_IO packet [SOF, 0x06, 0x02, 0x00, ADDR, ACTION, CRC]
        let mut unforce_pkt = [PACKET_SOF, CMD_FORCE_IO, 0x02, 0x00, 0x20, 0x00, 0x00];
        unforce_pkt[6] = MicroVm::compute_crc8(&unforce_pkt[1..6]);
        assert_eq!(vm.process_serial_packet(&unforce_pkt), Ok(CMD_FORCE_IO));
        assert_eq!(vm.is_bit_forced(0x20), None);
        vm.step_scan(10);
        assert!(vm.get_digital_output(0), "Y0 should be ON again after unforcing");

        // 4. CMD_WRITE_VALUE: write to %MW5 (word_mem[0x25])
        let val: i32 = 12345;
        let [b0, b1, b2, b3] = val.to_le_bytes();
        let mut write_pkt = [PACKET_SOF, CMD_WRITE_VALUE, 0x06, 0x00, 1u8, 0x25, b0, b1, b2, b3, 0x00];
        write_pkt[10] = MicroVm::compute_crc8(&write_pkt[1..10]);
        assert_eq!(vm.process_serial_packet(&write_pkt), Ok(CMD_WRITE_VALUE));
        assert_eq!(vm.word_mem[0x25], 12345);

        // 5. Clear all forces
        vm.clear_all_forces();
        assert_eq!(vm.is_bit_forced(0), None);
    }

    // ---------------- Feature level 2 tests ----------------

    const fn ins(op: u8, addr: u8, imm: u16) -> VmInstruction {
        VmInstruction { op, addr, imm }
    }
    const fn w(addr: u8) -> VmInstruction {
        ins(OP_OPND, OPND_KIND_WORD, addr as u16)
    }
    const fn k(v: i16) -> VmInstruction {
        ins(OP_OPND, OPND_KIND_CONST, v as u16)
    }
    const fn b(addr: u8) -> VmInstruction {
        ins(OP_OPND, OPND_KIND_BIT, addr as u16)
    }

    fn load(vm: &mut MicroVm, prog: &[VmInstruction]) {
        let mut raw = [0u8; MAX_INSTRUCTIONS * 4];
        for (i, p) in prog.iter().enumerate() {
            raw[i * 4..i * 4 + 4].copy_from_slice(&p.to_le_bytes());
        }
        vm.load_bytecode(&raw[..prog.len() * 4]).unwrap();
    }

    #[test]
    fn test_vm_movw_bitwise() {
        let mut vm = MicroVm::new();
        // LD X0; MOVW D0 <- AI0; WAND D1 = D0 & 0x00FF; WOR D2 = D1 | 0x100; WXOR D3 = D2 ^ D0;
        // WNOT D4 = !D1; SHL D5 = D1 << 4 (level); SHR D6 = D0 >> K2; ST Y0
        let prog = [
            ins(0x01, 0x00, 0),
            ins(OP_MOVW, 0x20, 0), w(0x00),
            ins(OP_WAND, 0x21, 0), w(0x20), k(0x00FF),
            ins(OP_WOR, 0x22, 0), w(0x21), k(0x100),
            ins(OP_WXOR, 0x23, 0), w(0x22), w(0x20),
            ins(OP_WNOT, 0x24, 0), w(0x21), k(0),
            ins(OP_SHL, 0x25, 0), w(0x21), k(4),
            ins(OP_SHR, 0x26, 0), w(0x20), k(2),
            ins(0x07, 0x20, 0),
            ins(0xFF, 0, 0),
        ];
        load(&mut vm, &prog);
        vm.word_mem[0x00] = 0x1234;

        vm.step_scan(10);
        assert_eq!(vm.word_mem[0x20], 0, "EN=0 -> no MOVW");
        assert!(!vm.get_digital_output(0));

        vm.set_digital_input(0, true);
        vm.step_scan(10);
        assert_eq!(vm.word_mem[0x20], 0x1234);
        assert_eq!(vm.word_mem[0x21], 0x34);
        assert_eq!(vm.word_mem[0x22], 0x134);
        assert_eq!(vm.word_mem[0x23], 0x134 ^ 0x1234);
        assert_eq!(vm.word_mem[0x24], !0x34);
        assert_eq!(vm.word_mem[0x25], 0x340);
        assert_eq!(vm.word_mem[0x26], 0x1234 >> 2);
        assert!(vm.get_digital_output(0), "ENO passes power through word ops");
    }

    #[test]
    fn test_vm_opnd_negative_const() {
        let mut vm = MicroVm::new();
        let prog = [
            ins(0x01, 0x00, 0),
            ins(OP_MOVW, 0x20, 0), k(-5),
            ins(OP_LIMIT, 0x21, 0), k(-32768), k(-100), k(100),
            ins(0xFF, 0, 0),
        ];
        load(&mut vm, &prog);
        vm.set_digital_input(0, true);
        vm.step_scan(10);
        assert_eq!(vm.word_mem[0x20], -5);
        assert_eq!(vm.word_mem[0x21], -100);
        assert_eq!(vm.read_opnd([OP_OPND, OPND_KIND_CONST, 0x00, 0x80]), -32768);
        assert_eq!(vm.read_opnd([OP_OPND, OPND_KIND_CONST, 0xFF, 0x7F]), 32767);
    }

    #[test]
    fn test_vm_scale_limit() {
        let mut vm = MicroVm::new();
        // LD X0; SCALE D20 <- AI0 [0..4095] -> [0..1000]; LIMIT D21 <- D20 [100..900]
        let prog = [
            ins(0x01, 0x00, 0),
            ins(OP_SCALE, 0x34, 0), w(0x00), k(0), k(4095), k(0), k(1000),
            ins(OP_LIMIT, 0x35, 0), w(0x34), k(100), k(900),
            ins(0xFF, 0, 0),
        ];
        load(&mut vm, &prog);
        vm.set_digital_input(0, true);
        for (raw, scaled, limited) in [(0, 0, 100), (2048, 500, 500), (4095, 1000, 900), (409, 99, 100)] {
            vm.word_mem[0x00] = raw;
            vm.step_scan(10);
            assert_eq!(vm.word_mem[0x34], scaled, "scale({})", raw);
            assert_eq!(vm.word_mem[0x35], limited, "limit({})", scaled);
        }
    }

    #[test]
    fn test_vm_pid_converges() {
        let mut vm = MicroVm::new();
        // LD X0; PID inst 0: CV=D12 PV=D10 SP=D11 Kp=1.00 Ki=2.00/s Kd=0.05s OutMax=1000
        let prog = [
            ins(0x01, 0x00, 0),
            ins(OP_PID, 0x2C, 0), w(0x2A), w(0x2B), k(100), k(200), k(5), k(1000),
            ins(0xFF, 0, 0),
        ];
        load(&mut vm, &prog);
        vm.set_digital_input(0, true);
        vm.word_mem[0x2B] = 500; // SP

        // First-order plant: tau = 0.5 s, gain 1.0, dt = 10 ms
        let mut pv = 0.0f64;
        for _ in 0..1500 {
            vm.word_mem[0x2A] = pv as i32;
            vm.step_scan(10);
            let cv = vm.word_mem[0x2C] as f64;
            assert!((0.0..=1000.0).contains(&cv), "CV must stay within 0..OutMax");
            pv += (cv - pv) * (10.0 / 500.0);
        }
        let err = (pv - 500.0).abs();
        assert!(err <= 10.0, "PV should converge to SP +/-2% (pv = {})", pv);

        // EN = 0 resets the integral
        vm.set_digital_input(0, false);
        vm.step_scan(10);
        assert_eq!(vm.pid[0].integral, 0);
        assert!(!vm.pid[0].active);
    }

    #[test]
    fn test_vm_shift_reg_edge() {
        let mut vm = MicroVm::new();
        // LD X0; SHREG D60 len 4 slot 0 <- D59; SHBIT D70 slot 1 <- X1; SHL D40 edge slot 2 by 1
        let prog = [
            ins(0x01, 0x00, 0),
            ins(OP_SHREG, 0x20 + 60, 4), w(0x20 + 59),
            ins(OP_SHBIT, 0x20 + 70, 1), b(0x01),
            ins(OP_SHL, 0x20 + 40, SHIFT_EDGE_FLAG | 2), w(0x20 + 40), k(1),
            ins(0xFF, 0, 0),
        ];
        load(&mut vm, &prog);
        vm.word_mem[0x20 + 40] = 1;
        let mut val = 10;
        for pulse in 0..5 {
            vm.word_mem[0x20 + 59] = val;
            vm.set_digital_input(1, pulse % 2 == 0);
            vm.set_digital_input(0, true);
            vm.step_scan(10);
            // Holding EN high must NOT shift again
            vm.step_scan(10);
            vm.step_scan(10);
            vm.set_digital_input(0, false);
            vm.step_scan(10);
            val += 10;
        }
        // 5 rising edges: inputs 10,20,30,40,50 -> D60..D63 = 50,40,30,20
        assert_eq!(&vm.word_mem[0x20 + 60..0x20 + 64], &[50, 40, 30, 20]);
        assert_eq!(vm.word_mem[0x20 + 64], 0, "shift register must not exceed its length");
        // Bits shifted in: 1,0,1,0,1 -> 0b10101
        assert_eq!(vm.word_mem[0x20 + 70], 0b10101);
        // 5 edges of SHL by 1 starting from 1 -> 32
        assert_eq!(vm.word_mem[0x20 + 40], 32);
    }

    #[test]
    fn test_vm_uart_tx_rx_slots() {
        let mut vm = MicroVm::new();
        // LD X0; UTX inst 1 mode 1 <- D80; ST Y0 (ENO); LD X1; URX D81; ST Y1
        let prog = [
            ins(0x01, 0x00, 0),
            ins(OP_UTX, 1, 1), w(0x20 + 80),
            ins(0x07, 0x20, 0),
            ins(0x01, 0x01, 0),
            ins(OP_URX, 0x20 + 81, 0),
            ins(0x07, 0x21, 0),
            ins(0xFF, 0, 0),
        ];
        load(&mut vm, &prog);
        vm.word_mem[0x20 + 80] = 42;

        vm.set_digital_input(0, true);
        vm.step_scan(10);
        let job = vm.uart_tx_out.take().expect("rising edge must queue a UART job");
        assert_eq!(job, UartTx { instance: 1, mode: 1, value: 42 });
        assert!(vm.uart_tx_busy);
        assert!(!vm.get_digital_output(0), "ENO stays OFF while busy");
        let mut buf = [0u8; 16];
        let n = job.encode(&mut buf);
        assert_eq!(&buf[..n], b"42\r\n");

        // Firmware finished sending
        vm.uart_tx_busy = false;
        vm.step_scan(10);
        assert!(vm.uart_tx_out.is_none(), "Held EN must not resend");
        assert!(vm.get_digital_output(0), "ENO = sent");

        // RX: no byte -> ENO off; byte -> stored + ENO for one scan
        vm.set_digital_input(1, true);
        vm.step_scan(10);
        assert!(!vm.get_digital_output(1));
        vm.uart_rx_in = Some(0x41);
        vm.step_scan(10);
        assert_eq!(vm.word_mem[0x20 + 81], 0x41);
        assert!(vm.get_digital_output(1));
        assert!(vm.uart_rx_in.is_none());
        vm.step_scan(10);
        assert!(!vm.get_digital_output(1));
    }

    #[test]
    fn test_vm_reset_memory_keep_retain() {
        let mut vm = MicroVm::new();
        vm.bit_mem[0x40] = 1; // M0 (volatile)
        vm.bit_mem[0x80] = 1; // M64 (retain)
        vm.word_mem[0x20] = 7; // D0 (volatile)
        vm.word_mem[0x80] = 99; // D96 (retain)
        vm.word_mem[0x9F] = -3; // D127 (retain)
        vm.word_mem[0xA0] = 5; // D128 / %SD0 (not retained)
        vm.reset_memory_keep_retain();
        assert_eq!(vm.bit_mem[0x40], 0);
        assert_eq!(vm.bit_mem[0x80], 1);
        assert_eq!(vm.word_mem[0x20], 0);
        assert_eq!(vm.word_mem[0x80], 99);
        assert_eq!(vm.word_mem[0x9F], -3);
        assert_eq!(vm.word_mem[0xA0], 0);
    }

    #[test]
    fn test_vm_runtime_io_config_packets() {
        let mut vm = MicroVm::new();
        assert_eq!(vm.io_config, HardwareIoConfig::default_rp2040());

        // 1. Test CMD_GET_IO_CONFIG request and response packet encoding
        let mut get_pkt = [PACKET_SOF, CMD_GET_IO_CONFIG, 0x00, 0x00, 0x00];
        get_pkt[4] = MicroVm::compute_crc8(&get_pkt[1..4]);
        assert_eq!(vm.process_serial_packet(&get_pkt), Ok(CMD_GET_IO_CONFIG));

        let mut resp_buf = [0u8; IO_CONFIG_PACKET_LEN];
        let written = vm.io_config.write_response_packet(&mut resp_buf).unwrap();
        assert_eq!(written, IO_CONFIG_PACKET_LEN);
        assert_eq!(resp_buf[0], PACKET_SOF);
        assert_eq!(resp_buf[1], CMD_GET_IO_CONFIG_RESPONSE);

        // 2. Test CMD_SET_IO_CONFIG with custom pin assignment + sanitization of reserved/duplicate pins
        let mut custom_cfg = HardwareIoConfig::default_rp2040();
        custom_cfg.input_pins[0] = 16; // X0 -> GP16
        custom_cfg.input_pins[4] = 17; // X4 -> GP17
        custom_cfg.output_pins[0] = 18; // Y0 -> GP18
        custom_cfg.output_pins[6] = 19; // Y6 -> GP19
        custom_cfg.set_input_active_low(false);
        custom_cfg.set_input_pull_up(false);

        let payload = custom_cfg.to_bytes();
        let mut set_pkt = [0u8; IO_CONFIG_PACKET_LEN];
        set_pkt[0] = PACKET_SOF;
        set_pkt[1] = CMD_SET_IO_CONFIG;
        set_pkt[2] = IO_CONFIG_PAYLOAD_LEN as u8;
        set_pkt[3] = 0x00;
        set_pkt[4..4 + IO_CONFIG_PAYLOAD_LEN].copy_from_slice(&payload);
        set_pkt[IO_CONFIG_PACKET_LEN - 1] =
            MicroVm::compute_crc8(&set_pkt[1..IO_CONFIG_PACKET_LEN - 1]);

        assert_eq!(vm.process_serial_packet(&set_pkt), Ok(CMD_SET_IO_CONFIG));
        assert_eq!(vm.io_config.input_pins[0], 16);
        assert_eq!(vm.io_config.input_pins[4], 17);
        assert_eq!(vm.io_config.output_pins[0], 18);
        assert_eq!(vm.io_config.output_pins[6], 19);
        assert!(!vm.io_config.input_active_low());
        assert!(!vm.io_config.input_pull_up());
    }
}


