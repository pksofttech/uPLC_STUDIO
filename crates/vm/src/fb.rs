//! # Feature-Level-2 Function Blocks (`no_std`, no `alloc`)
//!
//! Pure arithmetic helpers shared by the Micro-VM (`MicroVm::step_scan`) and the
//! desktop Simulator (`my-canvas-core`) so both produce *bit-identical* results:
//!
//! - `PID`   : position-form PID with anti-windup, fixed-point gains ×100
//! - `SCALE` : linear scaling computed in `i64`
//! - `LIMIT` : clamp
//! - `SHL` / `SHR` : logical 32-bit shifts (negative count shifts the other way)
//! - `UART`  : UartSend payload formatting (low byte / ASCII decimal + CRLF)
//!
//! ## Operand Word Encoding (`OPND`, opcode `0x6F`)
//! ```text
//! [0x6F, kind, imm_lo, imm_hi]
//!   kind 0 = word register  -> imm_lo = word address
//!   kind 1 = constant i16   -> imm = value (sign-extended)
//!   kind 2 = bit address    -> imm_lo = bit address
//! ```

/// Firmware/VM feature level reported in the status snapshot (high nibble of the `acc_bool` byte).
/// `1` (or `0` = not reported) = legacy VM, `2` = OPND + PID/SCALE/LIMIT/Bitwise/Shift/UART opcodes.
pub const FW_FEATURE_LEVEL: u8 = 2;

pub const MAX_PID_INSTANCES: usize = 8;
pub const MAX_UART_INSTANCES: usize = 8;
/// Number of edge-detection slots available to edge-triggered function blocks (Shift / ShiftReg).
pub const MAX_FB_EDGE_SLOTS: usize = 256;

// --- Opcodes (feature level 2) ---
pub const OP_MOVW: u8 = 0x46;
pub const OP_WAND: u8 = 0x47;
pub const OP_WOR: u8 = 0x48;
pub const OP_WXOR: u8 = 0x49;
pub const OP_WNOT: u8 = 0x4A;
pub const OP_SHL: u8 = 0x4B;
pub const OP_SHR: u8 = 0x4C;
pub const OP_SHREG: u8 = 0x52;
pub const OP_SHBIT: u8 = 0x53;
pub const OP_OPND: u8 = 0x6F;
pub const OP_SCALE: u8 = 0x70;
pub const OP_LIMIT: u8 = 0x71;
pub const OP_PID: u8 = 0x72;
pub const OP_UTX: u8 = 0x74;
pub const OP_URX: u8 = 0x75;

// --- OPND kinds ---
pub const OPND_KIND_WORD: u8 = 0;
pub const OPND_KIND_CONST: u8 = 1;
pub const OPND_KIND_BIT: u8 = 2;

/// `SHL` / `SHR` header `imm` flag: when set, the shift executes only on the rising edge of the
/// rung condition, using edge slot `imm & 0xFF`. When clear, the shift executes every scan (level).
pub const SHIFT_EDGE_FLAG: u16 = 0x8000;

/// Number of `OPND` words that follow a header opcode (0 for opcodes without operand words).
pub const fn opnd_word_count(op: u8) -> usize {
    match op {
        OP_MOVW => 1,
        OP_WAND | OP_WOR | OP_WXOR | OP_WNOT | OP_SHL | OP_SHR => 2,
        OP_SHREG | OP_SHBIT => 1,
        OP_SCALE => 5,
        OP_LIMIT => 3,
        OP_PID => 6,
        OP_UTX => 1,
        OP_URX => 0,
        _ => 0,
    }
}

/// Saturating conversion `i64 -> i32`.
#[inline]
pub const fn sat_i32(v: i64) -> i32 {
    if v > i32::MAX as i64 {
        i32::MAX
    } else if v < i32::MIN as i64 {
        i32::MIN
    } else {
        v as i32
    }
}

/// Linear scaling: `out_min + (in - in_min) * (out_max - out_min) / (in_max - in_min)`.
/// Computed in `i64` (truncating division) and saturated to `i32`. If `in_max == in_min` → `out_min`.
pub fn scale(input: i32, in_min: i32, in_max: i32, out_min: i32, out_max: i32) -> i32 {
    let den = in_max as i64 - in_min as i64;
    if den == 0 {
        return out_min;
    }
    let num = (input as i64 - in_min as i64) * (out_max as i64 - out_min as i64);
    sat_i32(out_min as i64 + num / den)
}

/// Clamp `input` into `[mn, mx]` (if `mn > mx` the result is `mx`).
#[inline]
pub fn limit(input: i32, mn: i32, mx: i32) -> i32 {
    input.max(mn).min(mx)
}

/// Logical shift left by `n` bits (`n` clamped to 0..=31 in magnitude). Negative `n` shifts right.
#[inline]
pub fn shift_left(a: i32, n: i32) -> i32 {
    if n >= 0 {
        ((a as u32) << (n.min(31) as u32)) as i32
    } else {
        ((a as u32) >> ((-(n as i64)).min(31) as u32)) as i32
    }
}

/// Logical shift right by `n` bits (`n` clamped to 0..=31 in magnitude). Negative `n` shifts left.
#[inline]
pub fn shift_right(a: i32, n: i32) -> i32 {
    if n >= 0 {
        ((a as u32) >> (n.min(31) as u32)) as i32
    } else {
        ((a as u32) << ((-(n as i64)).min(31) as u32)) as i32
    }
}

