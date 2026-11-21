//! # Modbus RTU Protocol & Master Engine (`modbus.rs`)
//!
//! Provides `#![no_std]` compliant Modbus RTU framing, CRC-16 computation,
//! job dispatching, and response decoding for the Micro-VM.

/// Maximum concurrent Modbus Master Function Block instances supported by the VM
pub const MAX_MB_MASTER_INSTANCES: usize = 8;

/// Modbus Function Codes
pub const MB_FC_READ_COILS: u8 = 0x01;
pub const MB_FC_READ_DISCRETE_INPUTS: u8 = 0x02;
pub const MB_FC_READ_HOLDING_REGISTERS: u8 = 0x03;
pub const MB_FC_READ_INPUT_REGISTERS: u8 = 0x04;
pub const MB_FC_WRITE_SINGLE_COIL: u8 = 0x05;
pub const MB_FC_WRITE_SINGLE_REGISTER: u8 = 0x06;
pub const MB_FC_WRITE_MULTIPLE_COILS: u8 = 0x0F;
pub const MB_FC_WRITE_MULTIPLE_REGISTERS: u8 = 0x10;

/// Standard Modbus Exception Codes (Slave Responses)
pub const MB_EX_ILLEGAL_FUNCTION: u8 = 0x01;
pub const MB_EX_ILLEGAL_DATA_ADDRESS: u8 = 0x02;
pub const MB_EX_ILLEGAL_DATA_VALUE: u8 = 0x03;

/// Standard Modbus Error Codes returned in Function Block `STATUS`
pub const MB_ERR_OK: u16 = 0;
pub const MB_ERR_TIMEOUT: u16 = 1;
pub const MB_ERR_CRC: u16 = 2;
pub const MB_ERR_EXCEPTION: u16 = 3;
pub const MB_ERR_BUSY: u16 = 4;
pub const MB_ERR_INVALID_PARAM: u16 = 5;

/// Communication Job dispatched by the VM to the background Modbus driver task
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ModbusMasterJob {
    pub job_id: u8,
    pub instance_id: u8,
    pub slave_id: u8,
    pub function_code: u8,
    pub reg_addr: u16,
    pub count: u16,
    pub buffer_addr: u8,
    pub is_write: bool,
}

/// Completion result returned by the background Modbus driver task to the VM
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ModbusJobResult {
    pub job_id: u8,
    pub instance_id: u8,
    pub success: bool,
    pub error_code: u16,
}

/// IEC 61131-3 MB_MASTER Function Block Instance State
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ModbusMasterState {
    pub prev_req: bool,
    pub busy: bool,
    pub done: bool,
    pub error: bool,
    pub status: u16,
    pub job_id: u8,
}

/// Calculate standard Modbus RTU CRC-16 (Polynomial 0xA001, Init 0xFFFF)
pub fn compute_crc16(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &byte in data {
        crc ^= byte as u16;
        for _ in 0..8 {
            if (crc & 0x0001) != 0 {
                crc = (crc >> 1) ^ 0xA001;
            } else {
                crc >>= 1;
            }
        }
    }
    crc
}

