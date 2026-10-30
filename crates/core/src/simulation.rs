use std::collections::{HashMap, VecDeque};
use crate::bytecode::{BytecodeCompiler, PlcStatusSnapshot};
use crate::ladder::{parse_int_literal, CoilMode, ContactMode, IOPortMapping, LadderElement, LadderProgram, TypeTag};
use uplc_vm::{PidState, UartTx};

/// Canonical class of a word register address used by the Simulator alias table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WordKey {
    /// `%MW0..%MW127` / `D0..D127` / `V0..V127`
    Mw(usize),
    /// `%SD0..%SD31` / `SD*` / `D128..D159` / `D8000..D8031`
    Sd(usize),
    /// `%IW*` / `AI*` / `ADC*`
    Iw(usize),
    /// `%QW*` / `AO*` / `PWM*`
    Qw(usize),
    /// Timer current value `T*`
    Timer(usize),
    /// Counter current value `C*`
    Counter(usize),
}

fn parse_digits(s: &str) -> Option<usize> {
    if !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()) {
        s.parse().ok()
    } else {
        None
    }
}

fn classify_word(name: &str) -> Option<WordKey> {
    let s = name.trim().to_ascii_uppercase();
    let mw_or_sd = |n: usize| -> Option<WordKey> {
        if (8000..8032).contains(&n) {
            Some(WordKey::Sd(n - 8000))
        } else if n < 128 {
            Some(WordKey::Mw(n))
        } else if n < 160 {
            Some(WordKey::Sd(n - 128))
        } else {
            None
        }
    };
    if let Some(r) = s.strip_prefix("%MW") {
        return parse_digits(r).and_then(mw_or_sd);
    }
    if let Some(r) = s.strip_prefix("%SD").or_else(|| s.strip_prefix("SD")) {
        return parse_digits(r).filter(|n| *n < 32).map(WordKey::Sd);
    }
    if let Some(r) = s.strip_prefix("%IW").or_else(|| s.strip_prefix("ADC")).or_else(|| s.strip_prefix("AI")) {
        return parse_digits(r).filter(|n| *n < 16).map(WordKey::Iw);
    }
    if let Some(r) = s.strip_prefix("%QW").or_else(|| s.strip_prefix("PWM")).or_else(|| s.strip_prefix("AO")) {
        return parse_digits(r).filter(|n| *n < 16).map(WordKey::Qw);
    }
    if let Some(r) = s.strip_prefix('D').or_else(|| s.strip_prefix('V')) {
        return parse_digits(r).and_then(mw_or_sd);
    }
    if let Some(r) = s.strip_prefix('T') {
        return parse_digits(r).map(WordKey::Timer);
    }
    if let Some(r) = s.strip_prefix('C') {
        return parse_digits(r).map(WordKey::Counter);
    }
    None
}

fn word_aliases(key: WordKey) -> Vec<String> {
    match key {
        WordKey::Mw(n) => vec![format!("%MW{}", n), format!("D{}", n), format!("D{:02}", n), format!("V{}", n), format!("V{:02}", n)],
        WordKey::Sd(i) => vec![
            format!("%SD{}", i),
            format!("SD{}", i),
            format!("D{}", 128 + i),
            format!("D{}", 8000 + i),
            format!("%MW{}", 128 + i),
        ],
        WordKey::Iw(n) => vec![format!("%IW{}", n), format!("AI{}", n), format!("AI{:02}", n)],
        WordKey::Qw(n) => vec![format!("%QW{}", n), format!("AO{}", n), format!("AO{:02}", n)],
        WordKey::Timer(n) => vec![format!("T{}", n)],
        WordKey::Counter(n) => vec![format!("C{}", n)],
    }
}

/// Map a word operand string to the opcode used by the VM for a `Bitwise` element (`math_op`).
pub fn bitwise_opcode(math_op: &str) -> u8 {
    match math_op.trim().to_ascii_uppercase().as_str() {
        "OR" | "WOR" | "|" => uplc_vm::OP_WOR,
        "XOR" | "WXOR" | "^" => uplc_vm::OP_WXOR,
        "NOT" | "WNOT" | "~" | "!" => uplc_vm::OP_WNOT,
        "SHL" | "<<" => uplc_vm::OP_SHL,
        "SHR" | ">>" => uplc_vm::OP_SHR,
        _ => uplc_vm::OP_WAND,
    }
}

