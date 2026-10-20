use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::il_format::IlParser;
use crate::ladder::{parse_int_literal, CoilMode, ContactMode, IOPortMapping, LadderElement, LadderProgram, Rung, TypeTag};
use crate::simulation::bitwise_opcode;
use uplc_vm::{OPND_KIND_BIT, OPND_KIND_CONST, OPND_KIND_WORD, SHIFT_EDGE_FLAG};

/// Start of Frame byte for Serial Packet Protocol
pub const PACKET_SOF: u8 = 0xAA;

/// Serial Command IDs for RP2040 Micro-VM
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum PlcSerialCommand {
    /// 0x01: Download Bytecode Program to RAM / Flash
    DownloadProgram = 0x01,
    /// 0x02 (Payload = 0x01): Start / RUN PLC Scan Execution
    RunStopControl = 0x02,
    /// 0x03: Request Online Status Monitor (bit_mem & word_mem)
    MonitorStatus = 0x03,
    /// 0x04: Upload / Read Bytecode Program from PLC
    UploadProgram = 0x04,
    /// 0x05: Execute Single Scan Step on PLC
    StepPlc = 0x05,
    /// 0x06: Force Digital Bit (Input, Output, Relay) ON/OFF or Release
    ForceIo = 0x06,
    /// 0x07: Write Value into Word or Bit Memory
    WriteValue = 0x07,
    /// 0x08: Save the RETAIN area (`M64..M95`, `D96..D127`) to Flash now
    PersistSave = 0x08,
    /// 0x09: Clear the RETAIN area in RAM and Flash
    PersistClear = 0x09,
    /// 0x02 (Payload = 0x00): Stop / HALT PLC Scan Execution & Safe Output Shutdown
    StopPlc = 0x12,
}

impl PlcSerialCommand {
    /// Create CMD 0x06 packet: `[0xAA, 0x06, 0x02, 0x00, ADDR, ACTION, CRC8]`
    /// action: 0 = Unforce, 1 = Force ON, 2 = Force OFF, 0xFF = Clear All Forces
    pub fn create_force_packet(addr: u8, action: u8) -> Vec<u8> {
        let mut buf = Vec::with_capacity(7);
        buf.push(PACKET_SOF);
        buf.push(0x06); // CMD_FORCE_IO
        buf.push(0x02); // Len lo
        buf.push(0x00); // Len hi
        buf.push(addr);
        buf.push(action);
        let crc = BytecodeProgram::compute_crc8(&buf[1..6]);
        buf.push(crc);
        buf
    }

    /// Create CMD 0x07 packet: `[0xAA, 0x07, 0x06, 0x00, TARGET, ADDR, VAL (4B LE), CRC8]`
    /// target: 0 = Bit, 1 = Word
    pub fn create_write_value_packet(target_is_word: bool, addr: u8, val: i32) -> Vec<u8> {
        let mut buf = Vec::with_capacity(11);
        buf.push(PACKET_SOF);
        buf.push(0x07); // CMD_WRITE_VALUE
        buf.push(0x06); // Len lo
        buf.push(0x00); // Len hi
        buf.push(if target_is_word { 1 } else { 0 });
        buf.push(addr);
        buf.extend_from_slice(&val.to_le_bytes());
        let crc = BytecodeProgram::compute_crc8(&buf[1..10]);
        buf.push(crc);
        buf
    }
}

/// Online Monitoring Status Snapshot received from RP2040 Micro-VM or Local Simulator
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlcStatusSnapshot {
    pub running: bool,
    pub scan_count: u32,
    pub acc_bool: bool,
    pub acc_word: i32,
    pub bit_mem: [u8; 256],
    pub timer_cv: [u32; 16],
    pub counter_cv: [i32; 16],
    pub data_reg: [i32; 16],
    pub prog_len: u16,
    pub prog_crc: u16,
    /// Firmware / VM feature level (high nibble of the `acc_bool` byte).
    /// `0` = legacy firmware that does not report it (equivalent to level 1),
    /// `2` = supports OPND / PID / SCALE / LIMIT / Bitwise / Shift / UART opcodes.
    pub fw_feature_level: u8,
}

impl Default for PlcStatusSnapshot {
    fn default() -> Self {
        Self {
            running: false,
            scan_count: 0,
            acc_bool: false,
            acc_word: 0,
            bit_mem: [0; 256],
            timer_cv: [0; 16],
            counter_cv: [0; 16],
            data_reg: [0; 16],
            prog_len: 0,
            prog_crc: 0,
            fw_feature_level: 0,
        }
    }
}

impl PlcStatusSnapshot {
    /// `true` if the connected firmware can execute programs that require `level`
    /// (see [`BytecodeProgram::required_feature_level`]).
    pub fn firmware_supports(&self, level: u8) -> bool {
        self.fw_feature_level.max(1) >= level
    }

    pub const LEGACY_PAYLOAD_LEN: usize = 1 + 4 + 1 + 4 + 256 + 64 + 64 + 64; // 458 bytes
    pub const PAYLOAD_LEN: usize = Self::LEGACY_PAYLOAD_LEN + 2 + 2; // 462 bytes
    pub const PACKET_LEN: usize = 5 + Self::PAYLOAD_LEN; // 467 bytes

    /// Verify if the bytecode currently running on the PLC matches the compiled IDE BytecodeProgram
    pub fn matches_program(&self, bc: &BytecodeProgram) -> bool {
        let (ide_len, ide_crc) = bc.program_signature();
        self.prog_len == ide_len && self.prog_crc == ide_crc
    }

    /// Parse a complete response frame (`SOF 0xAA | CMD 0x83 | LEN | PAYLOAD | CRC8`)
    pub fn parse_from_packet(packet: &[u8]) -> Result<Self, &'static str> {
        if packet.len() < 5 + Self::LEGACY_PAYLOAD_LEN {
            return Err("Packet too short for status snapshot");
        }
        if packet[0] != PACKET_SOF {
            return Err("Invalid SOF byte");
        }
        if packet[1] != 0x83 {
            return Err("Unexpected command ID (expected 0x83)");
        }
        let len = u16::from_le_bytes([packet[2], packet[3]]) as usize;
        if len != Self::PAYLOAD_LEN && len != Self::LEGACY_PAYLOAD_LEN {
            return Err("Invalid payload length");
        }
        if packet.len() < 5 + len {
            return Err("Incomplete status snapshot packet");
        }
        let expected_crc = packet[4 + len];
        let actual_crc = BytecodeProgram::compute_crc8(&packet[1..4 + len]);
        if actual_crc != expected_crc {
            return Err("CRC8 checksum error");
        }

        let p = &packet[4..4 + len];
        let running = p[0] != 0;
        let scan_count = u32::from_le_bytes([p[1], p[2], p[3], p[4]]);
        let acc_bool = (p[5] & 0x01) != 0;
        let fw_feature_level = p[5] >> 4;
        let acc_word = i32::from_le_bytes([p[6], p[7], p[8], p[9]]);

        let mut bit_mem = [0u8; 256];
        bit_mem.copy_from_slice(&p[10..10 + 256]);

        let mut offset = 10 + 256;
        let mut timer_cv = [0u32; 16];
        for i in 0..16 {
            timer_cv[i] = u32::from_le_bytes([p[offset], p[offset + 1], p[offset + 2], p[offset + 3]]);
            offset += 4;
        }

        let mut counter_cv = [0i32; 16];
        for i in 0..16 {
            counter_cv[i] = i32::from_le_bytes([p[offset], p[offset + 1], p[offset + 2], p[offset + 3]]);
            offset += 4;
        }

        let mut data_reg = [0i32; 16];
        for i in 0..16 {
            data_reg[i] = i32::from_le_bytes([p[offset], p[offset + 1], p[offset + 2], p[offset + 3]]);
            offset += 4;
        }

        let (prog_len, prog_crc) = if len >= Self::PAYLOAD_LEN {
            let pl = u16::from_le_bytes([p[offset], p[offset + 1]]);
            let pc = u16::from_le_bytes([p[offset + 2], p[offset + 3]]);
            (pl, pc)
        } else {
            (0, 0)
        };

        Ok(Self {
            running,
            scan_count,
            acc_bool,
            acc_word,
            bit_mem,
            timer_cv,
            counter_cv,
            data_reg,
            prog_len,
            prog_crc,
            fw_feature_level,
        })
    }
}

/// 8-bit Opcodes for the Accumulator-based Micro-VM (RP2040)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum VmOpcode {
    Nop = 0x00,
    Ld = 0x01,
    Ldn = 0x02,
    And = 0x03,
    Andn = 0x04,
    Or = 0x05,
    Orn = 0x06,
    St = 0x07,
    Stn = 0x08,
    S = 0x09,
    R = 0x0A,
    Ldp = 0x0B,
    Ldf = 0x0C,
    Andp = 0x0D,
    Andf = 0x0E,
    Orp = 0x0F,
    AndLp = 0x10,
    OrLp = 0x11,
    Rp = 0x12,
    Orf = 0x13,
    Stp = 0x14,
    Stf = 0x15,
    RTrig = 0x16,
    FTrig = 0x17,
    Sr = 0x18,
    Rs = 0x19,
    Mc = 0x1A,
    Ton = 0x20,
    Ctu = 0x21,
    Tof = 0x22,
    Tp = 0x23,
    Ctd = 0x24,
    Ctud = 0x25,
    Eq = 0x30,
    Ne = 0x31,
    Gt = 0x32,
    Lt = 0x33,
    Ge = 0x34,
    Le = 0x35,
    Move = 0x40,
    Add = 0x41,
    Sub = 0x42,
    Mul = 0x43,
    Div = 0x44,
    Mod = 0x45,
    /// `MOVW dst, A` (+1 OPND)
    Movw = 0x46,
    /// `WAND dst, A, B` (+2 OPND)
    Wand = 0x47,
    Wor = 0x48,
    Wxor = 0x49,
    Wnot = 0x4A,
    /// `SHL dst, A, B` (+2 OPND); imm bit 15 = rising-edge trigger on slot `imm & 0xFF`
    Shl = 0x4B,
    Shr = 0x4C,
    Jmpc = 0x50,
    Lbl = 0x51,
    /// `SHREG base, imm = len | slot << 8` (+1 OPND: IN)
    Shreg = 0x52,
    /// `SHBIT word, imm = slot` (+1 OPND: BIT)
    Shbit = 0x53,
    MbMaster = 0x60,
    MbParam1 = 0x61,
    MbParam2 = 0x62,
    MbStatus = 0x63,
    /// Operand word `[0x6F, kind, imm_lo, imm_hi]` (kind 0 = word, 1 = const i16, 2 = bit)
    Opnd = 0x6F,
    /// `SCALE dst` (+5 OPND: IN, in_min, in_max, out_min, out_max)
    Scale = 0x70,
    /// `LIMIT dst` (+3 OPND: IN, MIN, MAX)
    Limit = 0x71,
    /// `PID dst(CV), imm = instance` (+6 OPND: PV, SP, Kp, Ki, Kd, OutMax)
    Pid = 0x72,
    /// `UTX instance, imm = mode` (+1 OPND: IN)
    Utx = 0x74,
    /// `URX dst`
    Urx = 0x75,
    End = 0xFF,
}