/// Encode a Modbus RTU request frame into `out_buf` with CRC-16 appended.
/// Returns the number of bytes written.
pub fn encode_rtu_request(
    job: &ModbusMasterJob,
    word_mem: &[i32; 256],
    bit_mem: &[u8; 256],
    out_buf: &mut [u8],
) -> Result<usize, &'static str> {
    if out_buf.len() < 8 {
        return Err("Output buffer too small for Modbus frame");
    }

    out_buf[0] = job.slave_id;
    out_buf[1] = job.function_code;
    out_buf[2] = (job.reg_addr >> 8) as u8;
    out_buf[3] = (job.reg_addr & 0xFF) as u8;

    let total_len = match job.function_code {
        MB_FC_READ_COILS
        | MB_FC_READ_DISCRETE_INPUTS
        | MB_FC_READ_HOLDING_REGISTERS
        | MB_FC_READ_INPUT_REGISTERS => {
            out_buf[4] = (job.count >> 8) as u8;
            out_buf[5] = (job.count & 0xFF) as u8;
            6
        }
        MB_FC_WRITE_SINGLE_COIL => {
            let val = bit_mem[job.buffer_addr as usize] != 0;
            out_buf[4] = if val { 0xFF } else { 0x00 };
            out_buf[5] = 0x00;
            6
        }
        MB_FC_WRITE_SINGLE_REGISTER => {
            let val = word_mem[job.buffer_addr as usize] as u16;
            out_buf[4] = (val >> 8) as u8;
            out_buf[5] = (val & 0xFF) as u8;
            6
        }
        MB_FC_WRITE_MULTIPLE_COILS => {
            let count = job.count as usize;
            let byte_count = count.div_ceil(8);
            if out_buf.len() < 7 + byte_count + 2 {
                return Err("Output buffer too small for Write Multiple Coils");
            }
            out_buf[4] = (job.count >> 8) as u8;
            out_buf[5] = (job.count & 0xFF) as u8;
            out_buf[6] = byte_count as u8;

            for b in &mut out_buf[7..7 + byte_count] {
                *b = 0;
            }
            for i in 0..count {
                let mem_idx = (job.buffer_addr as usize + i) & 0xFF;
                if bit_mem[mem_idx] != 0 {
                    out_buf[7 + i / 8] |= 1 << (i % 8);
                }
            }
            7 + byte_count
        }
        MB_FC_WRITE_MULTIPLE_REGISTERS => {
            let count = job.count as usize;
            let byte_count = count * 2;
            if out_buf.len() < 7 + byte_count + 2 {
                return Err("Output buffer too small for Write Multiple Registers");
            }
            out_buf[4] = (job.count >> 8) as u8;
            out_buf[5] = (job.count & 0xFF) as u8;
            out_buf[6] = byte_count as u8;

            let mut idx = 7;
            for i in 0..count {
                let mem_idx = (job.buffer_addr as usize + i) & 0xFF;
                let val = word_mem[mem_idx] as u16;
                out_buf[idx] = (val >> 8) as u8;
                out_buf[idx + 1] = (val & 0xFF) as u8;
                idx += 2;
            }
            idx
        }
        _ => return Err("Unsupported Modbus function code"),
    };

    // Append CRC-16 (Little Endian: Low byte first, then High byte)
    let crc = compute_crc16(&out_buf[..total_len]);
    let [c_lo, c_hi] = crc.to_le_bytes();
    out_buf[total_len] = c_lo;
    out_buf[total_len + 1] = c_hi;

    Ok(total_len + 2)
}

