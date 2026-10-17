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
    pub running: bool,
    pub scan_count: u32,

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
            running: true,
            scan_count: 0,
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
                    } else {
                        self.word_mem[addr] = 0;
                        self.bit_mem[addr] = 0;
                    }
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
                    }
                }
                0x23 => {
                    // TP (Pulse Timer)
                    let pt = ins.imm as i32;
                    if acc && mc_active && self.word_mem[addr] == 0 && self.bit_mem[addr] == 0 {
                        self.bit_mem[addr] = 1;
                        self.word_mem[addr] = dt_ms as i32;
                    } else if self.bit_mem[addr] != 0 {
                        let next_cv = self.word_mem[addr].saturating_add(dt_ms as i32);
                        if next_cv >= pt {
                            self.word_mem[addr] = pt;
                            self.bit_mem[addr] = 0;
                        } else {
                            self.word_mem[addr] = next_cv;
                        }
                    } else if !acc {
                        self.word_mem[addr] = 0;
                    }
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
                }

                // Comparisons
                0x30 => acc = acc && (self.word_mem[addr] == ins.imm as i32), // EQ
                0x31 => acc = acc && (self.word_mem[addr] != ins.imm as i32), // NE
                0x32 => acc = acc && (self.word_mem[addr] > ins.imm as i32),  // GT
                0x33 => acc = acc && (self.word_mem[addr] < ins.imm as i32),  // LT
                0x34 => acc = acc && (self.word_mem[addr] >= ins.imm as i32), // GE
                0x35 => acc = acc && (self.word_mem[addr] <= ins.imm as i32), // LE

                // Word Move & Math
                0x40 => {
                    if acc && mc_active {
                        self.word_mem[addr] = ins.imm as i32;
                    }
                }
                0x41 => {
                    if acc && mc_active {
                        self.word_mem[addr] = self.word_mem[addr].saturating_add(ins.imm as i32);
                    }
                }
                0x42 => {
                    if acc && mc_active {
                        self.word_mem[addr] = self.word_mem[addr].saturating_sub(ins.imm as i32);
                    }
                }
                0x43 => {
                    if acc && mc_active {
                        self.word_mem[addr] = self.word_mem[addr].saturating_mul(ins.imm as i32);
                    }
                }
                0x44 => {
                    if acc && mc_active && ins.imm != 0 {
                        self.word_mem[addr] /= ins.imm as i32;
                    }
                }
                0x45 => {
                    if acc && mc_active && ins.imm != 0 {
                        self.word_mem[addr] %= ins.imm as i32;
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

        self.prev_bit_mem = self.bit_mem;
        self.scan_count = self.scan_count.wrapping_add(1);
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
                self.running = !self.running;
                if !self.running {
                    // Safe output shutdown on STOP
                    for y in 0x20..=0x3F {
                        self.bit_mem[y] = 0;
                    }
                }
                Ok(CMD_RUN_STOP)
            }
            CMD_MONITOR_STATUS => Ok(CMD_MONITOR_STATUS),
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
}