impl VmOpcode {
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            0x00 => Some(Self::Nop),
            0x01 => Some(Self::Ld),
            0x02 => Some(Self::Ldn),
            0x03 => Some(Self::And),
            0x04 => Some(Self::Andn),
            0x05 => Some(Self::Or),
            0x06 => Some(Self::Orn),
            0x07 => Some(Self::St),
            0x08 => Some(Self::Stn),
            0x09 => Some(Self::S),
            0x0A => Some(Self::R),
            0x0B => Some(Self::Ldp),
            0x0C => Some(Self::Ldf),
            0x0D => Some(Self::Andp),
            0x0E => Some(Self::Andf),
            0x0F => Some(Self::Orp),
            0x10 => Some(Self::AndLp),
            0x11 => Some(Self::OrLp),
            0x12 => Some(Self::Rp),
            0x13 => Some(Self::Orf),
            0x14 => Some(Self::Stp),
            0x15 => Some(Self::Stf),
            0x16 => Some(Self::RTrig),
            0x17 => Some(Self::FTrig),
            0x18 => Some(Self::Sr),
            0x19 => Some(Self::Rs),
            0x1A => Some(Self::Mc),
            0x20 => Some(Self::Ton),
            0x21 => Some(Self::Ctu),
            0x22 => Some(Self::Tof),
            0x23 => Some(Self::Tp),
            0x24 => Some(Self::Ctd),
            0x25 => Some(Self::Ctud),
            0x30 => Some(Self::Eq),
            0x31 => Some(Self::Ne),
            0x32 => Some(Self::Gt),
            0x33 => Some(Self::Lt),
            0x34 => Some(Self::Ge),
            0x35 => Some(Self::Le),
            0x40 => Some(Self::Move),
            0x41 => Some(Self::Add),
            0x42 => Some(Self::Sub),
            0x43 => Some(Self::Mul),
            0x44 => Some(Self::Div),
            0x45 => Some(Self::Mod),
            0x46 => Some(Self::Movw),
            0x47 => Some(Self::Wand),
            0x48 => Some(Self::Wor),
            0x49 => Some(Self::Wxor),
            0x4A => Some(Self::Wnot),
            0x4B => Some(Self::Shl),
            0x4C => Some(Self::Shr),
            0x50 => Some(Self::Jmpc),
            0x51 => Some(Self::Lbl),
            0x52 => Some(Self::Shreg),
            0x53 => Some(Self::Shbit),
            0x60 => Some(Self::MbMaster),
            0x61 => Some(Self::MbParam1),
            0x62 => Some(Self::MbParam2),
            0x63 => Some(Self::MbStatus),
            0x6F => Some(Self::Opnd),
            0x70 => Some(Self::Scale),
            0x71 => Some(Self::Limit),
            0x72 => Some(Self::Pid),
            0x74 => Some(Self::Utx),
            0x75 => Some(Self::Urx),
            0xFF => Some(Self::End),
            _ => None,
        }
    }

    pub fn mnemonic(&self) -> &'static str {
        match self {
            Self::Nop => "NOP",
            Self::Ld => "LD",
            Self::Ldn => "LDN",
            Self::And => "AND",
            Self::Andn => "ANDN",
            Self::Or => "OR",
            Self::Orn => "ORN",
            Self::St => "ST",
            Self::Stn => "STN",
            Self::S => "S",
            Self::R => "R",
            Self::Ldp => "LDP",
            Self::Ldf => "LDF",
            Self::Andp => "ANDP",
            Self::Andf => "ANDF",
            Self::Orp => "ORP",
            Self::AndLp => "AND(",
            Self::OrLp => "OR(",
            Self::Rp => ")",
            Self::Orf => "ORF",
            Self::Stp => "STP",
            Self::Stf => "STF",
            Self::RTrig => "R_TRIG",
            Self::FTrig => "F_TRIG",
            Self::Sr => "SR",
            Self::Rs => "RS",
            Self::Mc => "MC",
            Self::Ton => "TON",
            Self::Ctu => "CTU",
            Self::Tof => "TOF",
            Self::Tp => "TP",
            Self::Ctd => "CTD",
            Self::Ctud => "CTUD",
            Self::Eq => "EQ",
            Self::Ne => "NE",
            Self::Gt => "GT",
            Self::Lt => "LT",
            Self::Ge => "GE",
            Self::Le => "LE",
            Self::Move => "MOVE",
            Self::Add => "ADD",
            Self::Sub => "SUB",
            Self::Mul => "MUL",
            Self::Div => "DIV",
            Self::Mod => "MOD",
            Self::Movw => "MOVW",
            Self::Wand => "WAND",
            Self::Wor => "WOR",
            Self::Wxor => "WXOR",
            Self::Wnot => "WNOT",
            Self::Shl => "SHL",
            Self::Shr => "SHR",
            Self::Jmpc => "JMPC",
            Self::Lbl => "LBL",
            Self::Shreg => "SHREG",
            Self::Shbit => "SHBIT",
            Self::MbMaster => "MB_MASTER",
            Self::MbParam1 => "MB_PARAM1",
            Self::MbParam2 => "MB_PARAM2",
            Self::MbStatus => "MB_STATUS",
            Self::Opnd => "OPND",
            Self::Scale => "SCALE",
            Self::Limit => "LIMIT",
            Self::Pid => "PID",
            Self::Utx => "UTX",
            Self::Urx => "URX",
            Self::End => "END",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Nop => "No Operation",
            Self::Ld => "acc = bit_mem[addr]",
            Self::Ldn => "acc = !bit_mem[addr]",
            Self::And => "acc = acc & bit_mem[addr]",
            Self::Andn => "acc = acc & !bit_mem[addr]",
            Self::Or => "acc = acc | bit_mem[addr]",
            Self::Orn => "acc = acc | !bit_mem[addr]",
            Self::St => "bit_mem[addr] = acc",
            Self::Stn => "bit_mem[addr] = !acc",
            Self::S => "if (acc) bit_mem[addr] = 1",
            Self::R => "if (acc) bit_mem[addr] = 0",
            Self::Ldp => "acc = rising_edge(bit_mem[addr])",
            Self::Ldf => "acc = falling_edge(bit_mem[addr])",
            Self::Andp => "acc = acc & rising_edge(bit_mem[addr])",
            Self::Andf => "acc = acc & falling_edge(bit_mem[addr])",
            Self::Orp => "acc = acc | rising_edge(bit_mem[addr])",
            Self::AndLp => "push(AND, acc)",
            Self::OrLp => "push(OR, acc)",
            Self::Rp => "acc = pop() op acc",
            Self::Orf => "acc = acc | falling_edge(bit_mem[addr])",
            Self::Stp => "bit_mem[addr] = rising_edge(acc)",
            Self::Stf => "bit_mem[addr] = falling_edge(acc)",
            Self::RTrig => "acc = rising_edge(acc)",
            Self::FTrig => "acc = falling_edge(acc)",
            Self::Sr => "SR Bistable Latch (Set Dominant)",
            Self::Rs => "RS Bistable Latch (Reset Dominant)",
            Self::Mc => "Master Control Relay",
            Self::Ton => "Timer On-Delay (PT = imm ms)",
            Self::Ctu => "Counter Up (PV = imm)",
            Self::Tof => "Timer Off-Delay (PT = imm ms)",
            Self::Tp => "Pulse Timer (PT = imm ms)",
            Self::Ctd => "Counter Down (PV = imm)",
            Self::Ctud => "Up/Down Counter (PV = imm)",
            Self::Eq => "acc = acc & (word_mem[addr] == imm)",
            Self::Ne => "acc = acc & (word_mem[addr] != imm)",
            Self::Gt => "acc = acc & (word_mem[addr] > imm)",
            Self::Lt => "acc = acc & (word_mem[addr] < imm)",
            Self::Ge => "acc = acc & (word_mem[addr] >= imm)",
            Self::Le => "acc = acc & (word_mem[addr] <= imm)",
            Self::Move => "if (acc) word_mem[addr] = imm",
            Self::Add => "if (acc) word_mem[addr] += imm",
            Self::Sub => "if (acc) word_mem[addr] -= imm",
            Self::Mul => "if (acc) word_mem[addr] *= imm",
            Self::Div => "if (acc) word_mem[addr] /= imm",
            Self::Mod => "if (acc) word_mem[addr] %= imm",
            Self::Movw => "if (acc) word_mem[addr] = A",
            Self::Wand => "if (acc) word_mem[addr] = A & B",
            Self::Wor => "if (acc) word_mem[addr] = A | B",
            Self::Wxor => "if (acc) word_mem[addr] = A ^ B",
            Self::Wnot => "if (acc) word_mem[addr] = !A",
            Self::Shl => "word_mem[addr] = A << B (level, or rising edge if imm bit15)",
            Self::Shr => "word_mem[addr] = A >> B (level, or rising edge if imm bit15)",
            Self::Jmpc => "if (acc) pc = imm",
            Self::Lbl => "Jump Target Label",
            Self::Shreg => "Word shift register on rising edge (len = imm lo, slot = imm hi)",
            Self::Shbit => "word = (word << 1) | BIT on rising edge (slot = imm)",
            Self::Opnd => "Operand word (kind 0 = word, 1 = const i16, 2 = bit)",
            Self::Scale => "if (acc) word_mem[addr] = SCALE(IN, in_min, in_max, out_min, out_max)",
            Self::Limit => "if (acc) word_mem[addr] = LIMIT(IN, MIN, MAX)",
            Self::Pid => "PID controller (CV = word_mem[addr], instance = imm)",
            Self::Utx => "UART send on rising edge (instance = addr, mode = imm)",
            Self::Urx => "UART receive byte into word_mem[addr] (acc = received)",
            Self::MbMaster => "Modbus RTU Master Function Block (IEC 61131-3)",
            Self::MbParam1 => "Modbus Master Buffer & Register Address",
            Self::MbParam2 => "Modbus Master Mode Flags & Register Count",
            Self::MbStatus => "Query Modbus Function Block Status (DONE/BUSY/ERR/STATUS)",
            Self::End => "End of Scan Cycle (Return)",
        }
    }

    /// Number of `OPND` words following this (header) opcode
    pub fn operand_words(&self) -> usize {
        uplc_vm::opnd_word_count(*self as u8)
    }

    /// `true` for opcodes that need firmware feature level 2
    pub fn is_feature_level_2(&self) -> bool {
        (*self as u8) == 0x6F || self.operand_words() > 0 || *self == Self::Urx
    }
}

/// Warnings / errors produced while compiling a `LadderProgram` into bytecode
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompileReport {
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

impl CompileReport {
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn has_messages(&self) -> bool {
        !self.warnings.is_empty() || !self.errors.is_empty()
    }

    pub fn warn(&mut self, msg: impl Into<String>) {
        self.warnings.push(msg.into());
    }

    pub fn error(&mut self, msg: impl Into<String>) {
        self.errors.push(msg.into());
    }
}

/// Mutable state shared while compiling one program
#[derive(Default)]
struct CompileCtx {
    label_pcs: HashMap<String, u16>,
    pending_jumps: Vec<(usize, String)>,
    pid_instances: HashMap<String, u8>,
    uart_instances: HashMap<String, u8>,
    next_edge_slot: u16,
    report: CompileReport,
}

impl CompileCtx {
    fn alloc_edge_slot(&mut self, what: &str) -> u8 {
        let slot = self.next_edge_slot;
        if slot as usize >= uplc_vm::MAX_FB_EDGE_SLOTS {
            self.report.error(format!(
                "{}: too many edge-triggered blocks (max {})",
                what,
                uplc_vm::MAX_FB_EDGE_SLOTS
            ));
        }
        self.next_edge_slot = self.next_edge_slot.saturating_add(1);
        (slot % uplc_vm::MAX_FB_EDGE_SLOTS as u16) as u8
    }

    fn instance(map: &mut HashMap<String, u8>, report: &mut CompileReport, name: &str, kind: &str, max: usize) -> u8 {
        let key = if name.trim().is_empty() { kind.to_string() } else { name.trim().to_ascii_uppercase() };
        if let Some(&id) = map.get(&key) {
            return id;
        }
        let id = map.len();
        if id >= max {
            report.error(format!("{} '{}': more than {} {} instances are not supported", kind, name, max, kind));
        }
        let id = id.min(max - 1) as u8;
        map.insert(key, id);
        id
    }
}

/// Memory Space Region in Unified Memory Mapping
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemoryRegion {
    /// Bit 0x00..=0x1F: Digital Inputs X0..X31 (%IX0.0..%IX3.7)
    DigitalInputX,
    /// Bit 0x20..=0x3F: Digital Outputs Y0..Y31 (%QX0.0..%QX3.7)
    DigitalOutputY,
    /// Bit 0x40..=0xBF: Internal Relays M0..M127 (%MX0.0..%MX15.7)
    InternalRelayM,
    /// Bit & Word 0xC0..=0xDF: Timers T0..T31 (Done Bit in bit_mem, CV in word_mem)
    TimerT,
    /// Bit & Word 0xE0..=0xFF: Counters C0..C31 (Done Bit in bit_mem, CV in word_mem)
    CounterC,
    /// Word 0x00..=0x0F: Analog Inputs AI0..AI15 (%IW0..%IW15)
    AnalogInputAI,
    /// Word 0x10..=0x1F: Analog Outputs AO0..AO15 (%QW0..%QW15)
    AnalogOutputAO,
    /// Word 0x20..=0xBF: Data Registers D0..D159 (%MW0..%MW159)
    DataRegisterD,
    /// Instruction has no memory operand (e.g. AND(, OR(, ), END)
    None,
}

impl MemoryRegion {
    pub fn space_label(&self) -> &'static str {
        match self {
            Self::DigitalInputX => "bit_mem[0x00..0x1F] (Inputs X)",
            Self::DigitalOutputY => "bit_mem[0x20..0x3F] (Outputs Y)",
            Self::InternalRelayM => "bit_mem[0x40..0xBF] (Relays M)",
            Self::TimerT => "bit/word_mem[0xC0..0xDF] (Timers T)",
            Self::CounterC => "bit/word_mem[0xE0..0xFF] (Counters C)",
            Self::AnalogInputAI => "word_mem[0x00..0x0F] (Analog AI)",
            Self::AnalogOutputAO => "word_mem[0x10..0x1F] (Analog AO)",
            Self::DataRegisterD => "word_mem[0x20..0xBF] (Registers D)",
            Self::None => "-",
        }
    }
}

/// A single 32-bit (4-Byte) Fixed-Width VM Instruction
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BytecodeInstruction {
    pub pc: u16,
    pub op: VmOpcode,
    pub addr: u8,
    pub imm: u16,
    pub region: MemoryRegion,
    pub operand_name: String,
    pub symbol: String,
    pub il_source: String,
}

impl BytecodeInstruction {
    /// Encode this instruction into 4 raw bytes (Little-Endian immediate):
    /// `[op, addr, imm_lo, imm_hi]`
    pub fn to_bytes(&self) -> [u8; 4] {
        let [imm_lo, imm_hi] = self.imm.to_le_bytes();
        [self.op as u8, self.addr, imm_lo, imm_hi]
    }

    /// Format the 4 bytes as uppercase hex string e.g. `"01 00 00 00"`
    pub fn hex_bytes(&self) -> String {
        let b = self.to_bytes();
        format!("{:02X} {:02X} {:02X} {:02X}", b[0], b[1], b[2], b[3])
    }
}

/// Summary of an active variable mapped into the Unified Memory Space
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryMapEntry {
    pub addr: u8,
    pub canonical_name: String,
    pub iec_address: String,
    pub symbol: String,
    pub region: MemoryRegion,
    pub hw_pin: String,
}

/// Compiled Bytecode Program ready for RP2040 Micro-VM execution or Serial transmission
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BytecodeProgram {
    pub hardware: String,
    pub instructions: Vec<BytecodeInstruction>,
    pub memory_map: Vec<MemoryMapEntry>,
}

impl BytecodeProgram {
    /// Total raw bytecode payload size in bytes (`instructions.len() * 4`)
    pub fn payload_size_bytes(&self) -> usize {
        self.instructions.len() * 4
    }

    /// Minimum firmware feature level needed to run this program
    /// (`2` if any OPND-based opcode is used, otherwise `1`).
    pub fn required_feature_level(&self) -> u8 {
        if self.instructions.iter().any(|i| i.op.is_feature_level_2()) {
            2
        } else {
            1
        }
    }