/// Decode a received Modbus RTU response frame, verify CRC-16,
/// and update PLC memory (`word_mem` or `bit_mem`) accordingly.
pub fn decode_rtu_response(
    job: &ModbusMasterJob,
    resp: &[u8],
    word_mem: &mut [i32; 256],
    bit_mem: &mut [u8; 256],
) -> Result<(), u16> {
    if resp.len() < 5 {
        return Err(MB_ERR_TIMEOUT);
    }

    // Verify CRC-16
    let len = resp.len();
    let expected_crc = u16::from_le_bytes([resp[len - 2], resp[len - 1]]);
    let actual_crc = compute_crc16(&resp[..len - 2]);
    if actual_crc != expected_crc {
        return Err(MB_ERR_CRC);
    }

    // Verify Slave ID
    if resp[0] != job.slave_id {
        return Err(MB_ERR_INVALID_PARAM);
    }

    // Check for Modbus Exception Response (Function Code | 0x80)
    if resp[1] == (job.function_code | 0x80) {
        return Err(MB_ERR_EXCEPTION);
    }

    if resp[1] != job.function_code {
        return Err(MB_ERR_INVALID_PARAM);
    }

    match job.function_code {
        MB_FC_READ_HOLDING_REGISTERS | MB_FC_READ_INPUT_REGISTERS => {
            let byte_count = resp[2] as usize;
            let count = job.count as usize;
            if byte_count != count * 2 || resp.len() < 3 + byte_count + 2 {
                return Err(MB_ERR_INVALID_PARAM);
            }
            for i in 0..count {
                let hi = resp[3 + i * 2];
                let lo = resp[3 + i * 2 + 1];
                let reg_val = u16::from_be_bytes([hi, lo]) as i32;
                let target_addr = (job.buffer_addr as usize + i) & 0xFF;
                word_mem[target_addr] = reg_val;
            }
            Ok(())
        }
        MB_FC_READ_COILS | MB_FC_READ_DISCRETE_INPUTS => {
            let byte_count = resp[2] as usize;
            let count = job.count as usize;
            if resp.len() < 3 + byte_count + 2 {
                return Err(MB_ERR_INVALID_PARAM);
            }
            for i in 0..count {
                let byte_idx = 3 + (i / 8);
                let bit_idx = i % 8;
                let val = if byte_idx < 3 + byte_count {
                    (resp[byte_idx] >> bit_idx) & 1
                } else {
                    0
                };
                let target_addr = (job.buffer_addr as usize + i) & 0xFF;
                bit_mem[target_addr] = val;
            }
            Ok(())
        }
        MB_FC_WRITE_SINGLE_COIL
        | MB_FC_WRITE_SINGLE_REGISTER
        | MB_FC_WRITE_MULTIPLE_COILS
        | MB_FC_WRITE_MULTIPLE_REGISTERS => {
            // Write operations succeed when slave echoes valid response frame
            Ok(())
        }
        _ => Err(MB_ERR_INVALID_PARAM),
    }
}

/// OpenPLC / IEC 61131-3 Modbus RTU Slave Memory Map Limits
const SLAVE_COIL_BASE: usize = 0x20; // %QX0.0..%QX3.7 (0..31), %MX0.0..%MX15.7 (32..159)
const SLAVE_COIL_COUNT: usize = 160;
const SLAVE_DI_BASE: usize = 0x00; // %IX0.0..%IX3.7 (0..31)
const SLAVE_DI_COUNT: usize = 32;
const SLAVE_HR_BASE: usize = 0x20; // %MW0..%MW159 (0..159)
const SLAVE_HR_COUNT: usize = 160;
const SLAVE_IR_BASE: usize = 0x00; // %IW0..%IW15 (0..15)
const SLAVE_IR_COUNT: usize = 16;

fn finish_rtu_frame(out_buf: &mut [u8], payload_len: usize) -> usize {
    let crc = compute_crc16(&out_buf[..payload_len]);
    let [c_lo, c_hi] = crc.to_le_bytes();
    out_buf[payload_len] = c_lo;
    out_buf[payload_len + 1] = c_hi;
    payload_len + 2
}

fn encode_slave_exception(
    slave_id: u8,
    fc: u8,
    ex_code: u8,
    out_buf: &mut [u8],
) -> Result<usize, &'static str> {
    if out_buf.len() < 5 {
        return Err("Output buffer too small for Modbus exception");
    }
    out_buf[0] = slave_id;
    out_buf[1] = fc | 0x80;
    out_buf[2] = ex_code;
    Ok(finish_rtu_frame(out_buf, 3))
}

fn read_bits_packed(
    bit_mem: &[u8; 256],
    base: usize,
    start: usize,
    count: usize,
    out_bytes: &mut [u8],
) {
    let byte_count = count.div_ceil(8);
    for b in &mut out_bytes[..byte_count] {
        *b = 0;
    }
    for i in 0..count {
        if bit_mem[base + start + i] != 0 {
            out_bytes[i / 8] |= 1 << (i % 8);
        }
    }
}

