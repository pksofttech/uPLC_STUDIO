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

pub const MAX_INSTRUCTIONS: usize = 512;
pub const MAX_STACK_DEPTH: usize = 16;
pub const PACKET_SOF: u8 = 0xAA;

pub const CMD_DOWNLOAD_PROGRAM: u8 = 0x01;
pub const CMD_RUN_STOP: u8 = 0x02;
pub const CMD_MONITOR_STATUS: u8 = 0x03;
pub const CMD_STATUS_RESPONSE: u8 = 0x83;
pub const CMD_UPLOAD_PROGRAM: u8 = 0x04;
pub const CMD_UPLOAD_PROGRAM_RESPONSE: u8 = 0x84;

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
    /// Previous scan bit state for rising/falling edge detection (`LDP`, `LDF`, `R_TRIG`, etc.)
    pub prev_bit_mem: [u8; 256],
    /// Unified Word Memory (`AI`, `AO`, `D`, `T-cv`, `C-cv`)
    pub word_mem: [i32; 256],
    /// Previous rung input state for counters (`CTU`, `CTD`, `CTUD`)
    pub ctu_prev: [u8; 32],
    /// Previous rung input state for `STP` / `STF` / `R_TRIG` / `F_TRIG`
    pub edge_prev: [u8; 256],
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
            prev_bit_mem: [0; 256],
            word_mem: [0; 256],
            ctu_prev: [0; 32],
            edge_prev: [0; 256],
        }
    }

    /// Reset all outputs, internal relays, timers, and counters (Safe Shutdown)
    pub fn reset_memory(&mut self) {
        self.bit_mem = [0; 256];
        self.prev_bit_mem = [0; 256];
        self.word_mem = [0; 256];
        self.ctu_prev = [0; 32];
        self.edge_prev = [0; 256];
        self.acc_bool = false;
        self.acc_word = 0;
        self.scan_count = 0;
    }

    /// Load raw 32-bit aligned bytecode payload (`4 * N` bytes) into VM program memory
    pub fn load_bytecode(&mut self, payload: &[u8]) -> Result<u16, &'static str> {
        if payload.len() % 4 != 0 {
            return Err("Payload length must be a multiple of 4 bytes");
        }
        let count = payload.len() / 4;
        if count > MAX_INSTRUCTIONS {
            return Err("Program exceeds MAX_INSTRUCTIONS (512)");
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
            self.bit_mem[x_index as usize] = u8::from(state);
        }
    }

    #[inline]
    pub fn get_digital_output(&self, y_index: u8) -> bool {
        if y_index < 32 {
            self.bit_mem[(0x20 + y_index) as usize] != 0
        } else {
            false
        }
    }

    /// Execute 1 full PLC Scan Cycle with elapsed time `dt_ms` (in milliseconds)
    pub fn step_scan(&mut self, dt_ms: u32) {
        if !self.running || self.prog_len == 0 {
            return;
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
                        if self.word_mem[addr] >= ins.imm as i32 {
                            self.bit_mem[addr] = 1;
                        }
                    }
                    self.ctu_prev[c_idx] = u8::from(acc && mc_active);
                    self.acc_word = self.word_mem[addr];
                    acc = self.bit_mem[addr] != 0;
                }
                0x24 => {
                    // CTD (Counter Down)
                    let c_idx = addr.saturating_sub(0xE0) & 0x1F;
                    let prev_cd = self.ctu_prev[c_idx] != 0;
                    if acc && mc_active && !prev_cd {
                        self.word_mem[addr] = self.word_mem[addr].saturating_sub(1);
                        if self.word_mem[addr] <= 0 {
                            self.bit_mem[addr] = 1;
                        }
                    }
                    self.ctu_prev[c_idx] = u8::from(acc && mc_active);
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
                0xFF => break, // END
                _ => {}
            }

            pc += 1;
        }

        self.acc_bool = acc;
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

        out[idx] = if self.acc_bool { 1 } else { 0 };
        idx += 1;

        out[idx..idx + 4].copy_from_slice(&self.acc_word.to_le_bytes());
        idx += 4;

        out[idx..idx + 256].copy_from_slice(&self.bit_mem);
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

        // Data Registers D0..D15 / %MW0..%MW15 (word_mem[0..16])
        for i in 0..16 {
            let val = self.word_mem[i];
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
        assert_eq!(buf[9], 1); // acc_bool = true
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
}