    /// Serialize all instructions into raw 32-bit aligned bytecode bytes (`4 * N` bytes)
    pub fn to_raw_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.payload_size_bytes());
        for ins in &self.instructions {
            bytes.extend_from_slice(&ins.to_bytes());
        }
        bytes
    }

    /// Calculate CRC-8 (polynomial 0x07) over a slice of bytes
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

    /// Calculate CRC-16/CCITT-FALSE (polynomial 0x1021, init 0xFFFF) over raw bytecode bytes
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

    /// Return the deterministic Bytecode Program Signature `(prog_len: u16, prog_crc: u16)`
    pub fn program_signature(&self) -> (u16, u16) {
        let raw = self.to_raw_bytes();
        (self.instructions.len() as u16, Self::compute_crc16(&raw))
    }

    /// Build a complete Serial/UART Packet Frame:
    /// `[SOF (0xAA)] [CMD (1B)] [LEN_LO (1B)] [LEN_HI (1B)] [PAYLOAD] [CRC8 (1B)]`
    pub fn to_serial_packet(&self, cmd: PlcSerialCommand) -> Vec<u8> {
        let (wire_cmd, payload) = match cmd {
            PlcSerialCommand::DownloadProgram => (0x01u8, self.to_raw_bytes()),
            PlcSerialCommand::RunStopControl => (0x02u8, vec![0x01u8]),
            PlcSerialCommand::StopPlc => (0x02u8, vec![0x00u8]),
            PlcSerialCommand::MonitorStatus => (0x03u8, Vec::new()),
            PlcSerialCommand::UploadProgram => (0x04u8, Vec::new()),
            PlcSerialCommand::StepPlc => (0x05u8, Vec::new()),
            PlcSerialCommand::ForceIo => (0x06u8, Vec::new()),
            PlcSerialCommand::WriteValue => (0x07u8, Vec::new()),
            PlcSerialCommand::PersistSave => (0x08u8, Vec::new()),
            PlcSerialCommand::PersistClear => (0x09u8, Vec::new()),
        };
        let len = (payload.len() as u16).to_le_bytes();
        let mut packet = Vec::with_capacity(5 + payload.len());
        packet.push(PACKET_SOF);
        packet.push(wire_cmd);
        packet.push(len[0]);
        packet.push(len[1]);
        packet.extend_from_slice(&payload);
        // Compute CRC8 over CMD + LENGTH + PAYLOAD
        let crc = Self::compute_crc8(&packet[1..]);
        packet.push(crc);
        packet
    }

    /// Construct a complete `CMD_FORCE_IO (0x06)` Serial Packet Frame
    pub fn create_force_packet(addr: u8, action: u8) -> Vec<u8> {
        PlcSerialCommand::create_force_packet(addr, action)
    }

    /// Construct a complete `CMD_WRITE_VALUE (0x07)` Serial Packet Frame
    pub fn create_write_value_packet(target_is_word: bool, addr: u8, val: i32) -> Vec<u8> {
        PlcSerialCommand::create_write_value_packet(target_is_word, addr, val)
    }

    /// Decode raw 32-bit aligned bytecode payload bytes (`4 * N` bytes) into a `BytecodeProgram`
    pub fn from_raw_bytes(payload: &[u8], hardware: &str) -> Result<Self, String> {
        if payload.len() % 4 != 0 {
            return Err("Payload size must be a multiple of 4 bytes".to_string());
        }
        let count = payload.len() / 4;
        let mut instructions = Vec::with_capacity(count);
        let mut mem_table: HashMap<(u8, bool), MemoryMapEntry> = HashMap::new();

        for (i, chunk) in payload.chunks_exact(4).enumerate() {
            let op_byte = chunk[0];
            let addr = chunk[1];
            let imm = u16::from_le_bytes([chunk[2], chunk[3]]);

            let op = VmOpcode::from_u8(op_byte)
                .ok_or_else(|| format!("Unknown VM opcode 0x{:02X} at instruction {}", op_byte, i))?;

            let for_word = match op {
                VmOpcode::Eq
                | VmOpcode::Ne
                | VmOpcode::Gt
                | VmOpcode::Lt
                | VmOpcode::Ge
                | VmOpcode::Le
                | VmOpcode::Move
                | VmOpcode::Add
                | VmOpcode::Sub
                | VmOpcode::Mul
                | VmOpcode::Div
                | VmOpcode::Mod
                | VmOpcode::Movw
                | VmOpcode::Wand
                | VmOpcode::Wor
                | VmOpcode::Wxor
                | VmOpcode::Wnot
                | VmOpcode::Shl
                | VmOpcode::Shr
                | VmOpcode::Shreg
                | VmOpcode::Shbit
                | VmOpcode::Scale
                | VmOpcode::Limit
                | VmOpcode::Pid
                | VmOpcode::Urx => true,
                _ => false,
            };

            // OPND words describe their target in `imm` (addr = kind); UTX addr = UART instance
            let (canonical, iec_addr, region, mem_addr, mem_word) = match op {
                VmOpcode::Opnd => match addr {
                    OPND_KIND_WORD | OPND_KIND_BIT => {
                        let is_word = addr == OPND_KIND_WORD;
                        let (_, iec, reg) = BytecodeCompiler::format_address(imm as u8, is_word);
                        (BytecodeCompiler::opnd_text(addr, imm), iec, reg, imm as u8, is_word)
                    }
                    _ => (BytecodeCompiler::opnd_text(addr, imm), String::new(), MemoryRegion::None, 0, false),
                },
                VmOpcode::Utx => (format!("U{}", addr), String::new(), MemoryRegion::None, addr, false),
                _ => {
                    let (c, i, r) = BytecodeCompiler::format_address(addr, for_word);
                    (c, i, r, addr, for_word)
                }
            };

            let il_source = match op {
                VmOpcode::Nop => "NOP".to_string(),
                VmOpcode::End => "END".to_string(),
                VmOpcode::AndLp => "AND(".to_string(),
                VmOpcode::OrLp => "OR(".to_string(),
                VmOpcode::Rp => ")".to_string(),
                VmOpcode::Ton => format!("TON {}, {}", canonical, imm),
                VmOpcode::Tof => format!("TOF {}, {}", canonical, imm),
                VmOpcode::Tp => format!("TP {}, {}", canonical, imm),
                VmOpcode::Ctu => format!("CTU {}, {}", canonical, imm),
                VmOpcode::Ctd => format!("CD {}, {}", canonical, imm),
                VmOpcode::Ctud => format!("CTUD {}, {}", canonical, imm),
                VmOpcode::Eq
                | VmOpcode::Ne
                | VmOpcode::Gt
                | VmOpcode::Lt
                | VmOpcode::Ge
                | VmOpcode::Le
                | VmOpcode::Move
                | VmOpcode::Add
                | VmOpcode::Sub
                | VmOpcode::Mul
                | VmOpcode::Div
                | VmOpcode::Mod => format!("{} {} {}", op.mnemonic(), canonical, imm),
                VmOpcode::Jmpc => format!("JMPC {}", imm),
                VmOpcode::Lbl => format!("LBL {}", addr),
                VmOpcode::MbMaster => {
                    let slave = imm & 0xFF;
                    let func = (imm >> 8) & 0xFF;
                    format!("MB_MASTER MB{}, Slave={}, Func={}", addr, slave, func)
                }
                VmOpcode::MbParam1 => format!("MB_PARAM1 Buf={}, Addr={}", canonical, imm),
                VmOpcode::MbParam2 => format!("MB_PARAM2 Flags={}, Count={}", addr, imm),
                VmOpcode::MbStatus => format!("MB_STATUS MB{}, Flag={}", addr, imm),
                VmOpcode::Opnd => format!("  OPND {}", canonical),
                VmOpcode::Shreg => format!("SHREG {}, LEN={}, SLOT={}", canonical, imm & 0xFF, imm >> 8),
                VmOpcode::Shbit => format!("SHBIT {}, SLOT={}", canonical, imm & 0xFF),
                VmOpcode::Shl | VmOpcode::Shr if imm & SHIFT_EDGE_FLAG != 0 => {
                    format!("{} {}, EDGE SLOT={}", op.mnemonic(), canonical, imm & 0xFF)
                }
                VmOpcode::Pid => format!("PID {}, INST={}", canonical, imm),
                VmOpcode::Utx => format!("UTX U{}, MODE={}", addr, imm),
                _ => format!("{} {}", op.mnemonic(), canonical),
            };

            if region != MemoryRegion::None {
                mem_table.entry((mem_addr, mem_word)).or_insert_with(|| MemoryMapEntry {
                    addr: mem_addr,
                    canonical_name: canonical.clone(),
                    iec_address: iec_addr,
                    symbol: String::new(),
                    region,
                    hw_pin: String::new(),
                });
            }

            instructions.push(BytecodeInstruction {
                pc: i as u16,
                op,
                addr,
                imm,
                region,
                operand_name: canonical,
                symbol: String::new(),
                il_source,
            });
        }

        let mut memory_map: Vec<MemoryMapEntry> = mem_table.into_values().collect();
        memory_map.sort_by_key(|e| (BytecodeCompiler::region_sort_order(e.region), e.addr));

        Ok(Self {
            hardware: hardware.to_string(),
            instructions,
            memory_map,
        })
    }

    /// Parse a complete response frame (`SOF 0xAA | CMD 0x84 | LEN | PAYLOAD | CRC8`)
    pub fn parse_upload_packet(packet: &[u8], hardware: &str) -> Result<Self, String> {
        if packet.len() < 5 {
            return Err("Packet too short (minimum 5 bytes)".to_string());
        }
        if packet[0] != PACKET_SOF {
            return Err("Invalid SOF byte (expected 0xAA)".to_string());
        }
        if packet[1] != 0x84 {
            return Err(format!(
                "Unexpected command ID 0x{:02X} (expected 0x84)",
                packet[1]
            ));
        }
        let payload_len = u16::from_le_bytes([packet[2], packet[3]]) as usize;
        if packet.len() < 5 + payload_len {
            return Err(format!(
                "Incomplete upload packet frame: expected {} bytes, got {}",
                5 + payload_len,
                packet.len()
            ));
        }
        let expected_crc = packet[4 + payload_len];
        let actual_crc = Self::compute_crc8(&packet[1..4 + payload_len]);
        if actual_crc != expected_crc {
            return Err(format!(
                "CRC8 checksum error: computed 0x{:02X}, received 0x{:02X}",
                actual_crc, expected_crc
            ));
        }

        Self::from_raw_bytes(&packet[4..4 + payload_len], hardware)
    }

    /// Decompile the bytecode instructions back into IEC 61131-3 Instruction List (IL) standard text
    pub fn decompile_to_il(&self) -> String {
        let mut out = String::new();
        out.push_str("(* ==================================================================== *)\n");
        out.push_str("(* IEC 61131-3 IL Program (Decompiled from PLC Micro-VM Bytecode)        *)\n");
        out.push_str(&format!("(* Target Hardware: {:<51} *)\n", self.hardware));
        out.push_str("(* Generated by LaduinoPLC Bytecode Decompiler                         *)\n");
        out.push_str("(* ==================================================================== *)\n\n");
        out.push_str("PROGRAM MainProgram\n");

        // Emit VAR table from memory_map
        if !self.memory_map.is_empty() {
            out.push_str("VAR\n");
            for m in &self.memory_map {
                match m.region {
                    MemoryRegion::DigitalInputX => {
                        out.push_str(&format!("    {} AT {} : BOOL;\n", m.canonical_name, m.iec_address));
                    }
                    MemoryRegion::DigitalOutputY => {
                        out.push_str(&format!("    {} AT {} : BOOL;\n", m.canonical_name, m.iec_address));
                    }
                    MemoryRegion::InternalRelayM => {
                        out.push_str(&format!("    {} AT {} : BOOL;\n", m.canonical_name, m.iec_address));
                    }
                    MemoryRegion::TimerT => {
                        out.push_str(&format!("    {} : TON;\n", m.canonical_name));
                    }
                    MemoryRegion::CounterC => {
                        out.push_str(&format!("    {} : CTU;\n", m.canonical_name));
                    }
                    MemoryRegion::DataRegisterD => {
                        out.push_str(&format!("    {} AT {} : INT;\n", m.canonical_name, m.iec_address));
                    }
                    _ => {}
                }
            }
            out.push_str("END_VAR\n\n");
        }

        // Decompile instructions into Networks / Rungs
        let mut network_idx = 1;
        let mut need_network_header = true;
        let mut paren_depth: usize = 0;

        let mut scale_idx = 0usize;
        let mut limit_idx = 0usize;
        let mut urx_idx = 0usize;
        let mut idx = 0usize;
        while idx < self.instructions.len() {
            let ins = &self.instructions[idx];
            idx += 1;
            if ins.op == VmOpcode::End {
                break;
            }
            // Operand words following a header opcode (rendered as IL operand text)
            let n_ops = ins.op.operand_words();
            let ops: Vec<String> = (0..n_ops)
                .map(|j| match self.instructions.get(idx + j) {
                    Some(o) if o.op == VmOpcode::Opnd => BytecodeCompiler::opnd_text(o.addr, o.imm),
                    _ => "0".to_string(),
                })
                .collect();
            let op_is_const = |j: usize| {
                self.instructions
                    .get(idx + j)
                    .map(|o| o.op == VmOpcode::Opnd && o.addr == OPND_KIND_CONST)
                    .unwrap_or(true)
            };
            let op_word_addr = |j: usize| {
                self.instructions
                    .get(idx + j)
                    .filter(|o| o.op == VmOpcode::Opnd && o.addr == OPND_KIND_WORD)
                    .map(|o| o.imm as u8)
            };
            let dst = BytecodeCompiler::word_operand_text(ins.addr);

            let is_lead_load = matches!(
                ins.op,
                VmOpcode::Ld | VmOpcode::Ldn | VmOpcode::Ldp | VmOpcode::Ldf
            ) && paren_depth == 0;

            if (is_lead_load || ins.op == VmOpcode::Lbl) && !need_network_header {
                out.push('\n');
                need_network_header = true;
            }

            if need_network_header {
                out.push_str(&format!("(* Network {:02}: Auto-generated from PLC *)\n", network_idx));
                network_idx += 1;
                need_network_header = false;
            }

            let indent = if paren_depth > 0 && ins.op != VmOpcode::Rp && ins.op != VmOpcode::AndLp && ins.op != VmOpcode::OrLp {
                "        "
            } else {
                "    "
            };

            match ins.op {
                VmOpcode::Ld => out.push_str(&format!("{}LD   {}\n", indent, ins.operand_name)),
                VmOpcode::Ldn => out.push_str(&format!("{}LDN  {}\n", indent, ins.operand_name)),
                VmOpcode::Ldp => out.push_str(&format!("{}LDP  {}\n", indent, ins.operand_name)),
                VmOpcode::Ldf => out.push_str(&format!("{}LDF  {}\n", indent, ins.operand_name)),
                VmOpcode::And => out.push_str(&format!("{}AND  {}\n", indent, ins.operand_name)),
                VmOpcode::Andn => out.push_str(&format!("{}ANDN {}\n", indent, ins.operand_name)),
                VmOpcode::Andp => out.push_str(&format!("{}ANDP {}\n", indent, ins.operand_name)),
                VmOpcode::Andf => out.push_str(&format!("{}ANDF {}\n", indent, ins.operand_name)),
                VmOpcode::Or => out.push_str(&format!("{}OR   {}\n", indent, ins.operand_name)),
                VmOpcode::Orn => out.push_str(&format!("{}ORN  {}\n", indent, ins.operand_name)),
                VmOpcode::Orp => out.push_str(&format!("{}ORP  {}\n", indent, ins.operand_name)),
                VmOpcode::Orf => out.push_str(&format!("{}ORF  {}\n", indent, ins.operand_name)),
                VmOpcode::OrLp => {
                    out.push_str("    OR(\n");
                    paren_depth += 1;
                }
                VmOpcode::AndLp => {
                    out.push_str("    AND(\n");
                    paren_depth += 1;
                }
                VmOpcode::Rp => {
                    paren_depth = paren_depth.saturating_sub(1);
                    out.push_str("    )\n");
                }
                VmOpcode::St => {
                    out.push_str(&format!("{}ST   {}\n", indent, ins.operand_name));
                }
                VmOpcode::Stn => {
                    out.push_str(&format!("{}STN  {}\n", indent, ins.operand_name));
                }
                VmOpcode::S => {
                    out.push_str(&format!("{}S    {}\n", indent, ins.operand_name));
                }
                VmOpcode::R => {
                    out.push_str(&format!("{}R    {}\n", indent, ins.operand_name));
                }
                VmOpcode::Ton => {
                    out.push_str(&format!("{}CAL  {}(IN := TRUE, PT := T#{}ms)\n", indent, ins.operand_name, ins.imm));
                }
                VmOpcode::Tof => {
                    out.push_str(&format!("{}CAL  {}(IN := FALSE, PT := T#{}ms)\n", indent, ins.operand_name, ins.imm));
                }
                VmOpcode::Tp => {
                    out.push_str(&format!("{}CAL  {}(IN := TRUE, PT := T#{}ms)\n", indent, ins.operand_name, ins.imm));
                }
                VmOpcode::Ctu => {
                    out.push_str(&format!("{}CAL  {}(CU := TRUE, PV := {})\n", indent, ins.operand_name, ins.imm));
                }
                VmOpcode::Ctd => {
                    out.push_str(&format!("{}CAL  {}(CD := TRUE, PV := {})\n", indent, ins.operand_name, ins.imm));
                }
                VmOpcode::Ctud => {
                    out.push_str(&format!("{}CAL  {}(CU := TRUE, PV := {})\n", indent, ins.operand_name, ins.imm));
                }
                VmOpcode::Eq => out.push_str(&format!("{}EQ   {} {}\n", indent, ins.operand_name, ins.imm)),
                VmOpcode::Ne => out.push_str(&format!("{}NE   {} {}\n", indent, ins.operand_name, ins.imm)),
                VmOpcode::Gt => out.push_str(&format!("{}GT   {} {}\n", indent, ins.operand_name, ins.imm)),
                VmOpcode::Lt => out.push_str(&format!("{}LT   {} {}\n", indent, ins.operand_name, ins.imm)),
                VmOpcode::Ge => out.push_str(&format!("{}GE   {} {}\n", indent, ins.operand_name, ins.imm)),
                VmOpcode::Le => out.push_str(&format!("{}LE   {} {}\n", indent, ins.operand_name, ins.imm)),
                VmOpcode::Move => {
                    out.push_str(&format!("{}MOVE {} {}\n", indent, ins.operand_name, ins.imm));
                }
                VmOpcode::Add => {
                    out.push_str(&format!("{}ADD  {} {}\n", indent, ins.operand_name, ins.imm));
                }
                VmOpcode::Sub => {
                    out.push_str(&format!("{}SUB  {} {}\n", indent, ins.operand_name, ins.imm));
                }
                VmOpcode::Mul => {
                    out.push_str(&format!("{}MUL  {} {}\n", indent, ins.operand_name, ins.imm));
                }
                VmOpcode::Div => {
                    out.push_str(&format!("{}DIV  {} {}\n", indent, ins.operand_name, ins.imm));
                }
                VmOpcode::Mod => {
                    out.push_str(&format!("{}MOD  {} {}\n", indent, ins.operand_name, ins.imm));
                }
                VmOpcode::Jmpc => {
                    out.push_str(&format!("{}JMPC LBL_{}\n", indent, ins.imm));
                }
                VmOpcode::Lbl => {
                    out.push_str(&format!("LBL_{}:\n", ins.addr));
                }
                VmOpcode::Movw => {
                    if op_is_const(0) {
                        out.push_str(&format!("{}MOVE {} {}\n", indent, dst, ops[0]));
                    } else {
                        out.push_str(&format!("{}ADC  {} {}\n", indent, ops[0], dst));
                    }
                }
                VmOpcode::Wand | VmOpcode::Wor | VmOpcode::Wxor | VmOpcode::Wnot | VmOpcode::Shl | VmOpcode::Shr => {
                    let edge = matches!(ins.op, VmOpcode::Shl | VmOpcode::Shr) && (ins.imm & SHIFT_EDGE_FLAG) != 0;
                    if edge && op_word_addr(0) == Some(ins.addr) {
                        if op_is_const(1) {
                            let m = if ins.op == VmOpcode::Shl { "SHIFT_L" } else { "SHIFT_R" };
                            out.push_str(&format!("{}{} {} {}\n", indent, m, dst, ops[1]));
                        } else if ins.op == VmOpcode::Shl {
                            out.push_str(&format!("{}SHIFT_VAR {} {}\n", indent, dst, ops[1]));
                        } else {
                            out.push_str(&format!("{}SHR  {} {} {}\n", indent, dst, ops[0], ops[1]));
                        }
                    } else if ins.op == VmOpcode::Wnot {
                        out.push_str(&format!("{}WNOT {} {}\n", indent, dst, ops[0]));
                    } else {
                        out.push_str(&format!("{}{} {} {} {}\n", indent, ins.op.mnemonic(), dst, ops[0], ops[1]));
                    }
                }
                VmOpcode::Shreg => {
                    out.push_str(&format!("{}SHIFT_REG {} {} {}\n", indent, dst, ins.imm & 0xFF, ops[0]));
                }
                VmOpcode::Shbit => {
                    out.push_str(&format!("{}SHIFT_BIT {} {}\n", indent, dst, ops[0]));
                }
                VmOpcode::Scale => {
                    scale_idx += 1;
                    out.push_str(&format!(
                        "{}CAL  SCALE_{}(IN := {}, OUT => {}, IN_MIN := {}, IN_MAX := {}, OUT_MIN := {}, OUT_MAX := {})\n",
                        indent, scale_idx, ops[0], dst, ops[1], ops[2], ops[3], ops[4]
                    ));
                }
                VmOpcode::Limit => {
                    limit_idx += 1;
                    out.push_str(&format!(
                        "{}CAL  LIMIT_{}(IN := {}, OUT => {}, MN := {}, MX := {})\n",
                        indent, limit_idx, ops[0], dst, ops[1], ops[2]
                    ));
                }
                VmOpcode::Pid => {
                    out.push_str(&format!(
                        "{}CAL  PID{}(PV := {}, SP := {}, CV => {}, KP := {}, KI := {}, KD := {}, OUTMAX := {})\n",
                        indent, ins.imm, ops[0], ops[1], dst, ops[2], ops[3], ops[4], ops[5]
                    ));
                }
                VmOpcode::Utx => {
                    out.push_str(&format!("{}UART_SEND U{}(IN := {}, MODE := {})\n", indent, ins.addr, ops[0], ins.imm));
                }
                VmOpcode::Urx => {
                    urx_idx += 1;
                    out.push_str(&format!("{}UART_RECV URX{}(OUT => {})\n", indent, urx_idx, dst));
                }
                _ => {}
            }
            idx += n_ops;
        }

        out.push_str("\nEND_PROGRAM\n");
        out
    }

    /// Decompile the bytecode directly back into a full `LadderProgram` (Rungs and Elements)
    pub fn decompile_to_ladder(&self) -> Result<LadderProgram, String> {
        let il_code = self.decompile_to_il();
        let mut prog = IlParser::parse(&il_code)?;
        prog.hardware = self.hardware.clone();
        Ok(prog)
    }

    /// Format the complete Serial Packet as a hex string
    pub fn to_packet_hex_string(&self, cmd: PlcSerialCommand) -> String {
        self.to_serial_packet(cmd)
            .iter()
            .map(|b| format!("{:02X}", b))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Generate a human-readable disassembly and hex dump table
    pub fn to_disassembly_text(&self) -> String {
        let mut out = String::new();
        out.push_str("// ============================================================================\n");
        out.push_str("// LaduinoPLC - Micro-VM 32-bit Bytecode Disassembly & Unified Memory Map\n");
        out.push_str(&format!("// Target Hardware : {}\n", self.hardware));
        out.push_str(&format!(
            "// Instructions    : {} ({} Bytes payload)\n",
            self.instructions.len(),
            self.payload_size_bytes()
        ));
        out.push_str("// Format          : [Byte0: OP] [Byte1: ADDR] [Byte2..3: IMM16 (LE)]\n");
        out.push_str("// ============================================================================\n\n");
        out.push_str("PC    HEX (4B)      OPCODE   ADDR    IMM (K)   IL SOURCE / DESCRIPTION\n");
        out.push_str("----  ------------  -------  ------  --------  --------------------------------\n");
        for ins in &self.instructions {
            let addr_str = if ins.region == MemoryRegion::None && ins.op != VmOpcode::Jmpc {
                "0x00  ".to_string()
            } else {
                format!("0x{:02X}  ", ins.addr)
            };
            out.push_str(&format!(
                "{:04}  {}  {:<7}  {}  {:<8}  {:<18} ; {}\n",
                ins.pc,
                ins.hex_bytes(),
                ins.op.mnemonic(),
                addr_str,
                ins.imm,
                ins.il_source,
                ins.op.description()
            ));
        }

        let packet = self.to_serial_packet(PlcSerialCommand::DownloadProgram);
        let crc = packet.last().copied().unwrap_or(0);
        out.push_str("\n// --- Serial UART Packet Frame (SOF=0xAA, CMD=0x01, LEN, PAYLOAD, CRC8=0x");
        out.push_str(&format!("{:02X}) ---\n", crc));
        out.push_str(&self.to_packet_hex_string(PlcSerialCommand::DownloadProgram));
        out.push('\n');
        out
    }
}