/// Process an incoming Modbus RTU request frame as a Slave following OpenPLC / IEC 61131-3
/// memory mapping (`%QX`/`%MX` -> Coils, `%IX` -> Discrete Inputs, `%MW` -> Holding Regs, `%IW` -> Input Regs).
///
/// Returns `Ok(0)` when no response should be sent (wrong `slave_id`, invalid CRC, or broadcast),
/// or `Ok(n)` with the complete CRC-appended response frame in `out_buf[..n]`.
pub fn process_rtu_slave_request(
    slave_id: u8,
    req: &[u8],
    word_mem: &mut [i32; 256],
    bit_mem: &mut [u8; 256],
    out_buf: &mut [u8],
) -> Result<usize, &'static str> {
    if slave_id == 0 || req.len() < 8 {
        return Ok(0);
    }

    let len = req.len();
    let expected_crc = u16::from_le_bytes([req[len - 2], req[len - 1]]);
    if compute_crc16(&req[..len - 2]) != expected_crc {
        return Ok(0);
    }

    let req_slave = req[0];
    let is_broadcast = req_slave == 0;
    if !is_broadcast && req_slave != slave_id {
        return Ok(0);
    }

    let fc = req[1];
    let addr = u16::from_be_bytes([req[2], req[3]]) as usize;
    let val_or_cnt = u16::from_be_bytes([req[4], req[5]]);

    let reply_len = match fc {
        MB_FC_READ_COILS | MB_FC_READ_DISCRETE_INPUTS => {
            if is_broadcast {
                return Ok(0);
            }
            let count = val_or_cnt as usize;
            let (base, max_count) = if fc == MB_FC_READ_COILS {
                (SLAVE_COIL_BASE, SLAVE_COIL_COUNT)
            } else {
                (SLAVE_DI_BASE, SLAVE_DI_COUNT)
            };
            if count == 0 || count > max_count {
                return encode_slave_exception(slave_id, fc, MB_EX_ILLEGAL_DATA_VALUE, out_buf);
            }
            if addr.saturating_add(count) > max_count {
                return encode_slave_exception(slave_id, fc, MB_EX_ILLEGAL_DATA_ADDRESS, out_buf);
            }
            let byte_count = count.div_ceil(8);
            if out_buf.len() < 3 + byte_count + 2 {
                return Err("Output buffer too small for bit read response");
            }
            out_buf[0] = slave_id;
            out_buf[1] = fc;
            out_buf[2] = byte_count as u8;
            read_bits_packed(bit_mem, base, addr, count, &mut out_buf[3..3 + byte_count]);
            finish_rtu_frame(out_buf, 3 + byte_count)
        }
        MB_FC_READ_HOLDING_REGISTERS | MB_FC_READ_INPUT_REGISTERS => {
            if is_broadcast {
                return Ok(0);
            }
            let count = val_or_cnt as usize;
            let (base, max_count) = if fc == MB_FC_READ_HOLDING_REGISTERS {
                (SLAVE_HR_BASE, SLAVE_HR_COUNT)
            } else {
                (SLAVE_IR_BASE, SLAVE_IR_COUNT)
            };
            if count == 0 || count > 125 {
                return encode_slave_exception(slave_id, fc, MB_EX_ILLEGAL_DATA_VALUE, out_buf);
            }
            if addr.saturating_add(count) > max_count {
                return encode_slave_exception(slave_id, fc, MB_EX_ILLEGAL_DATA_ADDRESS, out_buf);
            }
            let byte_count = count * 2;
            if out_buf.len() < 3 + byte_count + 2 {
                return Err("Output buffer too small for register read response");
            }
            out_buf[0] = slave_id;
            out_buf[1] = fc;
            out_buf[2] = byte_count as u8;
            for i in 0..count {
                let reg = word_mem[base + addr + i] as u16;
                let [hi, lo] = reg.to_be_bytes();
                out_buf[3 + i * 2] = hi;
                out_buf[3 + i * 2 + 1] = lo;
            }
            finish_rtu_frame(out_buf, 3 + byte_count)
        }
        MB_FC_WRITE_SINGLE_COIL => {
            if addr >= SLAVE_COIL_COUNT {
                return if is_broadcast {
                    Ok(0)
                } else {
                    encode_slave_exception(slave_id, fc, MB_EX_ILLEGAL_DATA_ADDRESS, out_buf)
                };
            }
            if val_or_cnt != 0xFF00 && val_or_cnt != 0x0000 {
                return if is_broadcast {
                    Ok(0)
                } else {
                    encode_slave_exception(slave_id, fc, MB_EX_ILLEGAL_DATA_VALUE, out_buf)
                };
            }
            bit_mem[SLAVE_COIL_BASE + addr] = u8::from(val_or_cnt == 0xFF00);
            if is_broadcast {
                return Ok(0);
            }
            if out_buf.len() < 8 {
                return Err("Output buffer too small for single coil echo");
            }
            out_buf[..6].copy_from_slice(&req[..6]);
            finish_rtu_frame(out_buf, 6)
        }
        MB_FC_WRITE_SINGLE_REGISTER => {
            if addr >= SLAVE_HR_COUNT {
                return if is_broadcast {
                    Ok(0)
                } else {
                    encode_slave_exception(slave_id, fc, MB_EX_ILLEGAL_DATA_ADDRESS, out_buf)
                };
            }
            word_mem[SLAVE_HR_BASE + addr] = val_or_cnt as i32;
            if is_broadcast {
                return Ok(0);
            }
            if out_buf.len() < 8 {
                return Err("Output buffer too small for single register echo");
            }
            out_buf[..6].copy_from_slice(&req[..6]);
            finish_rtu_frame(out_buf, 6)
        }
        MB_FC_WRITE_MULTIPLE_COILS => {
            let count = val_or_cnt as usize;
            if count == 0 || count > SLAVE_COIL_COUNT || req.len() < 9 {
                return if is_broadcast {
                    Ok(0)
                } else {
                    encode_slave_exception(slave_id, fc, MB_EX_ILLEGAL_DATA_VALUE, out_buf)
                };
            }
            let byte_count = req[6] as usize;
            if byte_count != count.div_ceil(8) || req.len() < 7 + byte_count + 2 {
                return if is_broadcast {
                    Ok(0)
                } else {
                    encode_slave_exception(slave_id, fc, MB_EX_ILLEGAL_DATA_VALUE, out_buf)
                };
            }
            if addr.saturating_add(count) > SLAVE_COIL_COUNT {
                return if is_broadcast {
                    Ok(0)
                } else {
                    encode_slave_exception(slave_id, fc, MB_EX_ILLEGAL_DATA_ADDRESS, out_buf)
                };
            }
            for i in 0..count {
                let bit_val = (req[7 + i / 8] >> (i % 8)) & 1;
                bit_mem[SLAVE_COIL_BASE + addr + i] = bit_val;
            }
            if is_broadcast {
                return Ok(0);
            }
            if out_buf.len() < 8 {
                return Err("Output buffer too small for multi-coil response");
            }
            out_buf[..6].copy_from_slice(&req[..6]);
            finish_rtu_frame(out_buf, 6)
        }
        MB_FC_WRITE_MULTIPLE_REGISTERS => {
            let count = val_or_cnt as usize;
            if count == 0 || count > 123 || req.len() < 9 {
                return if is_broadcast {
                    Ok(0)
                } else {
                    encode_slave_exception(slave_id, fc, MB_EX_ILLEGAL_DATA_VALUE, out_buf)
                };
            }
            let byte_count = req[6] as usize;
            if byte_count != count * 2 || req.len() < 7 + byte_count + 2 {
                return if is_broadcast {
                    Ok(0)
                } else {
                    encode_slave_exception(slave_id, fc, MB_EX_ILLEGAL_DATA_VALUE, out_buf)
                };
            }
            if addr.saturating_add(count) > SLAVE_HR_COUNT {
                return if is_broadcast {
                    Ok(0)
                } else {
                    encode_slave_exception(slave_id, fc, MB_EX_ILLEGAL_DATA_ADDRESS, out_buf)
                };
            }
            for i in 0..count {
                let reg_val = u16::from_be_bytes([req[7 + i * 2], req[7 + i * 2 + 1]]) as i32;
                word_mem[SLAVE_HR_BASE + addr + i] = reg_val;
            }
            if is_broadcast {
                return Ok(0);
            }
            if out_buf.len() < 8 {
                return Err("Output buffer too small for multi-register response");
            }
            out_buf[..6].copy_from_slice(&req[..6]);
            finish_rtu_frame(out_buf, 6)
        }
        _ => {
            if is_broadcast {
                return Ok(0);
            }
            return encode_slave_exception(slave_id, fc, MB_EX_ILLEGAL_FUNCTION, out_buf);
        }
    };

    Ok(reply_len)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn append_crc(frame: &mut [u8], payload_len: usize) -> usize {
        finish_rtu_frame(frame, payload_len)
    }

    #[test]
    fn test_slave_fc06_fc03_openplc_mw_mapping() {
        let mut word_mem = [0i32; 256];
        let mut bit_mem = [0u8; 256];
        let mut out = [0u8; 64];

        // Master writes 0x1234 (4660) to Slave 01 Holding Register 5 (%MW5 -> word_mem[0x25])
        let mut req = [0x01, MB_FC_WRITE_SINGLE_REGISTER, 0x00, 0x05, 0x12, 0x34, 0, 0];
        let req_len = append_crc(&mut req, 6);
        let n = process_rtu_slave_request(1, &req[..req_len], &mut word_mem, &mut bit_mem, &mut out).unwrap();
        assert_eq!(n, 8);
        assert_eq!(&out[..8], &req[..8]);
        assert_eq!(word_mem[SLAVE_HR_BASE + 5], 0x1234);

        // Master reads 1 Holding Register from addr 5
        let mut read_req = [0x01, MB_FC_READ_HOLDING_REGISTERS, 0x00, 0x05, 0x00, 0x01, 0, 0];
        let r_len = append_crc(&mut read_req, 6);
        let n = process_rtu_slave_request(1, &read_req[..r_len], &mut word_mem, &mut bit_mem, &mut out).unwrap();
        assert_eq!(n, 7);
        assert_eq!(&out[..5], &[0x01, MB_FC_READ_HOLDING_REGISTERS, 0x02, 0x12, 0x34]);
    }

    #[test]
    fn test_slave_fc05_fc01_fc02_fc04_fc15_fc16_and_errors() {
        let mut word_mem = [0i32; 256];
        let mut bit_mem = [0u8; 256];
        let mut out = [0u8; 64];

        // Wrong slave ID -> silent ignore (0 bytes)
        let mut req_other = [0x02, MB_FC_READ_HOLDING_REGISTERS, 0x00, 0x00, 0x00, 0x01, 0, 0];
        let l = append_crc(&mut req_other, 6);
        assert_eq!(process_rtu_slave_request(1, &req_other[..l], &mut word_mem, &mut bit_mem, &mut out).unwrap(), 0);

        // Bad CRC -> silent ignore (0 bytes)
        req_other[0] = 0x01;
        req_other[7] ^= 0xFF;
        assert_eq!(process_rtu_slave_request(1, &req_other[..l], &mut word_mem, &mut bit_mem, &mut out).unwrap(), 0);

        // FC05 Write Single Coil addr 2 (%QX0.2) ON
        let mut fc05 = [0x01, MB_FC_WRITE_SINGLE_COIL, 0x00, 0x02, 0xFF, 0x00, 0, 0];
        let l = append_crc(&mut fc05, 6);
        assert_eq!(process_rtu_slave_request(1, &fc05[..l], &mut word_mem, &mut bit_mem, &mut out).unwrap(), 8);
        assert_eq!(bit_mem[SLAVE_COIL_BASE + 2], 1);

        // FC01 Read Coils addr 0 count 4 -> 0b0100 = 0x04
        let mut fc01 = [0x01, MB_FC_READ_COILS, 0x00, 0x00, 0x00, 0x04, 0, 0];
        let l = append_crc(&mut fc01, 6);
        let n = process_rtu_slave_request(1, &fc01[..l], &mut word_mem, &mut bit_mem, &mut out).unwrap();
        assert_eq!(n, 6);
        assert_eq!(&out[..4], &[0x01, MB_FC_READ_COILS, 0x01, 0x04]);

        // FC02 Read Discrete Input (%IX0.1)
        bit_mem[SLAVE_DI_BASE + 1] = 1;
        let mut fc02 = [0x01, MB_FC_READ_DISCRETE_INPUTS, 0x00, 0x00, 0x00, 0x02, 0, 0];
        let l = append_crc(&mut fc02, 6);
        let n = process_rtu_slave_request(1, &fc02[..l], &mut word_mem, &mut bit_mem, &mut out).unwrap();
        assert_eq!(n, 6);
        assert_eq!(&out[..4], &[0x01, MB_FC_READ_DISCRETE_INPUTS, 0x01, 0x02]);

        // FC04 Read Input Register (%IW1)
        word_mem[SLAVE_IR_BASE + 1] = 2048;
        let mut fc04 = [0x01, MB_FC_READ_INPUT_REGISTERS, 0x00, 0x01, 0x00, 0x01, 0, 0];
        let l = append_crc(&mut fc04, 6);
        let n = process_rtu_slave_request(1, &fc04[..l], &mut word_mem, &mut bit_mem, &mut out).unwrap();
        assert_eq!(n, 7);
        assert_eq!(&out[..5], &[0x01, MB_FC_READ_INPUT_REGISTERS, 0x02, 0x08, 0x00]);

        // FC15 Write Multiple Coils addr 0 count 3 -> 0b101
        let mut fc15 = [0x01, MB_FC_WRITE_MULTIPLE_COILS, 0x00, 0x00, 0x00, 0x03, 0x01, 0x05, 0, 0];
        let l = append_crc(&mut fc15, 8);
        let n = process_rtu_slave_request(1, &fc15[..l], &mut word_mem, &mut bit_mem, &mut out).unwrap();
        assert_eq!(n, 8);
        assert_eq!(&bit_mem[SLAVE_COIL_BASE..SLAVE_COIL_BASE + 3], &[1, 0, 1]);

        // FC16 Write Multiple Registers addr 10 count 2 -> [100, 200]
        let mut fc16 = [
            0x01, MB_FC_WRITE_MULTIPLE_REGISTERS, 0x00, 0x0A, 0x00, 0x02, 0x04,
            0x00, 0x64, 0x00, 0xC8, 0, 0,
        ];
        let l = append_crc(&mut fc16, 11);
        let n = process_rtu_slave_request(1, &fc16[..l], &mut word_mem, &mut bit_mem, &mut out).unwrap();
        assert_eq!(n, 8);
        assert_eq!(word_mem[SLAVE_HR_BASE + 10], 100);
        assert_eq!(word_mem[SLAVE_HR_BASE + 11], 200);

        // Out-of-range address -> Exception 0x02 (Illegal Data Address)
        let mut bad_addr = [0x01, MB_FC_READ_HOLDING_REGISTERS, 0x01, 0x00, 0x00, 0x01, 0, 0];
        let l = append_crc(&mut bad_addr, 6);
        let n = process_rtu_slave_request(1, &bad_addr[..l], &mut word_mem, &mut bit_mem, &mut out).unwrap();
        assert_eq!(n, 5);
        assert_eq!(&out[..3], &[0x01, MB_FC_READ_HOLDING_REGISTERS | 0x80, MB_EX_ILLEGAL_DATA_ADDRESS]);
    }
}
