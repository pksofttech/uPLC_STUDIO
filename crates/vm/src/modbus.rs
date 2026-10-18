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
pub const MB_FC_WRITE_MULTIPLE_REGISTERS: u8 = 0x10;

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
        | MB_FC_WRITE_MULTIPLE_REGISTERS => {
            // Write operations succeed when slave echoes valid response frame
            Ok(())
        }
        _ => Err(MB_ERR_INVALID_PARAM),
    }
}