/// Compiler that translates IEC 61131-3 IL or `LadderProgram` into 32-bit Micro-VM Bytecode
pub struct BytecodeCompiler;

impl BytecodeCompiler {
    /// Readable IL text of a word address (`%MW10`, `%IW0`, `%QW1`, `T0`, `C3`)
    pub fn word_operand_text(addr: u8) -> String {
        let (canonical, iec, region) = Self::format_address(addr, true);
        match region {
            MemoryRegion::TimerT | MemoryRegion::CounterC => canonical,
            _ => iec,
        }
    }

    /// Readable IL text of a bit address (`%IX0.1`, `%QX0.0`, `%MX8.0`, `T0`, `C3`)
    pub fn bit_operand_text(addr: u8) -> String {
        let (canonical, iec, region) = Self::format_address(addr, false);
        match region {
            MemoryRegion::TimerT | MemoryRegion::CounterC => canonical,
            _ => iec,
        }
    }

    /// Readable IL text of an `OPND` word (`kind`, `imm`)
    pub fn opnd_text(kind: u8, imm: u16) -> String {
        match kind {
            OPND_KIND_WORD => Self::word_operand_text(imm as u8),
            OPND_KIND_CONST => (imm as i16).to_string(),
            OPND_KIND_BIT => Self::bit_operand_text(imm as u8),
            _ => format!("?{}", imm),
        }
    }

    /// Format raw memory address into standard canonical name, IEC 61131-3 address, and MemoryRegion
    pub fn format_address(addr: u8, for_word_op: bool) -> (String, String, MemoryRegion) {
        if for_word_op {
            if addr < 0x10 {
                (format!("AI{}", addr), format!("%IW{}", addr), MemoryRegion::AnalogInputAI)
            } else if addr < 0x20 {
                let idx = addr - 0x10;
                (format!("AO{}", idx), format!("%QW{}", idx), MemoryRegion::AnalogOutputAO)
            } else if addr < 0xC0 {
                let idx = addr - 0x20;
                (format!("D{}", idx), format!("%MW{}", idx), MemoryRegion::DataRegisterD)
            } else if addr < 0xE0 {
                let idx = addr - 0xC0;
                (format!("T{}", idx), format!("%TM{}", idx), MemoryRegion::TimerT)
            } else {
                let idx = addr - 0xE0;
                (format!("C{}", idx), format!("%CT{}", idx), MemoryRegion::CounterC)
            }
        } else {
            if addr < 0x20 {
                let byte = addr / 8;
                let bit = addr % 8;
                (format!("X{}", addr), format!("%IX{}.{}", byte, bit), MemoryRegion::DigitalInputX)
            } else if addr < 0x40 {
                let idx = addr - 0x20;
                let byte = idx / 8;
                let bit = idx % 8;
                (format!("Y{}", idx), format!("%QX{}.{}", byte, bit), MemoryRegion::DigitalOutputY)
            } else if addr < 0xC0 {
                let idx = addr - 0x40;
                let byte = idx / 8;
                let bit = idx % 8;
                (format!("M{}", idx), format!("%MX{}.{}", byte, bit), MemoryRegion::InternalRelayM)
            } else if addr < 0xE0 {
                let idx = addr - 0xC0;
                (format!("T{}", idx), format!("%TM{}", idx), MemoryRegion::TimerT)
            } else {
                let idx = addr - 0xE0;
                (format!("C{}", idx), format!("%CT{}", idx), MemoryRegion::CounterC)
            }
        }
    }

    /// Resolve any operand string (`"X0"`, `"%IX0.0"`, `"Y1"`, `"%QX0.1"`, `"M0"`, `"T0"`, `"C0"`, `"D0"`, or symbol)
    /// into `(addr: u8, canonical_name: String, region: MemoryRegion, symbol: String, hw_pin: String)`
    pub fn resolve_address(
        raw_operand: &str,
        for_word_op: bool,
        io_mappings: &[IOPortMapping],
    ) -> (u8, String, MemoryRegion, String, String) {
        let trimmed = raw_operand.trim().trim_end_matches(';');
        if trimmed.is_empty() {
            return (0x00, String::new(), MemoryRegion::None, String::new(), String::new());
        }

        // Strip member suffix like T0.Q or C0.QU if present
        let base_token = if trimmed.starts_with('%') {
            trimmed
        } else {
            trimmed.split('.').next().unwrap_or(trimmed)
        };

        // 1. Check IO mappings first (matches plc_address, iec_address, or symbol)
        let mut in_idx = 0usize;
        let mut out_idx = 0usize;
        for io in io_mappings {
            let cur_idx = if io.is_input {
                let c = in_idx;
                in_idx += 1;
                c
            } else {
                let c = out_idx;
                out_idx += 1;
                c
            };
            let iec = io.iec_address(cur_idx);
            if io.plc_address.eq_ignore_ascii_case(base_token)
                || iec.eq_ignore_ascii_case(base_token)
                || (!io.symbol.is_empty() && io.symbol.eq_ignore_ascii_case(base_token))
            {
                let (addr, canonical, region) = Self::parse_direct_address(&io.plc_address, for_word_op)
                    .unwrap_or_else(|| {
                        if io.is_input {
                            let idx = (cur_idx as u8).min(31);
                            (idx, format!("X{}", idx), MemoryRegion::DigitalInputX)
                        } else {
                            let idx = (cur_idx as u8).min(31);
                            (0x20 + idx, format!("Y{}", idx), MemoryRegion::DigitalOutputY)
                        }
                    });
                return (
                    addr,
                    canonical,
                    region,
                    io.symbol.clone(),
                    io.hw_pin.clone(),
                );
            }
        }

        // 2. Parse direct IEC or PLC address
        if let Some((addr, canonical, region)) = Self::parse_direct_address(base_token, for_word_op) {
            return (addr, canonical, region, base_token.to_string(), String::new());
        }

        // 3. Fallback to M0 (or D0 for word ops)
        if for_word_op {
            (0x20, "D0".to_string(), MemoryRegion::DataRegisterD, base_token.to_string(), String::new())
        } else {
            (0x40, "M0".to_string(), MemoryRegion::InternalRelayM, base_token.to_string(), String::new())
        }
    }

    pub fn parse_direct_address(token: &str, for_word_op: bool) -> Option<(u8, String, MemoryRegion)> {
        let upper = token.trim().to_ascii_uppercase();

        // Standard IEC 61131-3 addresses (%IX, %QX, %MX, %IW, %QW, %MW)
        if let Some(rest) = upper.strip_prefix("%IX") {
            let bit_index = Self::parse_iec_byte_bit(rest)?;
            let clamped = bit_index.min(31) as u8;
            return Some((clamped, format!("X{}", clamped), MemoryRegion::DigitalInputX));
        }
        if let Some(rest) = upper.strip_prefix("%QX") {
            let bit_index = Self::parse_iec_byte_bit(rest)?;
            let clamped = bit_index.min(31) as u8;
            return Some((0x20 + clamped, format!("Y{}", clamped), MemoryRegion::DigitalOutputY));
        }
        if let Some(rest) = upper.strip_prefix("%MX") {
            let bit_index = Self::parse_iec_byte_bit(rest)?;
            let clamped = bit_index.min(127) as u8;
            return Some((0x40 + clamped, format!("M{}", clamped), MemoryRegion::InternalRelayM));
        }
        if let Some(rest) = upper.strip_prefix("%IW") {
            let idx = rest.parse::<usize>().ok()?.min(15) as u8;
            return Some((idx, format!("AI{}", idx), MemoryRegion::AnalogInputAI));
        }
        if let Some(rest) = upper.strip_prefix("%QW") {
            let idx = rest.parse::<usize>().ok()?.min(15) as u8;
            return Some((0x10 + idx, format!("AO{}", idx), MemoryRegion::AnalogOutputAO));
        }
        if let Some(rest) = upper.strip_prefix("%MW") {
            let idx = rest.parse::<usize>().ok()?.min(159) as u8;
            return Some((0x20 + idx, format!("D{}", idx), MemoryRegion::DataRegisterD));
        }

        // Special System Markers (%SM / M8000 series)
        if let Some(rest) = upper.strip_prefix("%SM") {
            let idx = rest.parse::<usize>().ok()?.min(31) as u8;
            return Some((0xA0 + idx, format!("SM{}", idx), MemoryRegion::InternalRelayM));
        }
        if upper.starts_with("M80") {
            if let Ok(num) = upper[1..].parse::<usize>() {
                if (8000..=8031).contains(&num) {
                    let idx = (num - 8000) as u8;
                    return Some((0xA0 + idx, format!("SM{}", idx), MemoryRegion::InternalRelayM));
                }
            }
        }

        // Special System Registers (%SD / D8000 series)
        if let Some(rest) = upper.strip_prefix("%SD") {
            let idx = rest.parse::<usize>().ok()?.min(31) as u8;
            return Some((0xA0 + idx, format!("SD{}", idx), MemoryRegion::DataRegisterD));
        }
        if upper.starts_with("D80") {
            if let Ok(num) = upper[1..].parse::<usize>() {
                if (8000..=8031).contains(&num) {
                    let idx = (num - 8000) as u8;
                    return Some((0xA0 + idx, format!("SD{}", idx), MemoryRegion::DataRegisterD));
                }
            }
        }

        // Analog AI / AO
        if let Some(rest) = upper.strip_prefix("AI").or_else(|| upper.strip_prefix("ADC")) {
            let idx = rest.parse::<usize>().ok()?.min(15) as u8;
            return Some((idx, format!("AI{}", idx), MemoryRegion::AnalogInputAI));
        }
        if let Some(rest) = upper.strip_prefix("AO").or_else(|| upper.strip_prefix("PWM")) {
            let idx = rest.parse::<usize>().ok()?.min(15) as u8;
            return Some((0x10 + idx, format!("AO{}", idx), MemoryRegion::AnalogOutputAO));
        }

        // Classic PLC addresses: X, Y, M, T, C, D/V/R
        if let Some(rest) = upper.strip_prefix('X').or_else(|| upper.strip_prefix('I')) {
            let idx = rest.parse::<usize>().ok()?.min(31) as u8;
            return Some((idx, format!("X{}", idx), MemoryRegion::DigitalInputX));
        }
        if let Some(rest) = upper.strip_prefix('Y').or_else(|| upper.strip_prefix('Q')) {
            let idx = rest.parse::<usize>().ok()?.min(31) as u8;
            return Some((0x20 + idx, format!("Y{}", idx), MemoryRegion::DigitalOutputY));
        }
        if let Some(rest) = upper.strip_prefix('M') {
            let idx = rest.parse::<usize>().ok()?.min(127) as u8;
            return Some((0x40 + idx, format!("M{}", idx), MemoryRegion::InternalRelayM));
        }
        if let Some(rest) = upper.strip_prefix('T') {
            let idx = rest.parse::<usize>().ok()?.min(31) as u8;
            return Some((0xC0 + idx, format!("T{}", idx), MemoryRegion::TimerT));
        }
        if let Some(rest) = upper.strip_prefix('C') {
            let idx = rest.parse::<usize>().ok()?.min(31) as u8;
            return Some((0xE0 + idx, format!("C{}", idx), MemoryRegion::CounterC));
        }
        if let Some(rest) = upper
            .strip_prefix('D')
            .or_else(|| upper.strip_prefix('V'))
            .or_else(|| upper.strip_prefix('R'))
        {
            let idx = rest.parse::<usize>().ok()?.min(159) as u8;
            if for_word_op {
                return Some((0x20 + idx, format!("D{}", idx), MemoryRegion::DataRegisterD));
            } else {
                let m_idx = idx.min(127);
                return Some((0x40 + m_idx, format!("M{}", m_idx), MemoryRegion::InternalRelayM));
            }
        }

        None
    }