/// Parses IEC 61131-3 (%IX0.0, %QX0.0, %MX0.0) or legacy (X0, Y0, M0) addresses
/// Returns location type ('I', 'Q', 'M') and 0-based bit index.
fn get_iec_channel_info(name: &str) -> Option<(char, usize)> {
    let s = name.trim().to_uppercase();

    // Check IEC %IXb.b or %QXb.b or %MXb.b
    if s.starts_with('%') {
        let remainder = &s[1..];
        if remainder.starts_with("IX") || remainder.starts_with("IB") || remainder.starts_with("IW") {
            let num = remainder[2..].trim();
            if let Some((b_str, bit_str)) = num.split_once('.') {
                let byte: usize = b_str.parse().ok()?;
                let bit: usize = bit_str.parse().ok()?;
                return Some(('I', byte * 8 + bit));
            } else if let Ok(idx) = num.parse::<usize>() {
                return Some(('I', idx));
            }
        } else if remainder.starts_with("QX") || remainder.starts_with("QB") || remainder.starts_with("QW") {
            let num = remainder[2..].trim();
            if let Some((b_str, bit_str)) = num.split_once('.') {
                let byte: usize = b_str.parse().ok()?;
                let bit: usize = bit_str.parse().ok()?;
                return Some(('Q', byte * 8 + bit));
            } else if let Ok(idx) = num.parse::<usize>() {
                return Some(('Q', idx));
            }
        } else if remainder.starts_with("MX") || remainder.starts_with("MB") || remainder.starts_with("MW") {
            let num = remainder[2..].trim();
            if let Some((b_str, bit_str)) = num.split_once('.') {
                let byte: usize = b_str.parse().ok()?;
                let bit: usize = bit_str.parse().ok()?;
                return Some(('M', byte * 8 + bit));
            } else if let Ok(idx) = num.parse::<usize>() {
                return Some(('M', idx));
            }
        } else if remainder.starts_with("SM") {
            let num = remainder[2..].trim();
            if let Ok(idx) = num.parse::<usize>() {
                return Some(('M', 96 + idx.min(31)));
            }
        }
    }

    // Check legacy X0..X99 or I0..I99
    if (s.starts_with('X') || s.starts_with('I')) && s.len() > 1 && s[1..].chars().all(|c| c.is_ascii_digit()) {
        if let Ok(idx) = s[1..].parse::<usize>() {
            return Some(('I', idx));
        }
    }

    // Check legacy Y0..Y99 or Q0..Q99
    if (s.starts_with('Y') || s.starts_with('Q')) && s.len() > 1 && s[1..].chars().all(|c| c.is_ascii_digit()) {
        if let Ok(idx) = s[1..].parse::<usize>() {
            return Some(('Q', idx));
        }
    }

    // Check legacy M0..M99 or R0..R99 or M8000..M8031
    if (s.starts_with('M') || s.starts_with('R')) && s.len() > 1 && s[1..].chars().all(|c| c.is_ascii_digit()) {
        if let Ok(idx) = s[1..].parse::<usize>() {
            if (8000..=8031).contains(&idx) {
                return Some(('M', 96 + (idx - 8000)));
            }
            return Some(('M', idx));
        }
    }

    // Check SM0..SM31
    if s.starts_with("SM") && s.len() > 2 && s[2..].chars().all(|c| c.is_ascii_digit()) {
        if let Ok(idx) = s[2..].parse::<usize>() {
            return Some(('M', 96 + idx.min(31)));
        }
    }

    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimState {
    Stopped,
    Running,
    Step,
}

#[derive(Debug, Clone)]
pub struct PlcSimulator {
    pub state: SimState,
    pub simulation_enabled: bool,
    pub online_mode: bool,
    pub remote_running: bool,
    pub cycle_time_ms: u32,
    pub speed_multiplier: u32,
    pub scan_count: u64,
    pub simulation_time_ms: u64,
    pub acc_bool: bool,
    pub acc_word: i32,
    
    // Digital inputs X0..X7 / %IX0.0..%IX0.7
    pub inputs: HashMap<String, bool>,
    // Digital outputs Y0..Y7 / %QX0.0..%QX0.7
    pub outputs: HashMap<String, bool>,
    // Internal relays / markers M0..M31 or R0..R31 / %MX0.0..%MX3.7
    pub markers: HashMap<String, bool>,
    // Timer states: (accum_ms, done)
    pub timers: HashMap<String, (u32, bool)>,
    // Counter states: (count, done, last_input_edge)
    pub counters: HashMap<String, (i32, bool, bool)>,
    // Data registers D0..D15 / V0..V15 / %MW0..%MW15
    pub registers: HashMap<String, i32>,
    // Edge detection states keyed by element ID
    pub edge_states: HashMap<u64, bool>,
    // Forced bit states: address (u8) -> forced bool value (Force ON / Force OFF)
    pub forced_bits: HashMap<u8, bool>,
    /// PID function block states keyed by instance name (uppercase)
    pub pid_states: HashMap<String, PidState>,
    /// Bytes "transmitted" by UartSend blocks (drained by the UI UART terminal via `take_uart_tx`)
    pub uart_tx_log: Vec<u8>,
    /// Bytes waiting to be "received" by UartRecv blocks (filled via `push_uart_rx`)
    pub uart_rx_queue: VecDeque<u8>,
}

impl Default for PlcSimulator {
    fn default() -> Self {
        let mut inputs = HashMap::new();
        for i in 0..16 {
            let byte = i / 8;
            let bit = i % 8;
            inputs.insert(format!("X{}", i), false);
            inputs.insert(format!("X{:02}", i), false);
            inputs.insert(format!("%IX{}.{}", byte, bit), false);
        }
        
        let mut outputs = HashMap::new();
        for i in 0..16 {
            let byte = i / 8;
            let bit = i % 8;
            outputs.insert(format!("Y{}", i), false);
            outputs.insert(format!("Y{:02}", i), false);
            outputs.insert(format!("%QX{}.{}", byte, bit), false);
        }

        let mut markers = HashMap::new();
        for i in 0..96 {
            let byte = i / 8;
            let bit = i % 8;
            markers.insert(format!("M{}", i), false);
            markers.insert(format!("M{:02}", i), false);
            markers.insert(format!("R{}", i), false);
            markers.insert(format!("R{:02}", i), false);
            markers.insert(format!("%MX{}.{}", byte, bit), false);
        }
        for i in 0..32 {
            let val = i == 0; // %SM0 is always ON by default
            markers.insert(format!("%SM{}", i), val);
            markers.insert(format!("SM{}", i), val);
            markers.insert(format!("M{}", 96 + i), val);
            markers.insert(format!("M{}", 8000 + i), val);
        }

        let mut registers = HashMap::new();
        for i in 0..128 {
            registers.insert(format!("D{}", i), 0);
            registers.insert(format!("D{:02}", i), 0);
            registers.insert(format!("V{}", i), 0);
            registers.insert(format!("V{:02}", i), 0);
            registers.insert(format!("%MW{}", i), 0);
        }
        for i in 0..16 {
            registers.insert(format!("AI{}", i), 0);
            registers.insert(format!("AI{:02}", i), 0);
            registers.insert(format!("%IW{}", i), 0);
            registers.insert(format!("AO{}", i), 0);
            registers.insert(format!("AO{:02}", i), 0);
            registers.insert(format!("%QW{}", i), 0);
        }
        for i in 0..32 {
            let val = if i == 0 { 100 } else { 0 }; // %SD0 watchdog preset 100ms
            registers.insert(format!("%SD{}", i), val);
            registers.insert(format!("SD{}", i), val);
            registers.insert(format!("D{}", 128 + i), val);
            registers.insert(format!("D{}", 8000 + i), val);
        }

        Self {
            state: SimState::Stopped,
            simulation_enabled: false,
            online_mode: false,
            remote_running: false,
            cycle_time_ms: 10,
            speed_multiplier: 1,
            scan_count: 0,
            simulation_time_ms: 0,
            acc_bool: false,
            acc_word: 0,
            inputs,
            outputs,
            markers,
            timers: HashMap::new(),
            counters: HashMap::new(),
            registers,
            edge_states: HashMap::new(),
            forced_bits: HashMap::new(),
            pid_states: HashMap::new(),
            uart_tx_log: Vec::new(),
            uart_rx_queue: VecDeque::new(),
        }
    }
}

impl PlcSimulator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Canonical register key of a word address so that `D5`, `V5` and `%MW5` (or `AI0` / `%IW0`,
    /// `D128` / `%SD0`, ...) refer to the same value. Unknown names map to their uppercase form.
    pub fn canonical_word_key(name: &str) -> String {
        match classify_word(name) {
            Some(k) => word_aliases(k).swap_remove(0),
            None => name.trim().to_ascii_uppercase(),
        }
    }

    /// Read a word operand: integer literals (`"100"`, `"-5"`, `"16#FF"`) are returned as-is,
    /// otherwise the value is read from the register / timer / counter (or bit as 0/1).
    pub fn read_word(&self, operand: &str) -> i32 {
        let t = operand.trim();
        if t.is_empty() {
            return 0;
        }
        if let Some(v) = parse_int_literal(t) {
            return v.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
        }
        match classify_word(t) {
            Some(WordKey::Timer(n)) => {
                return self
                    .timers
                    .get(&format!("T{}", n))
                    .or_else(|| self.timers.get(&format!("T{:02}", n)))
                    .map(|(acc, _)| *acc as i32)
                    .unwrap_or(0);
            }
            Some(WordKey::Counter(n)) => {
                return self
                    .counters
                    .get(&format!("C{}", n))
                    .or_else(|| self.counters.get(&format!("C{:02}", n)))
                    .map(|(cv, _, _)| *cv)
                    .unwrap_or(0);
            }
            Some(k) => {
                let aliases = word_aliases(k);
                for a in &aliases {
                    if let Some(&v) = self.registers.get(a) {
                        return v;
                    }
                }
                return 0;
            }
            None => {}
        }
        let upper = t.to_ascii_uppercase();
        if let Some(&v) = self.registers.get(&upper) {
            return v;
        }
        if get_iec_channel_info(&upper).is_some() {
            return i32::from(self.get_var_value(&upper));
        }
        0
    }

    /// Write a word register (all aliases are updated). Literals are ignored.
    pub fn write_word(&mut self, operand: &str, val: i32) {
        let t = operand.trim();
        if t.is_empty() || parse_int_literal(t).is_some() {
            return;
        }
        match classify_word(t) {
            Some(WordKey::Timer(n)) => {
                let e = self.timers.entry(format!("T{}", n)).or_insert((0, false));
                e.0 = val.max(0) as u32;
                if val <= 0 {
                    e.1 = false;
                }
            }
            Some(WordKey::Counter(n)) => {
                let e = self.counters.entry(format!("C{}", n)).or_insert((0, false, false));
                e.0 = val;
                if val <= 0 {
                    e.1 = false;
                }
            }
            Some(k) => {
                for a in word_aliases(k) {
                    self.registers.insert(a, val);
                }
            }
            None => {
                self.registers.insert(t.to_ascii_uppercase(), val);
            }
        }
    }

    /// Read a bit operand (`"%IX0.1"`, `"M5"`, ...). Integer literals are `!= 0`.
    pub fn read_bit(&self, operand: &str) -> bool {
        let t = operand.trim();
        if let Some(v) = parse_int_literal(t) {
            return v != 0;
        }
        match t.to_ascii_uppercase().as_str() {
            "TRUE" => true,
            "FALSE" | "" => false,
            _ => self.get_var_value(t),
        }
    }

    /// Queue bytes to be received by `UartRecv` blocks (1 byte per UartRecv execution).
    pub fn push_uart_rx(&mut self, bytes: &[u8]) {
        self.uart_rx_queue.extend(bytes.iter().copied());
    }

    /// Drain all bytes transmitted by `UartSend` blocks since the last call.
    pub fn take_uart_tx(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.uart_tx_log)
    }

    /// Rising-edge detection for edge-triggered function blocks (initial state = OFF, like the VM)
    fn fb_rising_edge(&mut self, id: u64, en: bool) -> bool {
        let prev = *self.edge_states.get(&id).unwrap_or(&false);
        self.edge_states.insert(id, en);
        en && !prev
    }

    /// Force a bit (Input X: 0x00..0x1F, Output Y: 0x20..0x3F, or Marker M: 0x40..0xBF) to ON/OFF
    pub fn force_bit(&mut self, addr: u8, val: bool) {
        self.forced_bits.insert(addr, val);
        self.apply_forced_bit_to_memory(addr, val);
    }

    /// Release force override on a bit address
    pub fn unforce_bit(&mut self, addr: u8) {
        self.forced_bits.remove(&addr);
    }

    /// Release all active force overrides
    pub fn clear_all_forces(&mut self) {
        self.forced_bits.clear();
    }

    /// Check if a bit address is forced, returning Some(val) or None
    pub fn is_bit_forced(&self, addr: u8) -> Option<bool> {
        self.forced_bits.get(&addr).copied()
    }

    /// Apply forced value directly into simulator's inputs/outputs/markers maps
    pub fn apply_forced_bit_to_memory(&mut self, addr: u8, val: bool) {
        if addr < 0x20 {
            // Inputs X0..X31 / %IX0.0..%IX3.7
            let byte = addr / 8;
            let bit = addr % 8;
            self.inputs.insert(format!("X{}", addr), val);
            self.inputs.insert(format!("X{:02}", addr), val);
            self.inputs.insert(format!("%IX{}.{}", byte, bit), val);
        } else if addr < 0x40 {
            // Outputs Y0..Y31 / %QX0.0..%QX3.7
            let idx = addr - 0x20;
            let byte = idx / 8;
            let bit = idx % 8;
            self.outputs.insert(format!("Y{}", idx), val);
            self.outputs.insert(format!("Y{:02}", idx), val);
            self.outputs.insert(format!("%QX{}.{}", byte, bit), val);
        } else if addr < 0xC0 {
            // Markers M0..M127 / %MX0.0..%MX15.7
            let idx = addr - 0x40;
            let byte = idx / 8;
            let bit = idx % 8;
            self.markers.insert(format!("M{}", idx), val);
            self.markers.insert(format!("M{:02}", idx), val);
            self.markers.insert(format!("R{}", idx), val);
            self.markers.insert(format!("R{:02}", idx), val);
            self.markers.insert(format!("%MX{}.{}", byte, bit), val);
            if idx >= 96 {
                let sm_idx = idx - 96;
                self.markers.insert(format!("%SM{}", sm_idx), val);
                self.markers.insert(format!("SM{}", sm_idx), val);
                self.markers.insert(format!("M{}", 8000 + sm_idx as u16), val);
            }
        }
    }

    #[inline]
    pub fn is_active(&self) -> bool {
        if self.online_mode {
            self.remote_running || self.state != SimState::Stopped
        } else {
            self.state != SimState::Stopped
        }
    }

    #[inline]
    pub fn is_running(&self) -> bool {
        self.state == SimState::Running
    }

    #[inline]
    pub fn is_stopped(&self) -> bool {
        self.state == SimState::Stopped
    }

    #[inline]
    pub fn is_step(&self) -> bool {
        self.state == SimState::Step
    }

    pub fn formatted_sim_time(&self) -> String {
        let total_ms = self.simulation_time_ms;
        let ms_part = (total_ms % 1000) / 10;
        let total_sec = total_ms / 1000;
        let sec = total_sec % 60;
        let min = (total_sec / 60) % 60;
        let hr = total_sec / 3600;
        format!("{:02}:{:02}:{:02}.{:02}", hr, min, sec, ms_part)
    }

    pub fn set_input(&mut self, name: &str, value: bool) {
        self.set_var_value(name, value);
    }

    pub fn toggle_input(&mut self, name: &str) -> bool {
        let cur = self.get_var_value(name);
        let new_val = !cur;
        self.set_var_value(name, new_val);
        new_val
    }

    pub fn get_var_value(&self, name: &str) -> bool {
        let upper = name.trim().to_uppercase();

        if let Some(&val) = self.inputs.get(&upper) {
            return val;
        }
        if let Some(&val) = self.outputs.get(&upper) {
            return val;
        }
        if let Some(&val) = self.markers.get(&upper) {
            return val;
        }
        if let Some((_, done)) = self.timers.get(&upper) {
            return *done;
        }
        if let Some((_, done, _)) = self.counters.get(&upper) {
            return *done;
        }

        // Check IEC/Legacy aliases if exact key was not directly set
        if let Some((loc, idx)) = get_iec_channel_info(&upper) {
            let byte = idx / 8;
            let bit = idx % 8;
            match loc {
                'I' => {
                    if let Some(&v) = self.inputs.get(&format!("X{}", idx)) {
                        return v;
                    }
                    if let Some(&v) = self.inputs.get(&format!("X{:02}", idx)) {
                        return v;
                    }
                    if let Some(&v) = self.inputs.get(&format!("%IX{}.{}", byte, bit)) {
                        return v;
                    }
                }
                'Q' => {
                    if let Some(&v) = self.outputs.get(&format!("Y{}", idx)) {
                        return v;
                    }
                    if let Some(&v) = self.outputs.get(&format!("Y{:02}", idx)) {
                        return v;
                    }
                    if let Some(&v) = self.outputs.get(&format!("%QX{}.{}", byte, bit)) {
                        return v;
                    }
                }
                'M' => {
                    if let Some(&v) = self.markers.get(&format!("M{}", idx)) {
                        return v;
                    }
                    if let Some(&v) = self.markers.get(&format!("M{:02}", idx)) {
                        return v;
                    }
                    if let Some(&v) = self.markers.get(&format!("R{}", idx)) {
                        return v;
                    }
                    if let Some(&v) = self.markers.get(&format!("%MX{}.{}", byte, bit)) {
                        return v;
                    }
                    if idx >= 96 {
                        let sm_idx = idx - 96;
                        if let Some(&v) = self.markers.get(&format!("%SM{}", sm_idx)) {
                            return v;
                        }
                    }
                }
                _ => {}
            }
        }

        false
    }

    pub fn set_var_value(&mut self, name: &str, val: bool) {
        let upper = name.trim().to_uppercase();
        if let Some((loc, idx)) = get_iec_channel_info(&upper) {
            let byte = idx / 8;
            let bit = idx % 8;
            match loc {
                'I' => {
                    self.inputs.insert(format!("X{}", idx), val);
                    self.inputs.insert(format!("X{:02}", idx), val);
                    self.inputs.insert(format!("%IX{}.{}", byte, bit), val);
                    self.inputs.insert(upper, val);
                }
                'Q' => {
                    self.outputs.insert(format!("Y{}", idx), val);
                    self.outputs.insert(format!("Y{:02}", idx), val);
                    self.outputs.insert(format!("%QX{}.{}", byte, bit), val);
                    self.outputs.insert(upper, val);
                }
                'M' => {
                    self.markers.insert(format!("M{}", idx), val);
                    self.markers.insert(format!("M{:02}", idx), val);
                    self.markers.insert(format!("R{}", idx), val);
                    self.markers.insert(format!("R{:02}", idx), val);
                    self.markers.insert(format!("%MX{}.{}", byte, bit), val);
                    if idx >= 96 {
                        let sm_idx = idx - 96;
                        self.markers.insert(format!("%SM{}", sm_idx), val);
                        self.markers.insert(format!("SM{}", sm_idx), val);
                        self.markers.insert(format!("M{}", 8000 + sm_idx), val);
                    }
                    self.markers.insert(upper, val);
                }
                _ => {}
            }
            return;
        }

        if let Some(WordKey::Counter(n)) = classify_word(&upper) {
            let e = self.counters.entry(format!("C{}", n)).or_insert((0, false, false));
            if !val {
                e.0 = 0;
                e.1 = false;
            } else {
                e.1 = true;
            }
            return;
        }
        if let Some(WordKey::Timer(n)) = classify_word(&upper) {
            let e = self.timers.entry(format!("T{}", n)).or_insert((0, false));
            if !val {
                e.0 = 0;
                e.1 = false;
            } else {
                e.1 = true;
            }
            return;
        }

        if upper.starts_with('Y') || upper.starts_with("%QX") || upper.starts_with('Q') {
            self.outputs.insert(upper, val);
        } else if upper.starts_with('M') || upper.starts_with('R') || upper.starts_with("%MX") {
            self.markers.insert(upper, val);
        } else {
            self.inputs.insert(upper, val);
        }
    }

    /// Bidirectional synchronization between physical addresses, IEC addresses, and tag symbols
    pub fn sync_io_mappings(&mut self, io_mappings: &[IOPortMapping]) {
        for (idx, io) in io_mappings.iter().enumerate() {
            let iec_addr = io.iec_address(idx);
            let sym_upper = io.symbol.trim().to_uppercase();
            if io.is_input {
                let plc_val = self.get_var_value(&io.plc_address);
                let iec_val = self.get_var_value(&iec_addr);
                let sym_val = if !sym_upper.is_empty() {
                    self.inputs.get(&sym_upper).copied().unwrap_or(false)
                } else {
                    false
                };
                let active = plc_val || iec_val || sym_val;
                self.set_var_value(&io.plc_address, active);
                self.set_var_value(&iec_addr, active);
                if !sym_upper.is_empty() {
                    self.inputs.insert(sym_upper, active);
                }
            } else {
                let plc_val = self.get_var_value(&io.plc_address);
                let iec_val = self.get_var_value(&iec_addr);
                let sym_val = if !sym_upper.is_empty() {
                    self.outputs.get(&sym_upper).copied().unwrap_or(false)
                } else {
                    false
                };
                let active = plc_val || iec_val || sym_val;
                self.set_var_value(&io.plc_address, active);
                self.set_var_value(&iec_addr, active);
                if !sym_upper.is_empty() {
                    self.outputs.insert(sym_upper, active);
                }
            }
        }
    }

    /// Start continuous simulation. Executes initial scan cycle immediately.
    pub fn start(&mut self, program: &mut LadderProgram) {
        self.state = SimState::Running;
        self.simulation_enabled = true;
        self.step_scan(program, 0);
    }

    /// Stop simulation. Enforces standard PLC safety shutdown:
    /// All physical outputs (Y/Q) are immediately forced to OFF,
    /// power flow is de-energized, and edge states are cleared.
    pub fn stop(&mut self, program: &mut LadderProgram) {
        self.state = SimState::Stopped;
        self.simulation_enabled = false;

        // PLC Safety: All physical outputs de-energize on STOP
        for val in self.outputs.values_mut() {
            *val = false;
        }
        self.edge_states.clear();

        // De-energize visual power flow on rungs and elements
        self.clear_conduction(program);
    }

    /// De-energize visual power flow on all rungs and elements (e.g. on CODE MISMATCH or paused monitoring)
    pub fn clear_conduction(&mut self, program: &mut LadderProgram) {
        for rung in &mut program.rungs {
            rung.is_energized = false;
            for el in &mut rung.elements {
                el.is_energized = false;
                el.is_done = false;
                for path in &mut el.branch_paths {
                    for sub in path {
                        sub.is_energized = false;
                        sub.is_done = false;
                    }
                }
            }
        }
    }

    /// Execute a single scan step and pause in Step mode.
    pub fn step(&mut self, program: &mut LadderProgram, dt_ms: u32) {
        self.state = SimState::Step;
        self.simulation_enabled = true;
        self.step_scan(program, dt_ms);
    }

    /// Full simulation reset: stops execution, zeroes scan count and timer,
    /// and resets all outputs, markers, registers, timers, counters and element accumulators.
    ///
    /// Like a power cycle of the real board, the RETAIN area (`M64..M95`, `D96..D127` /
    /// `%MW96..%MW127`) is preserved. Use [`reset_including_retain`](Self::reset_including_retain)
    /// to wipe it as well.
    pub fn reset(&mut self, program: &mut LadderProgram) {
        let retain_bits: Vec<bool> = (64..96).map(|i| self.get_var_value(&format!("M{}", i))).collect();
        let retain_words: Vec<i32> = (96..128).map(|i| self.read_word(&format!("%MW{}", i))).collect();
        self.reset_including_retain(program);
        for (i, v) in retain_bits.into_iter().enumerate() {
            self.set_var_value(&format!("M{}", 64 + i), v);
        }
        for (i, v) in retain_words.into_iter().enumerate() {
            self.write_word(&format!("%MW{}", 96 + i), v);
        }
    }

    /// Full reset including the RETAIN area (equivalent of "Clear RETAIN" + power cycle).
    pub fn reset_including_retain(&mut self, program: &mut LadderProgram) {
        self.stop(program);
        self.pid_states.clear();
        self.uart_tx_log.clear();
        self.uart_rx_queue.clear();
        self.scan_count = 0;
        self.simulation_time_ms = 0;

        for val in self.markers.values_mut() {
            *val = false;
        }
        for val in self.registers.values_mut() {
            *val = 0;
        }
        self.markers.insert("%SM0".to_string(), true);
        self.markers.insert("SM0".to_string(), true);
        self.markers.insert("M8000".to_string(), true);
        self.markers.insert("M96".to_string(), true);
        self.registers.insert("%SD0".to_string(), 100);
        self.registers.insert("SD0".to_string(), 100);
        self.registers.insert("D8000".to_string(), 100);
        self.registers.insert("D128".to_string(), 100);
        self.timers.clear();
        self.counters.clear();
        self.edge_states.clear();

        for rung in &mut program.rungs {
            for el in &mut rung.elements {
                el.current_accum = 0;
                el.is_done = false;
                for path in &mut el.branch_paths {
                    for sub in path {
                        sub.current_accum = 0;
                        sub.is_done = false;
                    }
                }
            }
        }
    }

    /// Execute one scan cycle across all rungs
    pub fn step_scan(&mut self, program: &mut LadderProgram, dt_ms: u32) {
        let scaled_dt = dt_ms.saturating_mul(self.speed_multiplier.max(1));
        self.scan_count += 1;
        self.simulation_time_ms += scaled_dt as u64;

        // Update Special System Markers (%SM / M8000..)
        let sm0_always_on = true;
        let sm1_always_off = false;
        let sm2_first_scan = self.scan_count <= 1;
        let sm10_clock_10ms = (self.simulation_time_ms % 20) < 10;
        let sm11_clock_100ms = (self.simulation_time_ms % 200) < 100;
        let sm12_clock_1s = (self.simulation_time_ms % 2000) < 1000;
        let sm13_clock_1min = (self.simulation_time_ms % 120_000) < 60_000;
        let sm20_error = false;
        let sm21_overrun = false;

        let sm_values: [(usize, usize, bool); 9] = [
            (0, 8000, sm0_always_on),
            (1, 8001, sm1_always_off),
            (2, 8002, sm2_first_scan),
            (10, 8011, sm10_clock_10ms),
            (11, 8012, sm11_clock_100ms),
            (12, 8013, sm12_clock_1s),
            (13, 8014, sm13_clock_1min),
            (20, 8004, sm20_error),
            (21, 8020, sm21_overrun),
        ];

        for (sm_idx, m_legacy, val) in sm_values {
            let m_addr = 96 + sm_idx;
            self.markers.insert(format!("%SM{}", sm_idx), val);
            self.markers.insert(format!("SM{}", sm_idx), val);
            self.markers.insert(format!("M{}", m_legacy), val);
            self.markers.insert(format!("M{}", m_addr), val);
        }

        // Update Special System Registers (%SD / D8000..)
        let scan_us = scaled_dt.saturating_mul(1000) as i32;
        let sd_values: [(usize, usize, usize, i32); 6] = [
            (0, 8000, 128, 100),                                         // Watchdog preset (100 ms)
            (1, 8010, 129, scan_us),                                     // Current scan time (us)
            (2, 8011, 130, scan_us),                                     // Min scan time (us)
            (3, 8012, 131, scan_us),                                     // Max scan time (us)
            (4, 8004, 132, (self.scan_count & 0xFFFF) as i32),           // Scan count low
            (5, 8005, 133, ((self.scan_count >> 16) & 0xFFFF) as i32),  // Scan count high
        ];

        for (sd_idx, d_legacy, d_addr, val) in sd_values {
            self.registers.insert(format!("%SD{}", sd_idx), val);
            self.registers.insert(format!("SD{}", sd_idx), val);
            self.registers.insert(format!("D{}", d_legacy), val);
            self.registers.insert(format!("D{}", d_addr), val);
        }

        // Re-assert forced inputs before scan cycle
        if !self.forced_bits.is_empty() {
            let in_forces: Vec<(u8, bool)> = self
                .forced_bits
                .iter()
                .filter(|(&a, _)| a < 0x20)
                .map(|(&a, &v)| (a, v))
                .collect();
            for (addr, val) in in_forces {
                self.apply_forced_bit_to_memory(addr, val);
            }
        }

        let mut jump_target: Option<String> = None;
        let mut rung_idx = 0;
        while rung_idx < program.rungs.len() {
            let rung = &mut program.rungs[rung_idx];

            if let Some(target) = &jump_target {
                let matches_label = rung.comment.trim().eq_ignore_ascii_case(target)
                    || rung.elements.iter().any(|e| {
                        (e.element_type == TypeTag::Label || e.element_type == TypeTag::Comment)
                            && e.name.trim().eq_ignore_ascii_case(target)
                    });
                if matches_label {
                    jump_target = None;
                } else {
                    rung.is_energized = false;
                    for el in &mut rung.elements {
                        el.is_energized = false;
                    }
                    rung_idx += 1;
                    continue;
                }
            }

            let mut power_flow = true;
            self.acc_bool = true;
            for elem in &mut rung.elements {
                power_flow = self.eval_element(elem, power_flow, scaled_dt);
                self.acc_bool = power_flow;
                if elem.element_type == TypeTag::Jump && power_flow {
                    jump_target = Some(elem.name.clone());
                }
            }

            rung.is_energized = power_flow;
            rung_idx += 1;
        }

        // Sync counter/timer display state in case a later rung modified or reset them (e.g. R C0 / MOVE C0 0)
        for rung in &mut program.rungs {
            for elem in &mut rung.elements {
                match elem.element_type {
                    TypeTag::Ctu | TypeTag::Ctd | TypeTag::Ctud => {
                        if let Some(&(cv, done, _)) = self.counters.get(&elem.name.to_uppercase()) {
                            elem.current_accum = cv;
                            elem.is_done = match elem.element_type {
                                TypeTag::Ctu | TypeTag::Ctud => cv >= elem.preset_value && done,
                                TypeTag::Ctd => cv <= 0 && done,
                                _ => done,
                            };
                        }
                    }
                    TypeTag::Ton | TypeTag::Tof | TypeTag::Tp => {
                        if let Some(&(cv, done)) = self.timers.get(&elem.name.to_uppercase()) {
                            elem.current_accum = cv as i32;
                            elem.is_done = done;
                        }
                    }
                    _ => {}
                }
            }
        }

        // Re-assert forced outputs and markers after ladder logic scan
        if !self.forced_bits.is_empty() {
            let out_forces: Vec<(u8, bool)> = self
                .forced_bits
                .iter()
                .filter(|(&a, _)| a >= 0x20)
                .map(|(&a, &v)| (a, v))
                .collect();
            for (addr, val) in out_forces {
                self.apply_forced_bit_to_memory(addr, val);
            }
        }
    }

    fn eval_element(&mut self, elem: &mut LadderElement, power_in: bool, dt_ms: u32) -> bool {
        let power_out = match elem.element_type {
            TypeTag::Contact => {
                let var_val = self.get_var_value(&elem.name);
                let last_val = *self.edge_states.get(&elem.id).unwrap_or(&var_val);
                self.edge_states.insert(elem.id, var_val);

                let conducts = match elem.contact_mode {
                    ContactMode::NormallyOpen => var_val,
                    ContactMode::NormallyClosed => !var_val,
                    ContactMode::PositiveTransition => var_val && !last_val,
                    ContactMode::NegativeTransition => !var_val && last_val,
                };
                elem.is_done = conducts;
                power_in && conducts
            }
            TypeTag::Coil => {
                let energized = power_in;
                let last_in = *self.edge_states.get(&elem.id).unwrap_or(&energized);
                self.edge_states.insert(elem.id, energized);

                match elem.coil_mode {
                    CoilMode::Normal => {
                        self.set_var_value(&elem.name, energized);
                    }
                    CoilMode::Negated => {
                        self.set_var_value(&elem.name, !energized);
                    }
                    CoilMode::SetLatch => {
                        if energized {
                            self.set_var_value(&elem.name, true);
                        }
                    }
                    CoilMode::ResetUnlatch => {
                        if energized {
                            self.set_var_value(&elem.name, false);
                        }
                    }
                    CoilMode::PositiveTransition => {
                        let rising = energized && !last_in;
                        self.set_var_value(&elem.name, rising);
                    }
                    CoilMode::NegativeTransition => {
                        let falling = !energized && last_in;
                        self.set_var_value(&elem.name, falling);
                    }
                }
                elem.is_done = self.get_var_value(&elem.name);
                power_in
            }
            TypeTag::Ton => {
                let entry = self.timers.entry(elem.name.to_uppercase()).or_insert((0, false));
                if power_in {
                    entry.0 = entry.0.saturating_add(dt_ms);
                    if entry.0 as i32 >= elem.preset_value {
                        entry.0 = elem.preset_value.max(0) as u32;
                        entry.1 = true;
                    }
                } else {
                    entry.0 = 0;
                    entry.1 = false;
                }
                elem.current_accum = entry.0 as i32;
                elem.is_done = entry.1;
                self.acc_word = elem.current_accum;
                power_in && entry.1
            }
            TypeTag::Tof => {
                let entry = self.timers.entry(elem.name.to_uppercase()).or_insert((0, false));
                if power_in {
                    entry.0 = 0;
                    entry.1 = true;
                } else if entry.1 {
                    let next_cv = entry.0.saturating_add(dt_ms);
                    let pt = elem.preset_value.max(0) as u32;
                    if next_cv >= pt {
                        entry.0 = pt;
                        entry.1 = false;
                    } else {
                        entry.0 = next_cv;
                    }
                }
                elem.current_accum = entry.0 as i32;
                elem.is_done = entry.1;
                self.acc_word = elem.current_accum;
                entry.1
            }
            TypeTag::Tp => {
                let entry = self.timers.entry(elem.name.to_uppercase()).or_insert((0, false));
                let last_in = *self.edge_states.get(&elem.id).unwrap_or(&power_in);
                self.edge_states.insert(elem.id, power_in);

                if power_in && !last_in && !entry.1 {
                    entry.0 = 0;
                    entry.1 = true;
                }

                if entry.1 {
                    entry.0 = entry.0.saturating_add(dt_ms);
                    if entry.0 as i32 >= elem.preset_value {
                        entry.0 = elem.preset_value.max(0) as u32;
                        entry.1 = false;
                    }
                } else if !power_in {
                    entry.0 = 0;
                }

                elem.current_accum = entry.0 as i32;
                elem.is_done = entry.1;
                self.acc_word = elem.current_accum;
                entry.1
            }
            TypeTag::Ctu | TypeTag::Ctud => {
                let last_edge = *self.edge_states.get(&elem.id).unwrap_or(&false);
                self.edge_states.insert(elem.id, power_in);
                let entry = self.counters.entry(elem.name.to_uppercase()).or_insert((0, false, false));
                entry.2 = power_in;
                if power_in && !last_edge {
                    entry.0 += 1;
                }
                entry.1 = entry.0 >= elem.preset_value;
                elem.current_accum = entry.0;
                elem.is_done = entry.1;
                self.acc_word = elem.current_accum;
                entry.1
            }
            TypeTag::Ctd => {
                let last_edge = *self.edge_states.get(&elem.id).unwrap_or(&false);
                self.edge_states.insert(elem.id, power_in);
                let entry = self.counters.entry(elem.name.to_uppercase()).or_insert((0, false, false));
                entry.2 = power_in;
                if power_in && !last_edge {
                    entry.0 = (entry.0 - 1).max(0);
                    entry.1 = entry.0 <= 0;
                } else if entry.0 > 0 && entry.0 < elem.preset_value {
                    entry.1 = false;
                }
                elem.current_accum = entry.0;
                elem.is_done = entry.1;
                self.acc_word = elem.current_accum;
                entry.1
            }
            TypeTag::Branch => {
                if elem.branch_paths.is_empty() {
                    power_in
                } else {
                    let mut any_path_conducts = false;
                    for path in &mut elem.branch_paths {
                        let mut sub_flow = power_in;
                        for sub_elem in path {
                            sub_flow = self.eval_element(sub_elem, sub_flow, dt_ms);
                        }
                        if sub_flow {
                            any_path_conducts = true;
                        }
                    }
                    elem.is_done = any_path_conducts;
                    power_in && any_path_conducts
                }
            }
            TypeTag::Wire => power_in,
            TypeTag::OpenCircuit => false,
            TypeTag::Comment => power_in,
            TypeTag::MasterRelay => power_in,
            TypeTag::Compare => {
                let reg_val = self.read_word(&elem.name);
                let op = elem.compare_op.trim().to_uppercase();
                let conducts = match op.as_str() {
                    "==" | "EQ" => reg_val == elem.preset_value,
                    "!=" | "<>" | "NE" => reg_val != elem.preset_value,
                    ">" | "GT" => reg_val > elem.preset_value,
                    "<" | "LT" => reg_val < elem.preset_value,
                    ">=" | "GE" => reg_val >= elem.preset_value,
                    "<=" | "LE" => reg_val <= elem.preset_value,
                    _ => false,
                };
                elem.current_accum = reg_val;
                elem.is_done = conducts;
                self.acc_word = reg_val;
                power_in && conducts
            }
            TypeTag::RTrig => {
                let last_in = *self.edge_states.get(&elem.id).unwrap_or(&power_in);
                self.edge_states.insert(elem.id, power_in);
                let conducts = power_in && !last_in;
                elem.is_done = conducts;
                power_in && conducts
            }
            TypeTag::FTrig => {
                let last_in = *self.edge_states.get(&elem.id).unwrap_or(&power_in);
                self.edge_states.insert(elem.id, power_in);
                let conducts = !power_in && last_in;
                elem.is_done = conducts;
                conducts
            }
            TypeTag::Math => {
                if power_in {
                    let dest = elem.word_target().to_string();
                    let cur = self.read_word(&dest);
                    let res = match elem.math_op.as_str() {
                        "+" => cur.saturating_add(elem.preset_value),
                        "-" => cur.saturating_sub(elem.preset_value),
                        "*" => cur.saturating_mul(elem.preset_value),
                        "/" => if elem.preset_value != 0 { cur / elem.preset_value } else { cur },
                        "%" => if elem.preset_value != 0 { cur % elem.preset_value } else { cur },
                        _ => cur,
                    };
                    self.write_word(&dest, res);
                    elem.current_accum = res;
                    self.acc_word = res;
                }
                power_in
            }
            TypeTag::Move => {
                if power_in {
                    let dest = elem.word_target().to_string();
                    self.write_word(&dest, elem.preset_value);
                    elem.current_accum = elem.preset_value;
                    self.acc_word = elem.preset_value;
                }
                elem.is_done = power_in;
                power_in
            }
            TypeTag::Sr => {
                let r_sig = if !elem.target_reg.is_empty() {
                    self.get_var_value(&elem.target_reg)
                } else if !elem.symbol.is_empty() {
                    self.get_var_value(&elem.symbol)
                } else {
                    false
                };
                let prev_q = if !elem.name.is_empty() {
                    self.get_var_value(&elem.name)
                } else {
                    elem.is_done
                };
                // SR: Set dominant -> Q1 = S1 || (!R && Q1_prev)
                let q1 = power_in || (!r_sig && prev_q);
                if !elem.name.is_empty() {
                    self.set_var_value(&elem.name, q1);
                }
                elem.is_done = q1;
                q1
            }
            TypeTag::Rs => {
                let r1_sig = if !elem.target_reg.is_empty() {
                    self.get_var_value(&elem.target_reg)
                } else if !elem.symbol.is_empty() {
                    self.get_var_value(&elem.symbol)
                } else {
                    false
                };
                let prev_q = if !elem.name.is_empty() {
                    self.get_var_value(&elem.name)
                } else {
                    elem.is_done
                };
                // RS: Reset dominant -> Q1 = !R1 && (S || Q1_prev)
                let q1 = !r1_sig && (power_in || prev_q);
                if !elem.name.is_empty() {
                    self.set_var_value(&elem.name, q1);
                }
                elem.is_done = q1;
                q1
            }
            TypeTag::Jump => {
                elem.is_done = power_in;
                power_in
            }
            TypeTag::Label => {
                elem.is_done = true;
                power_in
            }
            TypeTag::Adc => {
                if power_in {
                    let v = self.read_word(&elem.name);
                    self.write_word(&elem.target_reg, v);
                    elem.current_accum = v;
                    self.acc_word = v;
                }
                elem.is_done = power_in;
                power_in
            }
            TypeTag::Bitwise => {
                if power_in {
                    let op = bitwise_opcode(&elem.math_op);
                    let a = self.read_word(&elem.fb.in1);
                    let b = self.read_word(&elem.fb.in2);
                    let r = uplc_vm::bitwise(op, a, b);
                    self.write_word(&elem.target_reg, r);
                    elem.current_accum = r;
                    self.acc_word = r;
                }
                elem.is_done = power_in;
                power_in
            }
            TypeTag::Scale => {
                if power_in {
                    let k = elem.fb.k;
                    let r = uplc_vm::scale(self.read_word(&elem.fb.in1), k[0], k[1], k[2], k[3]);
                    self.write_word(&elem.target_reg, r);
                    elem.current_accum = r;
                    self.acc_word = r;
                }
                elem.is_done = power_in;
                power_in
            }
            TypeTag::Limit => {
                if power_in {
                    let k = elem.fb.k;
                    let r = uplc_vm::limit(self.read_word(&elem.fb.in1), k[0], k[1]);
                    self.write_word(&elem.target_reg, r);
                    elem.current_accum = r;
                    self.acc_word = r;
                }
                elem.is_done = power_in;
                power_in
            }
            TypeTag::Pid => {
                let key = elem.name.trim().to_ascii_uppercase();
                if power_in {
                    let pv = self.read_word(&elem.fb.in1);
                    let sp = self.read_word(&elem.fb.in2);
                    let k = elem.fb.k;
                    let st = self.pid_states.entry(key).or_insert_with(PidState::new);
                    let cv = st.step(pv, sp, k[0], k[1], k[2], k[3], dt_ms);
                    self.write_word(&elem.target_reg, cv);
                    elem.current_accum = cv;
                    self.acc_word = cv;
                } else if let Some(st) = self.pid_states.get_mut(&key) {
                    st.reset();
                }
                elem.is_done = power_in;
                power_in
            }
            TypeTag::ShiftRegisters => {
                if self.fb_rising_edge(elem.id, power_in) {
                    let input = self.read_word(&elem.fb.in1);
                    let len = elem.preset_value.clamp(1, 32) as usize;
                    if let Some((base, _, _)) = BytecodeCompiler::parse_direct_address(&elem.name, true) {
                        let base = base as usize;
                        let last = (base + len - 1).min(255);
                        let word_name = |a: usize| BytecodeCompiler::format_address(a as u8, true).1;
                        let mut i = last;
                        while i > base {
                            let v = self.read_word(&word_name(i - 1));
                            self.write_word(&word_name(i), v);
                            i -= 1;
                        }
                    }
                    self.write_word(&elem.name, input);
                    elem.current_accum = input;
                    self.acc_word = input;
                }
                elem.is_done = power_in;
                power_in
            }
            TypeTag::ShiftRegInbit => {
                if self.fb_rising_edge(elem.id, power_in) {
                    let bit = i32::from(self.read_bit(&elem.fb.in1));
                    let r = uplc_vm::shift_in_bit(self.read_word(&elem.name), bit);
                    self.write_word(&elem.name, r);
                    elem.current_accum = r;
                    self.acc_word = r;
                }
                elem.is_done = power_in;
                power_in
            }
            TypeTag::ShiftRegReset => {
                if power_in {
                    self.write_word(&elem.name, 0);
                    elem.current_accum = 0;
                }
                elem.is_done = power_in;
                power_in
            }
            TypeTag::ShiftLeft | TypeTag::ShiftRight | TypeTag::ShiftVariable => {
                if self.fb_rising_edge(elem.id, power_in) {
                    let cur = self.read_word(&elem.name);
                    let r = match elem.element_type {
                        TypeTag::ShiftLeft => uplc_vm::shift_left(cur, elem.preset_value),
                        TypeTag::ShiftRight => uplc_vm::shift_right(cur, elem.preset_value),
                        _ => uplc_vm::shift_left(cur, self.read_word(&elem.fb.in1)),
                    };
                    self.write_word(&elem.name, r);
                    elem.current_accum = r;
                    self.acc_word = r;
                }
                elem.is_done = power_in;
                power_in
            }
            TypeTag::UartSend => {
                if self.fb_rising_edge(elem.id, power_in) {
                    let job = UartTx { instance: 0, mode: elem.fb.mode, value: self.read_word(&elem.fb.in1) };
                    let mut buf = [0u8; 16];
                    let n = job.encode(&mut buf);
                    self.uart_tx_log.extend_from_slice(&buf[..n]);
                    elem.current_accum = job.value;
                }
                elem.is_done = power_in;
                power_in
            }
            TypeTag::UartRecv => {
                let mut got = false;
                if power_in {
                    if let Some(b) = self.uart_rx_queue.pop_front() {
                        self.write_word(&elem.target_reg, b as i32);
                        elem.current_accum = b as i32;
                        got = true;
                    }
                }
                elem.is_done = got;
                got
            }
            _ => power_in,
        };

        elem.is_energized = power_out;
        power_out
    }

    /// Capture a complete Online Monitoring Status Snapshot from the current simulator state
    pub fn capture_status_snapshot(&self) -> PlcStatusSnapshot {
        let mut bit_mem = [0u8; 256];
        // Digital Inputs 0x00..0x1F
        for i in 0..16 {
            if *self.inputs.get(&format!("X{}", i)).unwrap_or(&false) {
                bit_mem[i] = 1;
            }
        }
        // Digital Outputs 0x20..0x3F
        for i in 0..16 {
            if *self.outputs.get(&format!("Y{}", i)).unwrap_or(&false) {
                bit_mem[0x20 + i] = 1;
            }
        }
        // Internal Markers 0x40..0x9F (M0..M95)
        for i in 0..96 {
            if *self.markers.get(&format!("M{}", i)).unwrap_or(&false) {
                bit_mem[0x40 + i] = 1;
            }
        }
        // Special System Markers 0xA0..0xBF (%SM0..%SM31)
        for i in 0..32 {
            if *self.markers.get(&format!("%SM{}", i)).or_else(|| self.markers.get(&format!("M{}", 96 + i))).unwrap_or(&false) {
                bit_mem[0xA0 + i] = 1;
            }
        }
        // Timers 0xC0..0xCF
        let mut timer_cv = [0u32; 16];
        for i in 0..16 {
            if let Some(&(accum, done)) = self.timers.get(&format!("T{}", i)) {
                bit_mem[0xC0 + i] = u8::from(done);
                timer_cv[i] = accum;
            }
        }
        // Counters 0xE0..0xEF
        let mut counter_cv = [0i32; 16];
        for i in 0..16 {
            if let Some(&(count, done, _)) = self.counters.get(&format!("C{}", i)) {
                bit_mem[0xE0 + i] = u8::from(done);
                counter_cv[i] = count;
            }
        }
        // Data Registers D0..D15 / %MW0..%MW15
        let mut data_reg = [0i32; 16];
        for i in 0..16 {
            data_reg[i] = *self.registers.get(&format!("D{}", i)).unwrap_or(&0);
        }

        PlcStatusSnapshot {
            running: self.state == SimState::Running,
            scan_count: self.scan_count as u32,
            acc_bool: self.acc_bool,
            acc_word: self.acc_word,
            bit_mem,
            timer_cv,
            counter_cv,
            data_reg,
            prog_len: 0,
            prog_crc: 0,
            fw_feature_level: uplc_vm::FW_FEATURE_LEVEL,
        }
    }

    /// Apply an Online Monitoring Status Snapshot (from RP2040 Micro-VM or Simulator) into memory
    /// Convert a raw 12-bit on-chip temperature sensor ADC reading (AI3 / `%IW3`, 3.3 V reference)
    /// into 0.1 °C units. The raw value is clamped to `0..=4095` and computed in `i64` (overflow-safe).
    /// - RP2040:  `T = 27 - (Vbe - 0.706 V) / 1.721 mV`
    /// - STM32F1: `T = 25 + (1.43 V - Vsense) / 4.3 mV`
    pub fn cpu_temp_c01_from_raw(raw: i32, stm32f1: bool) -> i32 {
        let raw = i64::from(raw.clamp(0, 4095));
        let uv = raw * 3_300_000 / 4096;
        let t = if stm32f1 {
            250 + (1_430_000 - uv) / 430
        } else {
            270 - (uv - 706_000) / 1721
        };
        t as i32
    }

    /// Publish `%SD6` (CPU temperature, 0.1 °C) under all its register aliases
    pub fn set_cpu_temp_registers(&mut self, temp_c01: i32) {
        for key in ["%SD6", "SD6", "D8006", "D134"] {
            self.registers.insert(key.to_string(), temp_c01);
        }
    }

    pub fn apply_online_snapshot(&mut self, snapshot: &PlcStatusSnapshot) {
        self.scan_count = snapshot.scan_count as u64;
        self.remote_running = snapshot.running;
        self.simulation_enabled = snapshot.running;
        self.online_mode = true;
        self.acc_bool = snapshot.acc_bool;
        self.acc_word = snapshot.acc_word;

        // 1. Digital Inputs: X0..X15 / %IX0.0..%IX1.7
        for i in 0..16 {
            let byte = i / 8;
            let bit = i % 8;
            let val = snapshot.bit_mem[i] != 0;
            self.inputs.insert(format!("X{}", i), val);
            self.inputs.insert(format!("X{:02}", i), val);
            self.inputs.insert(format!("%IX{}.{}", byte, bit), val);
        }

        // 2. Digital Outputs: Y0..Y15 / %QX0.0..%QX1.7
        for i in 0..16 {
            let byte = i / 8;
            let bit = i % 8;
            let val = snapshot.bit_mem[0x20 + i] != 0;
            self.outputs.insert(format!("Y{}", i), val);
            self.outputs.insert(format!("Y{:02}", i), val);
            self.outputs.insert(format!("%QX{}.{}", byte, bit), val);
        }

        // 3. Internal Relays / Markers: M0..M95 / %MX0.0..%MX11.7
        for i in 0..96 {
            let byte = i / 8;
            let bit = i % 8;
            let val = snapshot.bit_mem[0x40 + i] != 0;
            self.markers.insert(format!("M{}", i), val);
            self.markers.insert(format!("M{:02}", i), val);
            self.markers.insert(format!("R{}", i), val);
            self.markers.insert(format!("R{:02}", i), val);
            self.markers.insert(format!("%MX{}.{}", byte, bit), val);
        }

        // 3b. Special System Markers: %SM0..%SM31 / M8000..M8031 (0xA0..0xBF)
        for i in 0..32 {
            let val = (snapshot.bit_mem[0xA0 + i] & 0x01) != 0;
            self.markers.insert(format!("%SM{}", i), val);
            self.markers.insert(format!("SM{}", i), val);
            self.markers.insert(format!("M{}", 96 + i), val);
            self.markers.insert(format!("M{}", 8000 + i), val);
        }

        // 4. Timers: T0..T15
        for i in 0..16 {
            let done = snapshot.bit_mem[0xC0 + i] != 0;
            let accum = snapshot.timer_cv[i];
            self.timers.insert(format!("T{}", i), (accum, done));
            self.timers.insert(format!("T{:02}", i), (accum, done));
        }

        // 5. Counters: C0..C15
        for i in 0..16 {
            let done = snapshot.bit_mem[0xE0 + i] != 0;
            let count = snapshot.counter_cv[i];
            self.counters.insert(format!("C{}", i), (count, done, false));
            self.counters.insert(format!("C{:02}", i), (count, done, false));
        }

        // 6. Data Registers & Analog Inputs: D0..D15 / %MW0..%MW15 and AI0..AI15 / %IW0..%IW15
        for i in 0..16 {
            let val = snapshot.data_reg[i];
            self.registers.insert(format!("D{}", i), val);
            self.registers.insert(format!("D{:02}", i), val);
            self.registers.insert(format!("V{}", i), val);
            self.registers.insert(format!("V{:02}", i), val);
            self.registers.insert(format!("%MW{}", i), val);
            self.registers.insert(format!("AI{}", i), val);
            self.registers.insert(format!("AI{:02}", i), val);
            self.registers.insert(format!("%IW{}", i), val);
        }

        // 6b. Special System Registers: %SD0..%SD7 from Snapshot telemetry
        let scan_low = (snapshot.scan_count & 0xFFFF) as i32;
        let scan_high = ((snapshot.scan_count >> 16) & 0xFFFF) as i32;
        self.registers.insert("%SD0".to_string(), 100);
        self.registers.insert("SD0".to_string(), 100);
        self.registers.insert("D8000".to_string(), 100);
        self.registers.insert("D128".to_string(), 100);
        self.registers.insert("%SD4".to_string(), scan_low);
        self.registers.insert("SD4".to_string(), scan_low);
        self.registers.insert("D8004".to_string(), scan_low);
        self.registers.insert("D132".to_string(), scan_low);
        self.registers.insert("%SD5".to_string(), scan_high);
        self.registers.insert("SD5".to_string(), scan_high);
        self.registers.insert("D8005".to_string(), scan_high);
        self.registers.insert("D133".to_string(), scan_high);

        // %SD6: CPU Junction Temperature in 0.1 °C units derived from AI3 (%IW3)
        // (RP2040 formula by default; `LadderDocument` re-applies the STM32 formula for STM32 targets)
        let temp_c01 = Self::cpu_temp_c01_from_raw(snapshot.data_reg[3], false);
        self.set_cpu_temp_registers(temp_c01);

        // %SD7: RETAIN Flash save count encoded in bit 1 of bit_mem[0xA0..0xB0]
        let mut sd7: u16 = 0;
        for b in 0..16 {
            if (snapshot.bit_mem[0xA0 + b] & 0x02) != 0 {
                sd7 |= 1u16 << b;
            }
        }
        let sd7_val = sd7 as i32;
        self.registers.insert("%SD7".to_string(), sd7_val);
        self.registers.insert("SD7".to_string(), sd7_val);
        self.registers.insert("D8007".to_string(), sd7_val);
        self.registers.insert("D135".to_string(), sd7_val);

        // 7. Re-apply any local force overrides (in case snapshot packet was in flight before force command arrived)
        if !self.forced_bits.is_empty() {
            let forces: Vec<(u8, bool)> = self.forced_bits.iter().map(|(&a, &v)| (a, v)).collect();
            for (addr, val) in forces {
                self.apply_forced_bit_to_memory(addr, val);
            }
        }
    }

    /// Re-evaluates conduction states (is_done, is_energized, rung.is_energized) without mutating variables.
    /// Used for Online Monitoring to light up wires, contacts, coils, timers, and parallel branches in real time.
    pub fn evaluate_conduction_only(&mut self, program: &mut LadderProgram) {
        let mut jump_target: Option<String> = None;
        let mut rung_idx = 0;
        while rung_idx < program.rungs.len() {
            let rung = &mut program.rungs[rung_idx];

            if let Some(target) = &jump_target {
                let matches_label = rung.comment.trim().eq_ignore_ascii_case(target)
                    || rung.elements.iter().any(|e| {
                        (e.element_type == TypeTag::Label || e.element_type == TypeTag::Comment)
                            && e.name.trim().eq_ignore_ascii_case(target)
                    });
                if matches_label {
                    jump_target = None;
                } else {
                    rung.is_energized = false;
                    for el in &mut rung.elements {
                        el.is_energized = false;
                        for path in &mut el.branch_paths {
                            for sub in path {
                                sub.is_energized = false;
                            }
                        }
                    }
                    rung_idx += 1;
                    continue;
                }
            }

            let mut power_flow = true;
            for elem in &mut rung.elements {
                power_flow = self.eval_element_read_only(elem, power_flow);
                if elem.element_type == TypeTag::Jump && power_flow {
                    jump_target = Some(elem.name.clone());
                }
            }
            rung.is_energized = power_flow;
            rung_idx += 1;
        }

        // Sync counter/timer display state in case a later rung modified or reset them (e.g. R C0 / MOVE C0 0)
        for rung in &mut program.rungs {
            for elem in &mut rung.elements {
                match elem.element_type {
                    TypeTag::Ctu | TypeTag::Ctd | TypeTag::Ctud => {
                        if let Some(&(cv, done, _)) = self.counters.get(&elem.name.to_uppercase()) {
                            elem.current_accum = cv;
                            elem.is_done = match elem.element_type {
                                TypeTag::Ctu | TypeTag::Ctud => cv >= elem.preset_value && done,
                                TypeTag::Ctd => cv <= 0 && done,
                                _ => done,
                            };
                        }
                    }
                    TypeTag::Ton | TypeTag::Tof | TypeTag::Tp => {
                        if let Some(&(cv, done)) = self.timers.get(&elem.name.to_uppercase()) {
                            elem.current_accum = cv as i32;
                            elem.is_done = done;
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    fn eval_element_read_only(&mut self, elem: &mut LadderElement, power_in: bool) -> bool {
        let conducts = match elem.element_type {
            TypeTag::Contact => {
                let var_val = self.get_var_value(&elem.name);
                let c = match elem.contact_mode {
                    ContactMode::NormallyOpen => var_val,
                    ContactMode::NormallyClosed => !var_val,
                    ContactMode::PositiveTransition => var_val,
                    ContactMode::NegativeTransition => !var_val,
                };
                elem.is_done = c;
                power_in && c
            }
            TypeTag::Coil => {
                elem.is_done = self.get_var_value(&elem.name);
                power_in
            }
            TypeTag::Ton => {
                if let Some(entry) = self.timers.get(&elem.name.to_uppercase()) {
                    elem.current_accum = entry.0 as i32;
                    elem.is_done = entry.1;
                }
                power_in && elem.is_done
            }
            TypeTag::Tof | TypeTag::Tp => {
                if let Some(entry) = self.timers.get(&elem.name.to_uppercase()) {
                    elem.current_accum = entry.0 as i32;
                    elem.is_done = entry.1;
                }
                elem.is_done
            }
            TypeTag::Ctu | TypeTag::Ctd | TypeTag::Ctud => {
                if let Some(entry) = self.counters.get(&elem.name.to_uppercase()) {
                    elem.current_accum = entry.0;
                    elem.is_done = entry.1;
                }
                elem.is_done
            }
            TypeTag::Branch => {
                if elem.branch_paths.is_empty() {
                    power_in
                } else {
                    let mut any_path_conducts = false;
                    for path in &mut elem.branch_paths {
                        let mut sub_flow = power_in;
                        for sub_elem in path {
                            sub_flow = self.eval_element_read_only(sub_elem, sub_flow);
                        }
                        if sub_flow {
                            any_path_conducts = true;
                        }
                    }
                    elem.is_done = any_path_conducts;
                    power_in && any_path_conducts
                }
            }
            TypeTag::OpenCircuit => {
                elem.is_done = false;
                false
            }
            TypeTag::Compare => {
                let reg_val = self.read_word(&elem.name);
                let op = elem.compare_op.trim().to_uppercase();
                let c = match op.as_str() {
                    "==" | "EQ" => reg_val == elem.preset_value,
                    "!=" | "<>" | "NE" => reg_val != elem.preset_value,
                    ">" | "GT" => reg_val > elem.preset_value,
                    "<" | "LT" => reg_val < elem.preset_value,
                    ">=" | "GE" => reg_val >= elem.preset_value,
                    "<=" | "LE" => reg_val <= elem.preset_value,
                    _ => false,
                };
                elem.current_accum = reg_val;
                elem.is_done = c;
                power_in && c
            }
            TypeTag::Math | TypeTag::Move => {
                elem.current_accum = self.read_word(elem.word_target());
                elem.is_done = power_in;
                power_in
            }
            TypeTag::Adc | TypeTag::Bitwise | TypeTag::Scale | TypeTag::Limit | TypeTag::Pid | TypeTag::UartRecv => {
                elem.current_accum = self.read_word(&elem.target_reg);
                elem.is_done = power_in;
                power_in
            }
            TypeTag::ShiftRegisters
            | TypeTag::ShiftRegInbit
            | TypeTag::ShiftRegReset
            | TypeTag::ShiftLeft
            | TypeTag::ShiftRight
            | TypeTag::ShiftVariable => {
                elem.current_accum = self.read_word(&elem.name);
                elem.is_done = power_in;
                power_in
            }
            TypeTag::Sr | TypeTag::Rs => {
                let q1 = if !elem.name.is_empty() {
                    self.get_var_value(&elem.name)
                } else {
                    power_in
                };
                elem.is_done = q1;
                q1
            }
            TypeTag::Label => {
                elem.is_done = true;
                power_in
            }
            _ => {
                elem.is_done = power_in;
                power_in
            }
        };

        elem.is_energized = conducts;
        conducts
    }
}

#[cfg(test)]
mod cpu_temp_tests {
    use super::PlcSimulator;

    #[test]
    fn cpu_temp_conversion_is_overflow_safe() {
        // Full-scale / out-of-range inputs must not panic (previously `raw as u32 * 3_300_000` overflowed)
        for raw in [i32::MIN, -1, 0, 1750, 4095, 4096, i32::MAX] {
            let _ = PlcSimulator::cpu_temp_c01_from_raw(raw, false);
            let _ = PlcSimulator::cpu_temp_c01_from_raw(raw, true);
        }
        // STM32F1: Vsense = 1.43 V (raw ≈ 1775) -> 25.0 °C
        assert!((PlcSimulator::cpu_temp_c01_from_raw(1775, true) - 250).abs() <= 2);
        // RP2040: Vbe = 0.706 V (raw ≈ 876) -> 27.0 °C
        assert!((PlcSimulator::cpu_temp_c01_from_raw(876, false) - 270).abs() <= 2);
    }
}