/// Bitwise word operation by opcode (`OP_WAND..=OP_SHR`). Unknown opcodes return `a`.
#[inline]
pub fn bitwise(op: u8, a: i32, b: i32) -> i32 {
    match op {
        OP_WAND => a & b,
        OP_WOR => a | b,
        OP_WXOR => a ^ b,
        OP_WNOT => !a,
        OP_SHL => shift_left(a, b),
        OP_SHR => shift_right(a, b),
        _ => a,
    }
}

/// Shift-in-bit: `w = (w << 1) | (bit & 1)` (32-bit logical).
#[inline]
pub fn shift_in_bit(w: i32, bit: i32) -> i32 {
    (((w as u32) << 1) | ((bit != 0) as u32)) as i32
}

/// Per-instance PID state (const-initializable).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PidState {
    /// Integral accumulator in units of `×100 × ms` (i.e. `Ki×100 · e · dt_ms`).
    pub integral: i64,
    /// Error of the previous scan (for the derivative term).
    pub prev_err: i32,
    /// `true` once the block has executed at least one scan since the last reset.
    pub active: bool,
}

impl PidState {
    pub const fn new() -> Self {
        Self { integral: 0, prev_err: 0, active: false }
    }

    /// Reset the integral and derivative history (called when EN = 0).
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Execute one PID step (position form + anti-windup), returning the new CV.
    ///
    /// - `kp`, `ki`, `kd`: gains ×100 (`Ki` per second, `Kd` in seconds)
    /// - `out_max`: CV upper limit (CV lower limit is 0)
    /// - `dt_ms`: elapsed scan time in milliseconds
    ///
    /// ```text
    /// e   = SP - PV
    /// I  += Ki * e * dt            (clamped so that I stays within 0..out_max)
    /// D   = Kd * (e - e_prev) / dt
    /// CV  = clamp(Kp*e + I + D, 0, out_max)
    /// ```
    pub fn step(&mut self, pv: i32, sp: i32, kp: i32, ki: i32, kd: i32, out_max: i32, dt_ms: u32) -> i32 {
        let e = sp as i64 - pv as i64;
        let out_max = out_max.max(0) as i64;
        if !self.active {
            self.prev_err = sat_i32(e);
            self.active = true;
        }
        let dt = dt_ms as i64;

        // Integral (×100 × ms), anti-windup clamp to the output range
        let i_max = out_max.saturating_mul(100_000);
        self.integral = self
            .integral
            .saturating_add((ki as i64).saturating_mul(e).saturating_mul(dt))
            .clamp(0, i_max);

        let p = (kp as i64).saturating_mul(e); // ×100
        let i = self.integral / 1000; // ×100
        let d = if dt > 0 {
            (kd as i64).saturating_mul(e - self.prev_err as i64).saturating_mul(1000) / dt // ×100
        } else {
            0
        };
        self.prev_err = sat_i32(e);

        let out = p.saturating_add(i).saturating_add(d) / 100;
        out.clamp(0, out_max) as i32
    }
}

/// Outgoing UART job produced by `UTX` and consumed by the firmware UART task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UartTx {
    pub instance: u8,
    /// 0 = send low byte of `value`, 1 = send ASCII decimal + `"\r\n"`
    pub mode: u8,
    pub value: i32,
}

impl UartTx {
    /// Encode the payload bytes into `out`, returning the number of bytes written (max 13).
    pub fn encode(&self, out: &mut [u8; 16]) -> usize {
        if self.mode == 0 {
            out[0] = (self.value & 0xFF) as u8;
            return 1;
        }
        let mut digits = [0u8; 12];
        let mut n = 0usize;
        let neg = self.value < 0;
        let mut v: u32 = if neg { (self.value as i64).unsigned_abs() as u32 } else { self.value as u32 };
        loop {
            digits[n] = b'0' + (v % 10) as u8;
            n += 1;
            v /= 10;
            if v == 0 {
                break;
            }
        }
        let mut len = 0usize;
        if neg {
            out[len] = b'-';
            len += 1;
        }
        while n > 0 {
            n -= 1;
            out[len] = digits[n];
            len += 1;
        }
        out[len] = b'\r';
        out[len + 1] = b'\n';
        len + 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scale_limit_shift_helpers() {
        assert_eq!(scale(2048, 0, 4095, 0, 1000), 500);
        assert_eq!(scale(4095, 0, 4095, 0, 1000), 1000);
        assert_eq!(scale(5, 3, 3, 7, 9), 7);
        assert_eq!(scale(0, 0, 100, 100, -100), 100);
        assert_eq!(limit(50, 100, 900), 100);
        assert_eq!(limit(950, 100, 900), 900);
        assert_eq!(shift_left(1, 4), 16);
        assert_eq!(shift_left(16, -2), 4);
        assert_eq!(shift_right(-1, 28), 0xF);
        assert_eq!(shift_left(1, 99), i32::MIN);
        assert_eq!(shift_in_bit(0b101, 1), 0b1011);
    }

    #[test]
    fn test_uart_encode() {
        let mut buf = [0u8; 16];
        let n = UartTx { instance: 0, mode: 1, value: -1234 }.encode(&mut buf);
        assert_eq!(&buf[..n], b"-1234\r\n");
        let n = UartTx { instance: 0, mode: 1, value: i32::MIN }.encode(&mut buf);
        assert_eq!(&buf[..n], b"-2147483648\r\n");
        let n = UartTx { instance: 0, mode: 0, value: 0x1241 }.encode(&mut buf);
        assert_eq!(&buf[..n], &[0x41]);
    }
}