    fn parse_iec_byte_bit(s: &str) -> Option<usize> {
        if let Some((b_str, bit_str)) = s.split_once('.') {
            let byte_idx = b_str.parse::<usize>().ok()?;
            let bit_idx = bit_str.parse::<usize>().ok()?;
            Some(byte_idx * 8 + bit_idx)
        } else {
            s.parse::<usize>().ok()
        }
    }

    /// Compile a `LadderProgram` directly into a `BytecodeProgram`
    pub fn compile_program(prog: &LadderProgram) -> BytecodeProgram {
        Self::compile_program_with_report(prog).0
    }

    /// Compile a `LadderProgram` into a `BytecodeProgram` and return the compile-time
    /// diagnostics (out-of-range constants, PID/UART instance limits, unsupported elements,
    /// program size limit, ...).
    pub fn compile_program_with_report(prog: &LadderProgram) -> (BytecodeProgram, CompileReport) {
        let mut instructions: Vec<BytecodeInstruction> = Vec::new();
        let mut mem_table: HashMap<(u8, bool), MemoryMapEntry> = HashMap::new();
        let mut ctx = CompileCtx::default();

        // Pre-populate IO mappings in memory table
        for (idx, io) in prog.io_mappings.iter().enumerate() {
            let (addr, canonical, region, sym, hw_pin) =
                Self::resolve_address(&io.plc_address, false, &prog.io_mappings);
            if region != MemoryRegion::None {
                mem_table.entry((addr, false)).or_insert(MemoryMapEntry {
                    addr,
                    canonical_name: canonical,
                    iec_address: io.iec_address(idx),
                    symbol: sym,
                    region,
                    hw_pin,
                });
            }
        }

        for rung in &prog.rungs {
            Self::compile_rung(
                rung,
                &prog.io_mappings,
                &mut instructions,
                &mut mem_table,
                &mut ctx,
            );
        }

        // Append END instruction (0xFF)
        let end_pc = instructions.len() as u16;
        instructions.push(BytecodeInstruction {
            pc: end_pc,
            op: VmOpcode::End,
            addr: 0x00,
            imm: 0x0000,
            region: MemoryRegion::None,
            operand_name: String::new(),
            symbol: String::new(),
            il_source: "END".to_string(),
        });

        // Resolve forward/backward jump targets
        for (ins_idx, lbl_name) in std::mem::take(&mut ctx.pending_jumps) {
            if let Some(&target_pc) = ctx.label_pcs.get(&lbl_name.to_uppercase()) {
                if let Some(ins) = instructions.get_mut(ins_idx) {
                    ins.imm = target_pc;
                }
            } else {
                ctx.report.warn(format!("JMP {}: label not found", lbl_name));
            }
        }

        if instructions.len() > uplc_vm::MAX_INSTRUCTIONS {
            ctx.report.error(format!(
                "Program too large: {} instructions (Micro-VM limit is {})",
                instructions.len(),
                uplc_vm::MAX_INSTRUCTIONS
            ));
        }

        let mut memory_map: Vec<MemoryMapEntry> = mem_table.into_values().collect();
        memory_map.sort_by_key(|e| (Self::region_sort_order(e.region), e.addr));

        (
            BytecodeProgram {
                hardware: prog.hardware.clone(),
                instructions,
                memory_map,
            },
            ctx.report,
        )
    }

    /// Compile an IEC 61131-3 IL text string into a `BytecodeProgram`
    pub fn compile_il(il_text: &str) -> Result<BytecodeProgram, String> {
        let prog = IlParser::parse(il_text)?;
        Ok(Self::compile_program(&prog))
    }

    /// Compile an IEC 61131-3 IL text string into a `BytecodeProgram` plus its `CompileReport`
    pub fn compile_il_with_report(il_text: &str) -> Result<(BytecodeProgram, CompileReport), String> {
        let prog = IlParser::parse(il_text)?;
        Ok(Self::compile_program_with_report(&prog))
    }

    fn region_sort_order(region: MemoryRegion) -> u8 {
        match region {
            MemoryRegion::DigitalInputX => 0,
            MemoryRegion::DigitalOutputY => 1,
            MemoryRegion::InternalRelayM => 2,
            MemoryRegion::TimerT => 3,
            MemoryRegion::CounterC => 4,
            MemoryRegion::AnalogInputAI => 5,
            MemoryRegion::AnalogOutputAO => 6,
            MemoryRegion::DataRegisterD => 7,
            MemoryRegion::None => 8,
        }
    }

    fn record_mem_entry(
        addr: u8,
        canonical: &str,
        raw_name: &str,
        symbol: &str,
        region: MemoryRegion,
        hw_pin: &str,
        for_word: bool,
        mem_table: &mut HashMap<(u8, bool), MemoryMapEntry>,
    ) {
        if region == MemoryRegion::None {
            return;
        }
        mem_table.entry((addr, for_word)).or_insert_with(|| MemoryMapEntry {
            addr,
            canonical_name: canonical.to_string(),
            iec_address: raw_name.to_string(),
            symbol: symbol.to_string(),
            region,
            hw_pin: hw_pin.to_string(),
        });
    }

    fn push_bit_ins(
        instructions: &mut Vec<BytecodeInstruction>,
        op: VmOpcode,
        raw_name: &str,
        io_mappings: &[IOPortMapping],
        mem_table: &mut HashMap<(u8, bool), MemoryMapEntry>,
        imm: u16,
        il_source: String,
    ) {
        let (addr, canonical, region, sym, hw_pin) =
            Self::resolve_address(raw_name, false, io_mappings);
        Self::record_mem_entry(addr, &canonical, raw_name, &sym, region, &hw_pin, false, mem_table);
        let pc = instructions.len() as u16;
        instructions.push(BytecodeInstruction {
            pc,
            op,
            addr,
            imm,
            region,
            operand_name: canonical,
            symbol: sym,
            il_source,
        });
    }

