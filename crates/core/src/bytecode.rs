use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::il_format::IlParser;
use crate::ladder::{CoilMode, ContactMode, IOPortMapping, LadderElement, LadderProgram, Rung, TypeTag};

/// Start of Frame byte for Serial Packet Protocol
pub const PACKET_SOF: u8 = 0xAA;

/// Serial Command IDs for RP2040 Micro-VM
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum PlcSerialCommand {
    /// 0x01: Download Bytecode Program to RAM / Flash
    DownloadProgram = 0x01,
    /// 0x02: PLC RUN / STOP Control
    RunStopControl = 0x02,
    /// 0x03: Request Online Status Monitor (bit_mem & word_mem)
    MonitorStatus = 0x03,
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
    Jmpc = 0x50,
    Lbl = 0x51,
    End = 0xFF,
}

impl VmOpcode {
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
            Self::Jmpc => "JMPC",
            Self::Lbl => "LBL",
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
            Self::Jmpc => "if (acc) pc = imm",
            Self::Lbl => "Jump Target Label",
            Self::End => "End of Scan Cycle (Return)",
        }
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

    /// Build a complete Serial/UART Packet Frame:
    /// `[SOF (0xAA)] [CMD (1B)] [LEN_LO (1B)] [LEN_HI (1B)] [PAYLOAD (4*N B)] [CRC8 (1B)]`
    pub fn to_serial_packet(&self, cmd: PlcSerialCommand) -> Vec<u8> {
        let payload = self.to_raw_bytes();
        let len = (payload.len() as u16).to_le_bytes();
        let mut packet = Vec::with_capacity(5 + payload.len());
        packet.push(PACKET_SOF);
        packet.push(cmd as u8);
        packet.push(len[0]);
        packet.push(len[1]);
        packet.extend_from_slice(&payload);
        // Compute CRC8 over CMD + LENGTH + PAYLOAD
        let crc = Self::compute_crc8(&packet[1..]);
        packet.push(crc);
        packet
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
        out.push_str("// uPLC Studio - Micro-VM 32-bit Bytecode Disassembly & Unified Memory Map\n");
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
        let mut instructions: Vec<BytecodeInstruction> = Vec::new();
        let mut mem_table: HashMap<(u8, bool), MemoryMapEntry> = HashMap::new();
        let mut label_pcs: HashMap<String, u16> = HashMap::new();
        let mut pending_jumps: Vec<(usize, String)> = Vec::new();

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
                &mut label_pcs,
                &mut pending_jumps,
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
        for (ins_idx, lbl_name) in pending_jumps {
            if let Some(&target_pc) = label_pcs.get(&lbl_name.to_uppercase()) {
                if let Some(ins) = instructions.get_mut(ins_idx) {
                    ins.imm = target_pc;
                }
            }
        }

        let mut memory_map: Vec<MemoryMapEntry> = mem_table.into_values().collect();
        memory_map.sort_by_key(|e| (Self::region_sort_order(e.region), e.addr));

        BytecodeProgram {
            hardware: prog.hardware.clone(),
            instructions,
            memory_map,
        }
    }

    /// Compile an IEC 61131-3 IL text string into a `BytecodeProgram`
    pub fn compile_il(il_text: &str) -> Result<BytecodeProgram, String> {
        let prog = IlParser::parse(il_text)?;
        Ok(Self::compile_program(&prog))
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
        label_pcs: &mut HashMap<String, u16>,
        pending_jumps: &mut Vec<(usize, String)>,
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
                label_pcs,
                pending_jumps,
            );
        }
    }

    fn compile_element(
        el: &LadderElement,
        first: &mut bool,
        io_mappings: &[IOPortMapping],
        instructions: &mut Vec<BytecodeInstruction>,
        mem_table: &mut HashMap<(u8, bool), MemoryMapEntry>,
        label_pcs: &mut HashMap<String, u16>,
        pending_jumps: &mut Vec<(usize, String)>,
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
                                label_pcs,
                                pending_jumps,
                            );
                        }
                    }
                    for path in el.branch_paths.iter().skip(1) {
                        Self::compile_parallel_or_path(
                            path,
                            io_mappings,
                            instructions,
                            mem_table,
                            label_pcs,
                            pending_jumps,
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
                                label_pcs,
                                pending_jumps,
                            );
                        }
                    }
                    for path in el.branch_paths.iter().skip(1) {
                        Self::compile_parallel_or_path(
                            path,
                            io_mappings,
                            instructions,
                            mem_table,
                            label_pcs,
                            pending_jumps,
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
                let imm = el.preset_value.clamp(0, 65535) as u16;
                let src = format!("{} {}, K{}", op.mnemonic(), el.name, imm);
                Self::push_word_ins(instructions, op, &el.name, io_mappings, mem_table, imm, src);
                *first = false;
            }
            TypeTag::Move => {
                let imm = el.preset_value.clamp(0, 65535) as u16;
                let src = format!("MOVE {}, K{}", el.name, imm);
                Self::push_word_ins(
                    instructions,
                    VmOpcode::Move,
                    &el.name,
                    io_mappings,
                    mem_table,
                    imm,
                    src,
                );
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
                let imm = el.preset_value.clamp(0, 65535) as u16;
                let src = format!("{} {}, K{}", op.mnemonic(), el.name, imm);
                Self::push_word_ins(instructions, op, &el.name, io_mappings, mem_table, imm, src);
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
                label_pcs.insert(el.name.to_uppercase(), pc);
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
                pending_jumps.push((idx, el.name.clone()));
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
            _ => {}
        }
    }

    fn compile_parallel_or_path(
        path: &[LadderElement],
        io_mappings: &[IOPortMapping],
        instructions: &mut Vec<BytecodeInstruction>,
        mem_table: &mut HashMap<(u8, bool), MemoryMapEntry>,
        label_pcs: &mut HashMap<String, u16>,
        pending_jumps: &mut Vec<(usize, String)>,
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
                    label_pcs,
                    pending_jumps,
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
}