    fn push_word_ins(
        instructions: &mut Vec<BytecodeInstruction>,
        op: VmOpcode,
        raw_name: &str,
        io_mappings: &[IOPortMapping],
        mem_table: &mut HashMap<(u8, bool), MemoryMapEntry>,
        imm: u16,
        il_source: String,
    ) {
        let (addr, canonical, region, sym, hw_pin) =
            Self::resolve_address(raw_name, true, io_mappings);
        Self::record_mem_entry(addr, &canonical, raw_name, &sym, region, &hw_pin, true, mem_table);
        let pc = instructions.len() as u16;
        instructions.push(BytecodeInstruction {
            pc,
            op,
            addr,
            imm,
            region,
            operand_name: canonical,
            symbol: sym,
            il_source,
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn push_raw(
        instructions: &mut Vec<BytecodeInstruction>,
        op: VmOpcode,
        addr: u8,
        imm: u16,
        region: MemoryRegion,
        operand_name: String,
        symbol: String,
        il_source: String,
    ) {
        let pc = instructions.len() as u16;
        instructions.push(BytecodeInstruction {
            pc,
            op,
            addr,
            imm,
            region,
            operand_name,
            symbol,
            il_source,
        });
    }

    /// `true` if `raw` resolves to a real address (IO mapping or direct PLC/IEC address)
    fn is_resolvable(raw: &str, io_mappings: &[IOPortMapping]) -> bool {
        let t = raw.trim().trim_end_matches(';');
        let base = if t.starts_with('%') { t } else { t.split('.').next().unwrap_or(t) };
        Self::parse_direct_address(base, true).is_some()
            || io_mappings.iter().enumerate().any(|(i, io)| {
                io.plc_address.eq_ignore_ascii_case(base)
                    || io.iec_address(i).eq_ignore_ascii_case(base)
                    || (!io.symbol.is_empty() && io.symbol.eq_ignore_ascii_case(base))
            })
    }

    /// Resolve a word destination register (OUT / CV / base), recording it in the memory map.
    fn word_dst(
        raw: &str,
        what: &str,
        io_mappings: &[IOPortMapping],
        mem_table: &mut HashMap<(u8, bool), MemoryMapEntry>,
        ctx: &mut CompileCtx,
    ) -> (u8, String, MemoryRegion, String) {
        if raw.trim().is_empty() {
            ctx.report.warn(format!("{}: missing destination register (using %MW0)", what));
        } else if !Self::is_resolvable(raw, io_mappings) {
            ctx.report.warn(format!("{}: unknown register '{}' (using %MW0)", what, raw.trim()));
        }
        let (addr, canonical, region, sym, hw) = Self::resolve_address(raw, true, io_mappings);
        let (addr, canonical, region) = match region {
            MemoryRegion::DigitalInputX | MemoryRegion::DigitalOutputY | MemoryRegion::InternalRelayM => {
                ctx.report.error(format!("{}: destination '{}' must be a word register", what, raw.trim()));
                (addr, canonical, region)
            }
            MemoryRegion::None => (0x20, "D0".to_string(), MemoryRegion::DataRegisterD),
            _ => (addr, canonical, region),
        };
        Self::record_mem_entry(addr, &canonical, raw.trim(), &sym, region, &hw, true, mem_table);
        (addr, canonical, region, sym)
    }

    /// Push a constant `OPND` word, checking the i16 range
    fn push_const_operand(instructions: &mut Vec<BytecodeInstruction>, v: i64, what: &str, ctx: &mut CompileCtx) {
        let clamped = if v < i16::MIN as i64 || v > i16::MAX as i64 {
            ctx.report.error(format!(
                "{}: constant {} is out of the i16 range (-32768..32767)",
                what, v
            ));
            v.clamp(i16::MIN as i64, i16::MAX as i64)
        } else {
            v
        };
        let imm = clamped as i16 as u16;
        let text = (clamped as i16).to_string();
        Self::push_raw(
            instructions,
            VmOpcode::Opnd,
            OPND_KIND_CONST,
            imm,
            MemoryRegion::None,
            text.clone(),
            String::new(),
            format!("  OPND {}", text),
        );
    }

    /// Push an `OPND` word for `raw`: integer literal -> const, bit address -> bit, otherwise word
    /// register. With `want_bit` the operand is resolved in the bit address space.
    fn push_operand(
        instructions: &mut Vec<BytecodeInstruction>,
        raw: &str,
        want_bit: bool,
        what: &str,
        io_mappings: &[IOPortMapping],
        mem_table: &mut HashMap<(u8, bool), MemoryMapEntry>,
        ctx: &mut CompileCtx,
    ) {
        let t = raw.trim();
        if t.is_empty() {
            ctx.report.warn(format!("{}: missing operand (using 0)", what));
            Self::push_const_operand(instructions, 0, what, ctx);
            return;
        }
        if let Some(v) = parse_int_literal(t) {
            Self::push_const_operand(instructions, v, what, ctx);
            return;
        }
        match t.to_ascii_uppercase().as_str() {
            "TRUE" => return Self::push_const_operand(instructions, 1, what, ctx),
            "FALSE" => return Self::push_const_operand(instructions, 0, what, ctx),
            _ => {}
        }
        if !Self::is_resolvable(t, io_mappings) {
            ctx.report.warn(format!("{}: unknown operand '{}'", what, t));
        }
        let (addr, canonical, region, sym, hw) = Self::resolve_address(t, !want_bit, io_mappings);
        let is_bit = want_bit
            || matches!(
                region,
                MemoryRegion::DigitalInputX | MemoryRegion::DigitalOutputY | MemoryRegion::InternalRelayM
            );
        Self::record_mem_entry(addr, &canonical, t, &sym, region, &hw, !is_bit, mem_table);
        let kind = if is_bit { OPND_KIND_BIT } else { OPND_KIND_WORD };
        let text = Self::opnd_text(kind, addr as u16);
        Self::push_raw(
            instructions,
            VmOpcode::Opnd,
            kind,
            addr as u16,
            region,
            text.clone(),
            sym,
            format!("  OPND {}", text),
        );
    }

    /// Feature-level-2 blocks placed at the start of a rung get an explicit `LD %SM0`
    /// (always ON) so the VM matches the Simulator (power rail = TRUE).
    fn ensure_rung_condition(
        first: &mut bool,
        instructions: &mut Vec<BytecodeInstruction>,
        io_mappings: &[IOPortMapping],
        mem_table: &mut HashMap<(u8, bool), MemoryMapEntry>,
    ) {
        if *first {
            Self::push_bit_ins(instructions, VmOpcode::Ld, "%SM0", io_mappings, mem_table, 0, "LD %SM0".to_string());
            *first = false;
        }
    }

    fn push_stack_ins(instructions: &mut Vec<BytecodeInstruction>, op: VmOpcode) {
        let pc = instructions.len() as u16;
        instructions.push(BytecodeInstruction {
            pc,
            op,
            addr: 0x00,
            imm: 0x0000,
            region: MemoryRegion::None,
            operand_name: String::new(),
            symbol: String::new(),
            il_source: op.mnemonic().to_string(),
        });
    }

    fn compile_rung(
        rung: &Rung,
        io_mappings: &[IOPortMapping],
        instructions: &mut Vec<BytecodeInstruction>,
        mem_table: &mut HashMap<(u8, bool), MemoryMapEntry>,
        ctx: &mut CompileCtx,
    ) {
        if rung.elements.is_empty() {
            return;
        }

        let mut first = true;
        for el in &rung.elements {
            Self::compile_element(
                el,
                &mut first,
                io_mappings,
                instructions,
                mem_table,
                ctx,
            );
        }
    }

    fn compile_element(
        el: &LadderElement,
        first: &mut bool,
        io_mappings: &[IOPortMapping],
        instructions: &mut Vec<BytecodeInstruction>,
        mem_table: &mut HashMap<(u8, bool), MemoryMapEntry>,
        ctx: &mut CompileCtx,
    ) {
        match el.element_type {
            TypeTag::Contact => {
                let op = match (el.contact_mode, *first) {
                    (ContactMode::NormallyOpen, true) => VmOpcode::Ld,
                    (ContactMode::NormallyOpen, false) => VmOpcode::And,
                    (ContactMode::NormallyClosed, true) => VmOpcode::Ldn,
                    (ContactMode::NormallyClosed, false) => VmOpcode::Andn,
                    (ContactMode::PositiveTransition, true) => VmOpcode::Ldp,
                    (ContactMode::PositiveTransition, false) => VmOpcode::Andp,
                    (ContactMode::NegativeTransition, true) => VmOpcode::Ldf,
                    (ContactMode::NegativeTransition, false) => VmOpcode::Andf,
                };
                let src = format!("{} {}", op.mnemonic(), el.name);
                Self::push_bit_ins(instructions, op, &el.name, io_mappings, mem_table, 0, src);
                *first = false;
            }
            TypeTag::Coil => {
                if *first {
                    // Unconditional rung coil: load constant true via LDN M127 or treat acc=1
                }
                let op = match el.coil_mode {
                    CoilMode::Normal => VmOpcode::St,
                    CoilMode::Negated => VmOpcode::Stn,
                    CoilMode::SetLatch => VmOpcode::S,
                    CoilMode::ResetUnlatch => VmOpcode::R,
                    CoilMode::PositiveTransition => VmOpcode::Stp,
                    CoilMode::NegativeTransition => VmOpcode::Stf,
                };
                let src = format!("{} {}", op.mnemonic(), el.name);
                Self::push_bit_ins(instructions, op, &el.name, io_mappings, mem_table, 0, src);
                *first = false;
            }
            TypeTag::Branch => {
                if *first {
                    if let Some(first_path) = el.branch_paths.first() {
                        let mut p_first = true;
                        for sub in first_path {
                            Self::compile_element(
                                sub,
                                &mut p_first,
                                io_mappings,
                                instructions,
                                mem_table,
                                ctx,
                            );
                        }
                    }
                    for path in el.branch_paths.iter().skip(1) {
                        Self::compile_parallel_or_path(
                            path,
                            io_mappings,
                            instructions,
                            mem_table,
                            ctx,
                        );
                    }
                    *first = false;
                } else {
                    Self::push_stack_ins(instructions, VmOpcode::AndLp);
                    if let Some(first_path) = el.branch_paths.first() {
                        let mut p_first = true;
                        for sub in first_path {
                            Self::compile_element(
                                sub,
                                &mut p_first,
                                io_mappings,
                                instructions,
                                mem_table,
                                ctx,
                            );
                        }
                    }
                    for path in el.branch_paths.iter().skip(1) {
                        Self::compile_parallel_or_path(
                            path,
                            io_mappings,
                            instructions,
                            mem_table,
                            ctx,
                        );
                    }
                    Self::push_stack_ins(instructions, VmOpcode::Rp);
                }
            }
            TypeTag::Ton | TypeTag::Tof | TypeTag::Tp => {
                let op = match el.element_type {
                    TypeTag::Tof => VmOpcode::Tof,
                    TypeTag::Tp => VmOpcode::Tp,
                    _ => VmOpcode::Ton,
                };
                let imm = el.preset_value.clamp(0, 65535) as u16;
                let src = format!("{} {}, K{}", op.mnemonic(), el.name, imm);
                Self::push_bit_ins(instructions, op, &el.name, io_mappings, mem_table, imm, src);
                *first = true;
            }
            TypeTag::Ctu | TypeTag::Ctd | TypeTag::Ctud => {
                let op = match el.element_type {
                    TypeTag::Ctd => VmOpcode::Ctd,
                    TypeTag::Ctud => VmOpcode::Ctud,
                    _ => VmOpcode::Ctu,
                };
                let imm = el.preset_value.clamp(0, 65535) as u16;
                let src = format!("{} {}, K{}", op.mnemonic(), el.name, imm);
                Self::push_bit_ins(instructions, op, &el.name, io_mappings, mem_table, imm, src);
                *first = true;
            }
            TypeTag::Compare => {
                let op = match el.compare_op.trim().to_uppercase().as_str() {
                    "!=" | "<>" | "NE" => VmOpcode::Ne,
                    ">" | "GT" => VmOpcode::Gt,
                    "<" | "LT" => VmOpcode::Lt,
                    ">=" | "GE" => VmOpcode::Ge,
                    "<=" | "LE" => VmOpcode::Le,
                    _ => VmOpcode::Eq,
                };
                if !(0..=65535).contains(&el.preset_value) {
                    ctx.report.warn(format!(
                        "Compare {}: constant {} clamped to 0..65535",
                        el.name, el.preset_value
                    ));
                }
                let imm = el.preset_value.clamp(0, 65535) as u16;
                let src = format!("{} {}, K{}", op.mnemonic(), el.name, imm);
                Self::push_word_ins(instructions, op, &el.name, io_mappings, mem_table, imm, src);
                *first = false;
            }
            TypeTag::Move => {
                let dest = el.word_target().to_string();
                if (0..=65535).contains(&el.preset_value) {
                    let imm = el.preset_value as u16;
                    let src = format!("MOVE {}, K{}", dest, imm);
                    Self::push_word_ins(
                        instructions,
                        VmOpcode::Move,
                        &dest,
                        io_mappings,
                        mem_table,
                        imm,
                        src,
                    );
                } else {
                    // Negative constants need the signed OPND form: MOVW dst, const
                    let what = format!("MOVE {}", dest);
                    let (addr, canonical, region, sym) = Self::word_dst(&dest, &what, io_mappings, mem_table, ctx);
                    Self::push_raw(
                        instructions,
                        VmOpcode::Movw,
                        addr,
                        0,
                        region,
                        canonical,
                        sym,
                        format!("MOVW {}, K{}", dest, el.preset_value),
                    );
                    Self::push_const_operand(instructions, el.preset_value as i64, &what, ctx);
                }
                *first = false;
            }
            TypeTag::Math => {
                let op = match el.math_op.trim() {
                    "-" | "SUB" => VmOpcode::Sub,
                    "*" | "MUL" => VmOpcode::Mul,
                    "/" | "DIV" => VmOpcode::Div,
                    "%" | "MOD" => VmOpcode::Mod,
                    _ => VmOpcode::Add,
                };
                let dest = el.word_target().to_string();
                if !(0..=65535).contains(&el.preset_value) {
                    ctx.report.warn(format!(
                        "{} {}: constant {} clamped to 0..65535",
                        op.mnemonic(),
                        dest,
                        el.preset_value
                    ));
                }
                let imm = el.preset_value.clamp(0, 65535) as u16;
                let src = format!("{} {}, K{}", op.mnemonic(), dest, imm);
                Self::push_word_ins(instructions, op, &dest, io_mappings, mem_table, imm, src);
                *first = false;
            }
            TypeTag::RTrig => {
                let src = format!("R_TRIG {}", el.name);
                Self::push_bit_ins(
                    instructions,
                    VmOpcode::RTrig,
                    &el.name,
                    io_mappings,
                    mem_table,
                    0,
                    src,
                );
                *first = false;
            }
            TypeTag::FTrig => {
                let src = format!("F_TRIG {}", el.name);
                Self::push_bit_ins(
                    instructions,
                    VmOpcode::FTrig,
                    &el.name,
                    io_mappings,
                    mem_table,
                    0,
                    src,
                );
                *first = false;
            }
            TypeTag::Sr => {
                let src = format!("SR {}", el.name);
                Self::push_bit_ins(
                    instructions,
                    VmOpcode::Sr,
                    &el.name,
                    io_mappings,
                    mem_table,
                    0,
                    src,
                );
                *first = false;
            }
            TypeTag::Rs => {
                let src = format!("RS {}", el.name);
                Self::push_bit_ins(
                    instructions,
                    VmOpcode::Rs,
                    &el.name,
                    io_mappings,
                    mem_table,
                    0,
                    src,
                );
                *first = false;
            }
            TypeTag::MasterRelay => {
                let pc = instructions.len() as u16;
                instructions.push(BytecodeInstruction {
                    pc,
                    op: VmOpcode::Mc,
                    addr: 0x00,
                    imm: 0x0000,
                    region: MemoryRegion::None,
                    operand_name: "MC".to_string(),
                    symbol: el.symbol.clone(),
                    il_source: "MC".to_string(),
                });
                *first = false;
            }
            TypeTag::Label => {
                let pc = instructions.len() as u16;
                ctx.label_pcs.insert(el.name.to_uppercase(), pc);
                instructions.push(BytecodeInstruction {
                    pc,
                    op: VmOpcode::Lbl,
                    addr: 0x00,
                    imm: pc,
                    region: MemoryRegion::None,
                    operand_name: el.name.clone(),
                    symbol: el.symbol.clone(),
                    il_source: format!("{}:", el.name),
                });
            }
            TypeTag::Jump => {
                let idx = instructions.len();
                let pc = idx as u16;
                ctx.pending_jumps.push((idx, el.name.clone()));
                instructions.push(BytecodeInstruction {
                    pc,
                    op: VmOpcode::Jmpc,
                    addr: 0x00,
                    imm: 0x0000,
                    region: MemoryRegion::None,
                    operand_name: el.name.clone(),
                    symbol: el.symbol.clone(),
                    il_source: format!("JMPC {}", el.name),
                });
                *first = false;
            }
            TypeTag::MbMaster => {
                let inst_id = el
                    .name
                    .trim_start_matches(|c: char| !c.is_ascii_digit())
                    .parse::<u8>()
                    .unwrap_or(0)
                    .min(7);
                let slave_id = if el.mb_slave_id > 0 { el.mb_slave_id } else { 1 };
                let func = if el.mb_function > 0 { el.mb_function } else { 3 };
                let reg_addr = el.preset_value.clamp(0, 65535) as u16;
                let count = (el.preset_value1.max(1)).clamp(1, 125) as u16;

                let (buf_addr, _, _, _, _) =
                    Self::resolve_address(&el.target_reg, true, io_mappings);

                let imm0 = (slave_id as u16) | ((func as u16) << 8);
                let pc0 = instructions.len() as u16;
                instructions.push(BytecodeInstruction {
                    pc: pc0,
                    op: VmOpcode::MbMaster,
                    addr: inst_id,
                    imm: imm0,
                    region: MemoryRegion::None,
                    operand_name: format!("MB{}", inst_id),
                    symbol: el.symbol.clone(),
                    il_source: format!("MB_MASTER MB{}, Slave={}, Func={}", inst_id, slave_id, func),
                });

                let pc1 = instructions.len() as u16;
                instructions.push(BytecodeInstruction {
                    pc: pc1,
                    op: VmOpcode::MbParam1,
                    addr: buf_addr,
                    imm: reg_addr,
                    region: MemoryRegion::DataRegisterD,
                    operand_name: el.target_reg.clone(),
                    symbol: String::new(),
                    il_source: format!("MB_PARAM1 Buf={}, Addr={}", el.target_reg, reg_addr),
                });

                let is_write = matches!(func, 0x05 | 0x06 | 0x0F | 0x10);
                let flags: u8 = if is_write { 1 } else { 0 };
                let pc2 = instructions.len() as u16;
                instructions.push(BytecodeInstruction {
                    pc: pc2,
                    op: VmOpcode::MbParam2,
                    addr: flags,
                    imm: count,
                    region: MemoryRegion::None,
                    operand_name: String::new(),
                    symbol: String::new(),
                    il_source: format!("MB_PARAM2 Count={}", count),
                });

                *first = false;
            }
            TypeTag::Adc
            | TypeTag::Bitwise
            | TypeTag::Scale
            | TypeTag::Limit
            | TypeTag::Pid
            | TypeTag::ShiftRegisters
            | TypeTag::ShiftRegInbit
            | TypeTag::ShiftRegReset
            | TypeTag::ShiftLeft
            | TypeTag::ShiftRight
            | TypeTag::ShiftVariable
            | TypeTag::UartSend
            | TypeTag::UartRecv => {
                Self::ensure_rung_condition(first, instructions, io_mappings, mem_table);
                Self::compile_fb_element(el, io_mappings, instructions, mem_table, ctx);
                *first = false;
            }
            TypeTag::OpenCircuit => {
                ctx.report.warn("Open Circuit element is not supported by the bytecode compiler (ignored)");
            }
            _ => {}
        }
    }

    /// Compile the feature-level-2 function blocks (OPND-based opcodes)
    fn compile_fb_element(
        el: &LadderElement,
        io_mappings: &[IOPortMapping],
        instructions: &mut Vec<BytecodeInstruction>,
        mem_table: &mut HashMap<(u8, bool), MemoryMapEntry>,
        ctx: &mut CompileCtx,
    ) {
        let fb = &el.fb;
        let label = format!("{} '{}'", el.element_type.display_name(), el.name);
        match el.element_type {
            TypeTag::Adc => {
                let (addr, canonical, region, sym) = Self::word_dst(&el.target_reg, &label, io_mappings, mem_table, ctx);
                Self::push_raw(
                    instructions,
                    VmOpcode::Movw,
                    addr,
                    0,
                    region,
                    canonical,
                    sym,
                    format!("ADC {} -> {}", el.name, el.target_reg),
                );
                Self::push_operand(instructions, &el.name, false, &label, io_mappings, mem_table, ctx);
            }
            TypeTag::Bitwise => {
                let op = VmOpcode::from_u8(bitwise_opcode(&el.math_op)).unwrap_or(VmOpcode::Wand);
                let (addr, canonical, region, sym) = Self::word_dst(&el.target_reg, &label, io_mappings, mem_table, ctx);
                Self::push_raw(
                    instructions,
                    op,
                    addr,
                    0,
                    region,
                    canonical,
                    sym,
                    format!("{} {} := {}, {}", op.mnemonic(), el.target_reg, fb.in1, fb.in2),
                );
                Self::push_operand(instructions, &fb.in1, false, &label, io_mappings, mem_table, ctx);
                if op == VmOpcode::Wnot && fb.in2.trim().is_empty() {
                    Self::push_const_operand(instructions, 0, &label, ctx);
                } else {
                    Self::push_operand(instructions, &fb.in2, false, &label, io_mappings, mem_table, ctx);
                }
            }
            TypeTag::Scale => {
                let (addr, canonical, region, sym) = Self::word_dst(&el.target_reg, &label, io_mappings, mem_table, ctx);
                Self::push_raw(
                    instructions,
                    VmOpcode::Scale,
                    addr,
                    0,
                    region,
                    canonical,
                    sym,
                    format!("SCALE {} := {} [{}..{}] -> [{}..{}]", el.target_reg, fb.in1, fb.k[0], fb.k[1], fb.k[2], fb.k[3]),
                );
                Self::push_operand(instructions, &fb.in1, false, &label, io_mappings, mem_table, ctx);
                if fb.k[0] == fb.k[1] {
                    ctx.report.warn(format!("{}: IN_MIN == IN_MAX (output is always OUT_MIN)", label));
                }
                for k in fb.k {
                    Self::push_const_operand(instructions, k as i64, &label, ctx);
                }
            }
            TypeTag::Limit => {
                let (addr, canonical, region, sym) = Self::word_dst(&el.target_reg, &label, io_mappings, mem_table, ctx);
                Self::push_raw(
                    instructions,
                    VmOpcode::Limit,
                    addr,
                    0,
                    region,
                    canonical,
                    sym,
                    format!("LIMIT {} := {} [{}..{}]", el.target_reg, fb.in1, fb.k[0], fb.k[1]),
                );
                Self::push_operand(instructions, &fb.in1, false, &label, io_mappings, mem_table, ctx);
                if fb.k[0] > fb.k[1] {
                    ctx.report.warn(format!("{}: MIN > MAX", label));
                }
                Self::push_const_operand(instructions, fb.k[0] as i64, &label, ctx);
                Self::push_const_operand(instructions, fb.k[1] as i64, &label, ctx);
            }
            TypeTag::Pid => {
                let inst = CompileCtx::instance(
                    &mut ctx.pid_instances,
                    &mut ctx.report,
                    &el.name,
                    "PID",
                    uplc_vm::MAX_PID_INSTANCES,
                );
                let (addr, canonical, region, sym) = Self::word_dst(&el.target_reg, &label, io_mappings, mem_table, ctx);
                Self::push_raw(
                    instructions,
                    VmOpcode::Pid,
                    addr,
                    inst as u16,
                    region,
                    canonical,
                    sym,
                    format!("PID#{} CV={} PV={} SP={}", inst, el.target_reg, fb.in1, fb.in2),
                );
                Self::push_operand(instructions, &fb.in1, false, &label, io_mappings, mem_table, ctx);
                Self::push_operand(instructions, &fb.in2, false, &label, io_mappings, mem_table, ctx);
                if fb.k[3] <= 0 {
                    ctx.report.warn(format!("{}: OutMax <= 0 (CV is always 0)", label));
                }
                for k in fb.k {
                    Self::push_const_operand(instructions, k as i64, &label, ctx);
                }
            }
            TypeTag::ShiftRegisters => {
                let len = el.preset_value.clamp(1, 32);
                if len != el.preset_value {
                    ctx.report.warn(format!("{}: length {} clamped to 1..32", label, el.preset_value));
                }
                let slot = ctx.alloc_edge_slot(&label);
                let (addr, canonical, region, sym) = Self::word_dst(&el.name, &label, io_mappings, mem_table, ctx);
                if addr as usize + len as usize > 0x100 {
                    ctx.report.warn(format!("{}: register range exceeds word memory", label));
                }
                Self::push_raw(
                    instructions,
                    VmOpcode::Shreg,
                    addr,
                    (len as u16) | ((slot as u16) << 8),
                    region,
                    canonical,
                    sym,
                    format!("SHREG {} LEN={} <- {}", el.name, len, fb.in1),
                );
                Self::push_operand(instructions, &fb.in1, false, &label, io_mappings, mem_table, ctx);
            }
            TypeTag::ShiftRegInbit => {
                let slot = ctx.alloc_edge_slot(&label);
                let (addr, canonical, region, sym) = Self::word_dst(&el.name, &label, io_mappings, mem_table, ctx);
                Self::push_raw(
                    instructions,
                    VmOpcode::Shbit,
                    addr,
                    slot as u16,
                    region,
                    canonical,
                    sym,
                    format!("SHBIT {} <- {}", el.name, fb.in1),
                );
                Self::push_operand(instructions, &fb.in1, true, &label, io_mappings, mem_table, ctx);
            }
            TypeTag::ShiftRegReset => {
                let (addr, canonical, region, sym) = Self::word_dst(&el.name, &label, io_mappings, mem_table, ctx);
                Self::push_raw(
                    instructions,
                    VmOpcode::Move,
                    addr,
                    0,
                    region,
                    canonical,
                    sym,
                    format!("MOVE {}, K0 (SHIFT_RST)", el.name),
                );
            }
            TypeTag::ShiftLeft | TypeTag::ShiftRight | TypeTag::ShiftVariable => {
                let op = if el.element_type == TypeTag::ShiftRight { VmOpcode::Shr } else { VmOpcode::Shl };
                let slot = ctx.alloc_edge_slot(&label);
                let (addr, canonical, region, sym) = Self::word_dst(&el.name, &label, io_mappings, mem_table, ctx);
                Self::push_raw(
                    instructions,
                    op,
                    addr,
                    SHIFT_EDGE_FLAG | slot as u16,
                    region,
                    canonical.clone(),
                    sym.clone(),
                    format!("{} {} (rising edge)", op.mnemonic(), el.name),
                );
                let text = Self::opnd_text(OPND_KIND_WORD, addr as u16);
                Self::push_raw(
                    instructions,
                    VmOpcode::Opnd,
                    OPND_KIND_WORD,
                    addr as u16,
                    region,
                    text.clone(),
                    sym,
                    format!("  OPND {}", text),
                );
                if el.element_type == TypeTag::ShiftVariable {
                    Self::push_operand(instructions, &fb.in1, false, &label, io_mappings, mem_table, ctx);
                } else {
                    Self::push_const_operand(instructions, el.preset_value as i64, &label, ctx);
                }
            }
            TypeTag::UartSend => {
                let inst = CompileCtx::instance(
                    &mut ctx.uart_instances,
                    &mut ctx.report,
                    &el.name,
                    "UART_SEND",
                    uplc_vm::MAX_UART_INSTANCES,
                );
                if fb.mode > 1 {
                    ctx.report.warn(format!("{}: unknown mode {} (using ASCII)", label, fb.mode));
                }
                let mode = fb.mode.min(1);
                Self::push_raw(
                    instructions,
                    VmOpcode::Utx,
                    inst,
                    mode as u16,
                    MemoryRegion::None,
                    format!("U{}", inst),
                    el.symbol.clone(),
                    format!("UTX U{} <- {} ({})", inst, fb.in1, if mode == 1 { "ASCII" } else { "byte" }),
                );
                Self::push_operand(instructions, &fb.in1, false, &label, io_mappings, mem_table, ctx);
            }
            TypeTag::UartRecv => {
                let (addr, canonical, region, sym) = Self::word_dst(&el.target_reg, &label, io_mappings, mem_table, ctx);
                Self::push_raw(
                    instructions,
                    VmOpcode::Urx,
                    addr,
                    0,
                    region,
                    canonical,
                    sym,
                    format!("URX {}", el.target_reg),
                );
            }
            _ => {}
        }
    }

    fn compile_parallel_or_path(
        path: &[LadderElement],
        io_mappings: &[IOPortMapping],
        instructions: &mut Vec<BytecodeInstruction>,
        mem_table: &mut HashMap<(u8, bool), MemoryMapEntry>,
        ctx: &mut CompileCtx,
    ) {
        if path.is_empty() {
            return;
        }
        if path.len() == 1 && path[0].element_type == TypeTag::Contact {
            let sub = &path[0];
            let op = match sub.contact_mode {
                ContactMode::NormallyOpen => VmOpcode::Or,
                ContactMode::NormallyClosed => VmOpcode::Orn,
                ContactMode::PositiveTransition => VmOpcode::Orp,
                ContactMode::NegativeTransition => VmOpcode::Orf,
            };
            let src = format!("{} {}", op.mnemonic(), sub.name);
            Self::push_bit_ins(instructions, op, &sub.name, io_mappings, mem_table, 0, src);
        } else {
            Self::push_stack_ins(instructions, VmOpcode::OrLp);
            let mut p_first = true;
            for sub in path {
                Self::compile_element(
                    sub,
                    &mut p_first,
                    io_mappings,
                    instructions,
                    mem_table,
                    ctx,
                );
            }
            Self::push_stack_ins(instructions, VmOpcode::Rp);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_self_holding_timer_bytecode_compilation() {
        let il = r#"
PROGRAM MainProgram
VAR
    X0 AT %IX0.0 : BOOL; (* Pin: D2 | Symbol: START_PB | Start *)
    X1 AT %IX0.1 : BOOL; (* Pin: D3 | Symbol: STOP_PB | Stop *)
    Y0 AT %QX0.0 : BOOL; (* Pin: D8 | Symbol: MOTOR | Motor *)
    Y1 AT %QX0.1 : BOOL; (* Pin: D9 | Symbol: LAMP | Lamp *)
    T0 : TON; (* Preset: 2000ms *)
END_VAR

(* Network 01: Self-holding with TON *)
    LD   X0
    OR   Y0
    ANDN X1
    ST   Y0
    CAL  T0(IN := TRUE, PT := T#2000ms)
    LD   T0
    ST   Y1
END_PROGRAM
"#;
        let bc = BytecodeCompiler::compile_il(il).expect("Failed to compile IL to Bytecode");
        // Verify instruction count: LD X0, OR Y0, ANDN X1, ST Y0, TON T0 K2000, LD T0, ST Y1, END = 8 instructions
        assert_eq!(bc.instructions.len(), 8);
        assert_eq!(bc.payload_size_bytes(), 32);

        // 0: LD X0 -> 01 00 00 00
        assert_eq!(bc.instructions[0].to_bytes(), [0x01, 0x00, 0x00, 0x00]);
        // 1: OR Y0 -> 05 20 00 00
        assert_eq!(bc.instructions[1].to_bytes(), [0x05, 0x20, 0x00, 0x00]);
        // 2: ANDN X1 -> 04 01 00 00
        assert_eq!(bc.instructions[2].to_bytes(), [0x04, 0x01, 0x00, 0x00]);
        // 3: ST Y0 -> 07 20 00 00
        assert_eq!(bc.instructions[3].to_bytes(), [0x07, 0x20, 0x00, 0x00]);
        // 4: TON T0, K2000 -> 20 C0 D0 07 (0x07D0 = 2000 LE)
        assert_eq!(bc.instructions[4].to_bytes(), [0x20, 0xC0, 0xD0, 0x07]);
        // 5: LD T0 -> 01 C0 00 00
        assert_eq!(bc.instructions[5].to_bytes(), [0x01, 0xC0, 0x00, 0x00]);
        // 6: ST Y1 -> 07 21 00 00
        assert_eq!(bc.instructions[6].to_bytes(), [0x07, 0x21, 0x00, 0x00]);
        // 7: END -> FF 00 00 00
        assert_eq!(bc.instructions[7].to_bytes(), [0xFF, 0x00, 0x00, 0x00]);

        // Verify Serial Packet Framing (SOF 0xAA + CMD 0x01 + LEN 32 (0x20, 0x00) + 32B Payload + 1B CRC8 = 37 bytes)
        let pkt = bc.to_serial_packet(PlcSerialCommand::DownloadProgram);
        assert_eq!(pkt.len(), 37);
        assert_eq!(pkt[0], 0xAA);
        assert_eq!(pkt[1], 0x01);
        assert_eq!(pkt[2], 32);
        assert_eq!(pkt[3], 0);
    }

    #[test]
    fn test_compile_demo_program_and_disassembly() {
        let doc = crate::Document::default();
        let bc = doc.compile_to_bytecode();
        assert!(!bc.instructions.is_empty());
        assert_eq!(bc.instructions.last().unwrap().op, VmOpcode::End);
        assert_eq!(bc.to_raw_bytes().len(), bc.instructions.len() * 4);
        let disasm = bc.to_disassembly_text();
        assert!(disasm.contains("OPCODE"));
        assert!(disasm.contains("END"));
    }

    #[test]
    fn test_rp2040_demo_led_blink_example() {
        let il = include_str!("../../../example/08_rp2040_demo_led_blink.il");
        let bc = BytecodeCompiler::compile_il(il).expect("Failed to compile 08_rp2040_demo_led_blink.il");
        assert_eq!(bc.hardware, "Raspberry Pi Pico (RP2040)");
        assert_eq!(bc.instructions.last().unwrap().op, VmOpcode::End);

        // Verify LED_PIN (%QX0.0 -> Y0 -> Unified Address 0x20, GP25) is in memory map
        let y0_entry = bc
            .memory_map
            .iter()
            .find(|e| e.addr == 0x20)
            .expect("Y0 (0x20) should be in memory map");
        assert_eq!(y0_entry.canonical_name, "Y0");
        assert_eq!(y0_entry.iec_address, "%QX0.0");
        assert_eq!(y0_entry.hw_pin, "GP25");

        // Verify Serial Packet framing (SOF 0xAA + CMD 0x01 + LEN + Payload + CRC8)
        let pkt = bc.to_serial_packet(PlcSerialCommand::DownloadProgram);
        assert_eq!(pkt[0], PACKET_SOF);
        assert_eq!(pkt[1], PlcSerialCommand::DownloadProgram as u8);
        assert_eq!(pkt.len(), 5 + bc.payload_size_bytes());
    }

    #[test]
    fn test_online_monitoring_snapshot_and_branch_conduction() {
        let il = include_str!("../../../example/08_rp2040_demo_led_blink.il");
        let mut doc = crate::Document::default();
        doc.load_from_il_str(il).expect("Should load 08_rp2040_demo_led_blink.il");

        // Build a simulated hardware snapshot where T0 CV=320ms, %MX0.0=1 (BLINK_PHASE ON), %QX0.0=1 (LED_PIN ON)
        let mut bit_mem = [0u8; 256];
        bit_mem[0x40] = 1; // %MX0.0 = 1
        bit_mem[0x20] = 1; // %QX0.0 = 1
        let mut timer_cv = [0u32; 16];
        timer_cv[0] = 320; // T0 = 320ms
        timer_cv[1] = 180; // T1 = 180ms
        let mut data_reg = [0i32; 16];
        data_reg[0] = 42; // %MW0 = 42

        let bc = doc.compile_to_bytecode();
        let (expected_prog_len, expected_prog_crc) = bc.program_signature();

        let snap = PlcStatusSnapshot {
            running: true,
            scan_count: 12345,
            acc_bool: true,
            acc_word: 320,
            bit_mem,
            timer_cv,
            counter_cv: [0i32; 16],
            data_reg,
            prog_len: expected_prog_len,
            prog_crc: expected_prog_crc,
            fw_feature_level: 0,
        };

        // Round-trip serialize + parse 467-byte packet (462-byte payload with prog_len + prog_crc)
        let mut payload = [0u8; 462];
        payload[0] = u8::from(snap.running);
        payload[1..5].copy_from_slice(&snap.scan_count.to_le_bytes());
        payload[5] = u8::from(snap.acc_bool);
        payload[6..10].copy_from_slice(&snap.acc_word.to_le_bytes());
        payload[10..266].copy_from_slice(&snap.bit_mem);
        for i in 0..16 {
            let off = 266 + i * 4;
            payload[off..off + 4].copy_from_slice(&snap.timer_cv[i].to_le_bytes());
        }
        for i in 0..16 {
            let off = 330 + i * 4;
            payload[off..off + 4].copy_from_slice(&snap.counter_cv[i].to_le_bytes());
        }
        for i in 0..16 {
            let off = 394 + i * 4;
            payload[off..off + 4].copy_from_slice(&snap.data_reg[i].to_le_bytes());
        }
        payload[458..460].copy_from_slice(&snap.prog_len.to_le_bytes());
        payload[460..462].copy_from_slice(&snap.prog_crc.to_le_bytes());

        let mut packet = Vec::with_capacity(467);
        packet.push(0xAA);
        packet.push(0x83);
        packet.extend_from_slice(&462u16.to_le_bytes());
        packet.extend_from_slice(&payload);
        let crc = BytecodeProgram::compute_crc8(&packet[1..]);
        packet.push(crc);

        let parsed = PlcStatusSnapshot::parse_from_packet(&packet).expect("Packet should parse");
        assert_eq!(parsed.scan_count, 12345);
        assert!(parsed.running);
        assert!(parsed.acc_bool);
        assert_eq!(parsed.acc_word, 320);
        assert_eq!(parsed.prog_len, expected_prog_len);
        assert_eq!(parsed.prog_crc, expected_prog_crc);
        assert!(parsed.matches_program(&bc), "Snapshot signature should match compiled program");

        // Apply snapshot to document
        doc.apply_online_snapshot(&parsed);

        // Local simulator state must remain Stopped so it does NOT fight hardware polling,
        // while simulator.is_active() and remote_running are true!
        assert_eq!(doc.simulator.state, crate::SimState::Stopped);
        assert!(doc.simulator.is_active());
        assert!(doc.simulator.remote_running);
        assert_eq!(doc.simulator.registers.get("%MW0").copied(), Some(42));

        // Network 03 is: (LDN %IX0.1 ANDN %MX0.0) OR %IX0.0 -> ST %QX0.0
        // When %MX0.0 = 1 and %IX0.0 = 0, Network 04 (%MX0.0 -> %QX0.1) is energized,
        // and when %MX0.0 = 0 (first 500ms phase), Network 03's top branch path conducts!
        let net4 = &doc.program.rungs[3];
        assert!(net4.is_energized, "Network 04 should be energized when %MX0.0=1");

        // Now test snapshot during the ON phase of Network 03 (%MX0.0 = 0, %QX0.0 = 1)
        let mut snap_on = parsed.clone();
        snap_on.bit_mem[0x40] = 0; // %MX0.0 = 0
        doc.apply_online_snapshot(&snap_on);

        let net3 = &doc.program.rungs[2];
        assert!(net3.is_energized, "Network 03 should be energized when %MX0.0=0 and %IX0.1=0");
        assert_eq!(net3.elements[0].element_type, TypeTag::Contact);
        assert!(net3.elements[0].is_energized, "First contact (%IX0.1 NC) should conduct");
        let branch_el = &net3.elements[1];
        assert_eq!(branch_el.element_type, TypeTag::Branch);
        assert!(branch_el.is_energized, "Parallel branch in Network 03 should conduct");
        assert!(branch_el.branch_paths[0][0].is_energized, "Top branch (%MX0.0 NC) should be energized");
        assert!(!branch_el.branch_paths[1][0].is_energized, "Bottom branch (%IX0.0 NO) should be de-energized");

        // Verify live IL code with Accumulator annotations
        doc.selected_rung_idx = Some(0);
        let online_il = doc.generate_il_code_online();
        assert!(online_il.contains("ACC="));
        assert!(online_il.contains("ET=320ms/500ms"));
    }

    #[test]
    fn test_bytecode_decompile_to_ladder_and_packet_upload() {
        let prog = LadderProgram::new_demo_program();
        let bc = BytecodeCompiler::compile_program(&prog);
        assert!(!bc.instructions.is_empty());

        // 1. Emulate PLC responding with CMD 0x84 packet containing raw bytecode
        let raw_bytes = bc.to_raw_bytes();
        let payload_len = raw_bytes.len();
        let mut upload_frame = Vec::new();
        upload_frame.push(PACKET_SOF); // 0xAA
        upload_frame.push(0x84);       // CMD_UPLOAD_PROGRAM_RESPONSE
        let [l_lo, l_hi] = (payload_len as u16).to_le_bytes();
        upload_frame.push(l_lo);
        upload_frame.push(l_hi);
        upload_frame.extend_from_slice(&raw_bytes);
        let crc = BytecodeProgram::compute_crc8(&upload_frame[1..]);
        upload_frame.push(crc);

        // 2. Parse upload frame on IDE side
        let uploaded_bc = BytecodeProgram::parse_upload_packet(&upload_frame, &prog.hardware).unwrap();
        assert_eq!(uploaded_bc.instructions.len(), bc.instructions.len());
        assert_eq!(uploaded_bc.to_raw_bytes(), raw_bytes);

        // 3. Decompile back into a LadderProgram
        let decompiled_ladder = uploaded_bc.decompile_to_ladder().unwrap();
        assert!(!decompiled_ladder.rungs.is_empty());
        assert_eq!(decompiled_ladder.hardware, prog.hardware);

        // Verify Network 01: Has start/stop logic with contacts and output coil
        let r0 = &decompiled_ladder.rungs[0];
        assert!(r0.elements.iter().any(|e| e.element_type == TypeTag::Coil));
        assert!(r0.elements.iter().any(|e| e.element_type == TypeTag::Contact || e.element_type == TypeTag::Branch));
    }

    #[test]
    fn test_rp2040_demo_led_blink_decompile_recompile_signature_match() {
        let il = include_str!("../../../example/08_rp2040_demo_led_blink.il");
        let mut doc = crate::Document::default();
        doc.load_from_il_str(il).expect("Should load 08_rp2040_demo_led_blink.il");
        let bc = doc.compile_to_bytecode();
        let decompiled_prog = bc.decompile_to_ladder().unwrap();
        let recompiled_bc = BytecodeCompiler::compile_program(&decompiled_prog);
        println!("Original BC len={}, crc=0x{:04X}", bc.instructions.len(), bc.program_signature().1);
        println!("Recompiled BC len={}, crc=0x{:04X}", recompiled_bc.instructions.len(), recompiled_bc.program_signature().1);
        for (i, ins) in bc.instructions.iter().enumerate() {
            let re_ins = recompiled_bc.instructions.get(i);
            println!("  [{:02}] Orig: {:?} {} {} | Re: {:?}", i, ins.op, ins.operand_name, ins.imm, re_ins.map(|r| (r.op, &r.operand_name, r.imm)));
        }
        if recompiled_bc.instructions.len() > bc.instructions.len() {
            for i in bc.instructions.len()..recompiled_bc.instructions.len() {
                let re_ins = &recompiled_bc.instructions[i];
                println!("  [{:02}] Extra Re: {:?} {} {}", i, re_ins.op, re_ins.operand_name, re_ins.imm);
            }
        }
        assert_eq!(bc.program_signature(), recompiled_bc.program_signature());

        // Also verify end-to-end through raw upload packet serialization
        let mut upload_frame = vec![0xAA, 0x84, 64, 0];
        upload_frame.extend_from_slice(&bc.to_raw_bytes());
        let crc = BytecodeProgram::compute_crc8(&upload_frame[1..]);
        upload_frame.push(crc);

        let uploaded_bc = BytecodeProgram::parse_upload_packet(&upload_frame, "Raspberry Pi Pico (RP2040)").unwrap();
        let decompiled_uploaded = uploaded_bc.decompile_to_ladder().unwrap();
        let recompiled_uploaded_bc = BytecodeCompiler::compile_program(&decompiled_uploaded);
        assert_eq!(recompiled_uploaded_bc.program_signature(), (16, 0xD7F1));
    }

    const FB_IL: &str = r#"
PROGRAM FbDemo
VAR
    TEMP_PID : PID;
END_VAR
(* Network 01: analog *)
    LD   %IX0.0
    ADC  %IW0 %MW1
    CAL  SC1(IN := %MW1, OUT => %MW2, IN_MIN := 0, IN_MAX := 4095, OUT_MIN := -500, OUT_MAX := 1500)
    CAL  LIM1(IN := %MW2, OUT => %MW3, MN := 0, MX := 1000)
(* Network 02: bitwise *)
    LD   %IX0.1
    WAND %MW4 %MW1 16#0F0F
    WOR  %MW5 %MW4 %MW3
    WXOR %MW6 %MW5 -1
    WNOT %MW7 %MW6
    SHL  %MW8 %MW4 3
    SHR  %MW9 %MW1 2
    ST   %QX0.0
(* Network 03: shift *)
    LD   %IX0.2
    SHIFT_REG %MW20 5 %MW3
    SHIFT_BIT %MW30 %IX0.3
    SHIFT_L %MW31 1
    SHIFT_R %MW32 2
    SHIFT_VAR %MW33 %MW34
(* Network 04: reset *)
    LD   %IX0.3
    SHIFT_RST %MW30
    MOVE %MW34 -2
(* Network 05: pid *)
    LD   %IX0.2
    CAL  TEMP_PID(PV := %MW40, SP := %MW41, CV => %MW42, KP := 150, KI := 50, KD := 10, OUTMAX := 1000)
(* Network 06: uart *)
    LD   %IX0.0
    UART_SEND U1(IN := %MW3, MODE := 1)
(* Network 07: uart rx *)
    LD   %IX0.1
    UART_RECV U2(OUT => %MW50)
    ST   %QX0.1
END_PROGRAM
"#;

    fn flatten(prog: &LadderProgram) -> Vec<LadderElement> {
        fn walk(els: &[LadderElement], out: &mut Vec<LadderElement>) {
            for e in els {
                if e.element_type == TypeTag::Branch {
                    for p in &e.branch_paths {
                        walk(p, out);
                    }
                } else {
                    out.push(e.clone());
                }
            }
        }
        let mut out = Vec::new();
        for r in &prog.rungs {
            walk(&r.elements, &mut out);
        }
        out
    }

    #[test]
    fn test_new_opcodes_encode_decode_roundtrip() {
        let (bc, report) = BytecodeCompiler::compile_il_with_report(FB_IL).unwrap();
        assert!(report.is_ok(), "unexpected errors: {:?}", report.errors);
        assert!(report.warnings.is_empty(), "unexpected warnings: {:?}", report.warnings);
        assert_eq!(bc.required_feature_level(), 2);

        // Every header opcode is followed by exactly operand_words() OPND words
        let mut i = 0;
        while i < bc.instructions.len() {
            let n = bc.instructions[i].op.operand_words();
            for j in 1..=n {
                assert_eq!(bc.instructions[i + j].op, VmOpcode::Opnd, "operand {} of {:?}", j, bc.instructions[i].op);
            }
            i += n + 1;
        }

        // Spot-check encodings
        let find = |op: VmOpcode| bc.instructions.iter().position(|x| x.op == op).unwrap();
        let movw = find(VmOpcode::Movw);
        assert_eq!(bc.instructions[movw].to_bytes(), [0x46, 0x21, 0, 0]); // dst %MW1 = 0x21
        assert_eq!(bc.instructions[movw + 1].to_bytes(), [0x6F, 0, 0x00, 0]); // %IW0
        let scale = find(VmOpcode::Scale);
        assert_eq!(bc.instructions[scale + 4].to_bytes(), [0x6F, 1, 0x0C, 0xFE]); // -500
        let wxor = find(VmOpcode::Wxor);
        assert_eq!(bc.instructions[wxor + 2].to_bytes(), [0x6F, 1, 0xFF, 0xFF]); // -1
        let shreg = find(VmOpcode::Shreg);
        assert_eq!(bc.instructions[shreg].imm & 0xFF, 5);
        let shbit = find(VmOpcode::Shbit);
        assert_eq!(bc.instructions[shbit + 1].to_bytes(), [0x6F, 2, 0x03, 0]); // %IX0.3 bit
        let pid = find(VmOpcode::Pid);
        assert_eq!(bc.instructions[pid].addr, 0x20 + 42);
        let movw_neg = bc.instructions.iter().rposition(|x| x.op == VmOpcode::Movw).unwrap();
        assert_eq!(bc.instructions[movw_neg + 1].to_bytes(), [0x6F, 1, 0xFE, 0xFF]); // MOVE %MW34 -2
        let utx = find(VmOpcode::Utx);
        assert_eq!(bc.instructions[utx].to_bytes(), [0x74, 0, 1, 0]);

        // Raw bytes -> BytecodeProgram -> raw bytes is lossless
        let raw = bc.to_raw_bytes();
        let decoded = BytecodeProgram::from_raw_bytes(&raw, &bc.hardware).unwrap();
        assert_eq!(decoded.to_raw_bytes(), raw);
        assert!(decoded.to_disassembly_text().contains("OPND"));

        // Decompile -> ladder -> recompile gives the identical program
        let ladder = decoded.decompile_to_ladder().unwrap();
        let recompiled = BytecodeCompiler::compile_program(&ladder);
        assert_eq!(recompiled.to_raw_bytes(), raw, "decompiled IL:\n{}", decoded.decompile_to_il());
    }

    #[test]
    fn test_decompile_to_ladder_restores_fb_elements() {
        let original = IlParser::parse(FB_IL).unwrap();
        let bc = BytecodeCompiler::compile_program(&original);
        let uploaded = BytecodeProgram::from_raw_bytes(&bc.to_raw_bytes(), "Raspberry Pi Pico (RP2040)").unwrap();
        let ladder = uploaded.decompile_to_ladder().unwrap();

        let orig: Vec<_> = flatten(&original).into_iter().filter(|e| e.element_type != TypeTag::Contact && e.element_type != TypeTag::Coil).collect();
        let back: Vec<_> = flatten(&ladder).into_iter().filter(|e| e.element_type != TypeTag::Contact && e.element_type != TypeTag::Coil).collect();
        assert_eq!(orig.len(), back.len());
        for (a, b) in orig.iter().zip(back.iter()) {
            if a.element_type == TypeTag::ShiftRegReset {
                // ShiftRegReset compiles to `MOVE w 0` per spec, so decompilation yields Move(0)
                assert_eq!(b.element_type, TypeTag::Move);
                assert_eq!(b.preset_value, 0);
                continue;
            }
            assert_eq!(a.element_type, b.element_type);
            assert_eq!(a.fb.k, b.fb.k, "{:?}", a.element_type);
            assert_eq!(a.fb.mode, b.fb.mode);
            assert_eq!(a.preset_value, b.preset_value, "{:?}", a.element_type);
            if a.element_type == TypeTag::Bitwise {
                assert_eq!(a.math_op, b.math_op);
            }
        }
    }

    #[test]
    fn test_compile_report_const_range_and_pid_limit() {
        let mut prog = LadderProgram::new_empty();
        prog.rungs.clear();
        let mut rung = Rung::new(0);
        rung.elements.push(LadderElement::new_contact(1, "%IX0.0", "IN", ContactMode::NormallyOpen));
        let mut sc = LadderElement {
            id: 2,
            element_type: TypeTag::Scale,
            name: "SC".into(),
            target_reg: "%MW1".into(),
            ..Default::default()
        };
        sc.fb.in1 = "%IW0".into();
        sc.fb.k = [0, 40000, 0, 1000]; // 40000 > i16::MAX
        rung.elements.push(sc);
        prog.rungs.push(rung);
        for i in 0..9 {
            let mut r = Rung::new(i + 1);
            r.elements.push(LadderElement::new_contact(100 + i as u64, "%IX0.1", "EN", ContactMode::NormallyOpen));
            let mut pid = LadderElement {
                id: 200 + i as u64,
                element_type: TypeTag::Pid,
                name: format!("PID_{}", i),
                target_reg: format!("%MW{}", 10 + i),
                ..Default::default()
            };
            pid.fb.in1 = "%MW0".into();
            pid.fb.in2 = "500".into();
            pid.fb.k = [100, 10, 0, 1000];
            r.elements.push(pid);
            prog.rungs.push(r);
        }
        let (_, report) = BytecodeCompiler::compile_program_with_report(&prog);
        assert!(!report.is_ok());
        assert!(report.errors.iter().any(|e| e.contains("40000") && e.contains("i16")), "{:?}", report.errors);
        assert!(report.errors.iter().any(|e| e.contains("more than 8 PID")), "{:?}", report.errors);

        // Legacy-only programs need feature level 1
        let legacy = BytecodeCompiler::compile_program(&LadderProgram::new_demo_program());
        assert_eq!(legacy.required_feature_level(), 1);
    }

    #[test]
    fn test_fb_at_rung_start_gets_always_on_condition() {
        let il = "PROGRAM P\n(* Network 01: x *)\n    CAL  SC1(IN := %IW0, OUT => %MW2, IN_MIN := 0, IN_MAX := 100, OUT_MIN := 0, OUT_MAX := 10)\nEND_PROGRAM\n";
        let bc = BytecodeCompiler::compile_il(il).unwrap();
        assert_eq!(bc.instructions[0].op, VmOpcode::Ld);
        assert_eq!(bc.instructions[0].addr, 0xA0, "LD %SM0 (always ON)");
        assert_eq!(bc.instructions[1].op, VmOpcode::Scale);
    }

    #[test]
    fn test_persist_commands_and_feature_level_snapshot() {
        let bc = BytecodeProgram { hardware: String::new(), instructions: Vec::new(), memory_map: Vec::new() };
        let save = bc.to_serial_packet(PlcSerialCommand::PersistSave);
        assert_eq!(&save[..4], &[0xAA, 0x08, 0x00, 0x00]);
        assert_eq!(save[4], BytecodeProgram::compute_crc8(&save[1..4]));
        let clear = bc.to_serial_packet(PlcSerialCommand::PersistClear);
        assert_eq!(&clear[..4], &[0xAA, 0x09, 0x00, 0x00]);

        // New VM status packet: acc byte carries the feature level in the high nibble
        let mut vm = uplc_vm::MicroVm::new();
        vm.acc_bool = true;
        let mut buf = [0u8; uplc_vm::STATUS_PACKET_LEN];
        let n = vm.write_status_packet(&mut buf).unwrap();
        let snap = PlcStatusSnapshot::parse_from_packet(&buf[..n]).unwrap();
        assert!(snap.acc_bool);
        assert_eq!(snap.fw_feature_level, uplc_vm::FW_FEATURE_LEVEL);
        assert!(snap.firmware_supports(2));

        // Legacy firmware (acc byte 0/1, 458-byte payload) -> level 0, supports only level 1
        let mut legacy = vec![0xAA, 0x83];
        legacy.extend_from_slice(&(PlcStatusSnapshot::LEGACY_PAYLOAD_LEN as u16).to_le_bytes());
        let mut payload = vec![0u8; PlcStatusSnapshot::LEGACY_PAYLOAD_LEN];
        payload[5] = 1;
        legacy.extend_from_slice(&payload);
        let crc = BytecodeProgram::compute_crc8(&legacy[1..]);
        legacy.push(crc);
        let snap = PlcStatusSnapshot::parse_from_packet(&legacy).unwrap();
        assert!(snap.acc_bool);
        assert_eq!(snap.fw_feature_level, 0);
        assert!(snap.firmware_supports(1));
        assert!(!snap.firmware_supports(2));
    }
}

