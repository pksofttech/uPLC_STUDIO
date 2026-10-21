use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TypeTag {
    Comment,
    Contact,
    Coil,
    Ton,
    Tof,
    #[serde(alias = "Tpc")]
    Tp,
    Ctu,
    Ctd,
    #[serde(alias = "Ctc")]
    Ctud,
    MasterRelay,
    Compare,
    Math,
    Move,
    Adc,
    ShiftRegisters,
    ShiftVariable,
    ShiftLeft,
    ShiftRight,
    ShiftRegReset,
    ShiftRegInbit,
    #[serde(alias = "OneShotRising")]
    RTrig,
    #[serde(alias = "OneShotFalling")]
    FTrig,
    Sr,
    Rs,
    Jump,
    Label,
    UartSend,
    UartRecv,
    MbMaster,
    /// PID controller FB (PV=fb.in1, SP=fb.in2, CV=target_reg, k=[Kp*100, Ki*100, Kd*100, out_max])
    Pid,
    /// Linear scaling FB (IN=fb.in1, OUT=target_reg, k=[in_min, in_max, out_min, out_max])
    Scale,
    /// Clamp FB (IN=fb.in1, OUT=target_reg, k=[min, max])
    Limit,
    /// Word bitwise FB (OUT=target_reg, A=fb.in1, B=fb.in2, op in `math_op`: AND|OR|XOR|NOT|SHL|SHR)
    Bitwise,
    Wire,
    OpenCircuit,
    Branch,
}

impl TypeTag {
    #[allow(non_upper_case_globals)]
    pub const Tpc: TypeTag = TypeTag::Tp;
    #[allow(non_upper_case_globals)]
    pub const Ctc: TypeTag = TypeTag::Ctud;
    #[allow(non_upper_case_globals)]
    pub const OneShotRising: TypeTag = TypeTag::RTrig;
    #[allow(non_upper_case_globals)]
    pub const OneShotFalling: TypeTag = TypeTag::FTrig;

    pub fn display_name(&self) -> &'static str {
        match self {
            TypeTag::Comment => "Comment",
            TypeTag::Contact => "Contacts",
            TypeTag::Coil => "Coil",
            TypeTag::Ton => "TON (Delay On Timer)",
            TypeTag::Tof => "TOF (Delay Off Timer)",
            TypeTag::Tp => "TP (Pulse Timer)",
            TypeTag::Ctu => "CTU (Count Up)",
            TypeTag::Ctd => "CTD (Count Down)",
            TypeTag::Ctud => "CTUD (Up/Down Counter)",
            TypeTag::MasterRelay => "Master Relay (MC)",
            TypeTag::Compare => "Compare (EQ/NE/GT/LT/GE/LE)",
            TypeTag::Math => "Math (ADD/SUB/MUL/DIV/MOD)",
            TypeTag::Move => "MOVE",
            TypeTag::Sr => "SR (Set-Dominant Bistable FB)",
            TypeTag::Rs => "RS (Reset-Dominant Bistable FB)",
            TypeTag::Jump => "JMP (Jump to Label)",
            TypeTag::Label => "LBL (Target Label)",
            TypeTag::Adc => "ADC (Analog Read)",
            TypeTag::ShiftRegisters => "Shift Register",
            TypeTag::ShiftVariable => "Shift Variable",
            TypeTag::ShiftLeft => "Shift Left (SHL)",
            TypeTag::ShiftRight => "Shift Right (SHR)",
            TypeTag::ShiftRegReset => "Shift Reset",
            TypeTag::ShiftRegInbit => "Shift InBit",
            TypeTag::RTrig => "R_TRIG (Rising Edge Detector)",
            TypeTag::FTrig => "F_TRIG (Falling Edge Detector)",
            TypeTag::UartSend => "UART Send",
            TypeTag::UartRecv => "UART Recv",
            TypeTag::MbMaster => "MB_MASTER (Modbus RTU Master FB)",
            TypeTag::Pid => "PID (Process Controller)",
            TypeTag::Scale => "SCALE (Linear Scaling)",
            TypeTag::Limit => "LIMIT (Clamp Min/Max)",
            TypeTag::Bitwise => "Bitwise (AND/OR/XOR/NOT/SHL/SHR)",
            TypeTag::Wire => "Short Circuit",
            TypeTag::OpenCircuit => "Open Circuit",
            TypeTag::Branch => "Parallel Branch",
        }
    }

    pub fn is_right_aligned(&self) -> bool {
        matches!(
            self,
            TypeTag::Coil
                | TypeTag::Ctu
                | TypeTag::Ctd
                | TypeTag::Ctud
                | TypeTag::MasterRelay
                | TypeTag::Move
                | TypeTag::Math
                | TypeTag::Jump
                | TypeTag::Adc
                | TypeTag::MbMaster
                | TypeTag::ShiftRegisters
                | TypeTag::ShiftVariable
                | TypeTag::ShiftLeft
                | TypeTag::ShiftRight
                | TypeTag::ShiftRegReset
                | TypeTag::ShiftRegInbit
                | TypeTag::Scale
                | TypeTag::Limit
                | TypeTag::Pid
                | TypeTag::Bitwise
                | TypeTag::UartRecv
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContactMode {
    NormallyOpen,       // --[ ]--
    NormallyClosed,     // --[/]--
    #[serde(alias = "RisingEdge", alias = "PositiveTransition")]
    PositiveTransition, // --[P]--
    #[serde(alias = "FallingEdge", alias = "NegativeTransition")]
    NegativeTransition, // --[N]--
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContactStyle {
    /// Standard IEC 61131-3 PLC Ladder: —[ ]— and —[/]—
    IecStandard,
    /// Electrical Schematic JIC / NEMA: —○  / ○— (Knife Blade / Mechanical Switch)
    JicSchematic,
    /// Industrial Pushbutton Symbol: —○ ┴ ○— (Pushbutton Operator / E-Stop)
    PushButton,
    /// European DIN EN 60617: —/ — (Hinged Lever Arm Contact)
    DinEuropean,
    /// Modern Electronic Toggle Switch: ⏻ Toggle Switch with Status Indicator
    ToggleSwitch,
}

impl Default for ContactStyle {
    fn default() -> Self {
        Self::IecStandard
    }
}

impl ContactStyle {
    pub const ALL: [Self; 5] = [
        Self::IecStandard,
        Self::JicSchematic,
        Self::PushButton,
        Self::DinEuropean,
        Self::ToggleSwitch,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::IecStandard => "IEC 61131-3 (Standard)",
            Self::JicSchematic => "JIC Schematic (Knife Switch)",
            Self::PushButton => "Push Button (Industrial)",
            Self::DinEuropean => "DIN EN 60617 (European)",
            Self::ToggleSwitch => "Toggle Switch (HMI / Modern)",
        }
    }

    pub fn short_name(&self) -> &'static str {
        match self {
            Self::IecStandard => "IEC [ ]",
            Self::JicSchematic => "JIC ○/○",
            Self::PushButton => "Button ┴",
            Self::DinEuropean => "DIN /",
            Self::ToggleSwitch => "Toggle ⏻",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::IecStandard => "Standard PLC ladder symbols with parallel plates —[ ]— and —[/]—",
            Self::JicSchematic => "American / JIC wiring schematic with rotating knife-blade contact —○ / ○—",
            Self::PushButton => "Industrial pushbutton plunger switch with mushroom cap and contact bridge",
            Self::DinEuropean => "European DIN EN 60617 electrical diagram single-line hinged switch",
            Self::ToggleSwitch => "Modern electronic HMI rocker/toggle switch with ON/OFF LED indicator",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoilMode {
    Normal,             // --( )--
    Negated,            // --(/)--
    SetLatch,           // --(S)--
    ResetUnlatch,       // --(R)--
    #[serde(alias = "PositiveTransition")]
    PositiveTransition, // --(P)--
    #[serde(alias = "NegativeTransition")]
    NegativeTransition, // --(N)--
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoilStyle {
    /// Standard IEC 61131-3 PLC Ladder: —( )— curved parentheses
    IecParentheses,
    /// Classic European DIN EN 60617 & Japanese JIS: —○— circular relay coil
    CircleRelay,
    /// American NEMA / JIC Motor Starter & Relay Box: —[ ]— rectangular box
    NemaBox,
    /// Industrial Electromechanical Solenoid Valve Actuator: —[ ⧖ ]—
    SolenoidValve,
    /// Modern Industrial Pilot Lamp / Indicator Light: —( ⨂ )— with crosshair lens
    PilotLight,
}

impl Default for CoilStyle {
    fn default() -> Self {
        Self::IecParentheses
    }
}

impl CoilStyle {
    pub const ALL: [Self; 5] = [
        Self::IecParentheses,
        Self::CircleRelay,
        Self::NemaBox,
        Self::SolenoidValve,
        Self::PilotLight,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::IecParentheses => "IEC 61131-3 (Parentheses)",
            Self::CircleRelay => "DIN / JIS Schematic (Circle)",
            Self::NemaBox => "NEMA / JIC Box (Starter)",
            Self::SolenoidValve => "Solenoid Valve Actuator",
            Self::PilotLight => "Pilot Lamp / Indicator Light",
        }
    }

    pub fn short_name(&self) -> &'static str {
        match self {
            Self::IecParentheses => "IEC ( )",
            Self::CircleRelay => "Circle ○",
            Self::NemaBox => "NEMA [ ]",
            Self::SolenoidValve => "Solenoid ⧖",
            Self::PilotLight => "Lamp ⨂",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::IecParentheses => "Standard PLC ladder curved parentheses —( )—, —(/)—, —(S)—, —(R)—",
            Self::CircleRelay => "European DIN EN 60617 & Japanese JIS circular relay coil symbol —○—",
            Self::NemaBox => "American NEMA / JIC motor starter and control relay box symbol —[ ]—",
            Self::SolenoidValve => "Industrial electromechanical pneumatic/hydraulic solenoid valve symbol",
            Self::PilotLight => "Industrial illuminated pilot indicator lamp with optical crosshair lens",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SchematicStyle {
    /// Standard IEC 61131-3: Switch —[ ]— / Coil —( )—
    #[default]
    IecStandard,
    /// American JIC / NEMA: Switch —○/○— / Coil —[ M ]—
    JicNema,
    /// European DIN EN 60617 / JIS: Switch —●/ — / Coil —○—
    DinEuropean,
    /// Control Panel Wiring: Switch Push Button —○-┴-○— / Coil Pilot Lamp —( ⨂ )—
    PushBtnPilot,
}

impl SchematicStyle {
    pub const ALL: [Self; 4] = [
        Self::IecStandard,
        Self::JicNema,
        Self::DinEuropean,
        Self::PushBtnPilot,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::IecStandard => "IEC 61131-3 Standard (—[ ]— / —( )—)",
            Self::JicNema => "JIC / NEMA American (—○/○— / —[ M ]—)",
            Self::DinEuropean => "European DIN / JIS (—●/ — / —○—)",
            Self::PushBtnPilot => "Push Button & Pilot Lamp",
        }
    }

    pub fn short_name(&self) -> &'static str {
        match self {
            Self::IecStandard => "IEC —( )—",
            Self::JicNema => "JIC —[ ]—",
            Self::DinEuropean => "DIN —○—",
            Self::PushBtnPilot => "Panel —(⨂)—",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::IecStandard => "Standard PLC ladder symbols (Contacts: —[ ]—, Coils: —( )—)",
            Self::JicNema => "American JIC / NEMA (Contacts: —○/○—, Coils: —[ M ]—)",
            Self::DinEuropean => "European DIN EN 60617 & Japanese JIS (Contacts: —●/ —, Coils: —○—)",
            Self::PushBtnPilot => "Control panel representation (Push buttons & illuminated lamps)",
        }
    }

    pub fn contact_style(&self) -> ContactStyle {
        match self {
            Self::IecStandard => ContactStyle::IecStandard,
            Self::JicNema => ContactStyle::JicSchematic,
            Self::DinEuropean => ContactStyle::DinEuropean,
            Self::PushBtnPilot => ContactStyle::PushButton,
        }
    }

    pub fn coil_style(&self) -> CoilStyle {
        match self {
            Self::IecStandard => CoilStyle::IecParentheses,
            Self::JicNema => CoilStyle::NemaBox,
            Self::DinEuropean => CoilStyle::CircleRelay,
            Self::PushBtnPilot => CoilStyle::PilotLight,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertPosition {
    AppendRung,
    Left,
    Right,
    TopParallel,
    BottomParallel,
}

/// Generic parameters for function blocks added in feature level 2
/// (PID / SCALE / LIMIT / Bitwise / Shift / UART).
///
/// * `in1` / `in2`: operand A / B — a register (`"%MW3"`, `"D3"`, `"%IW0"`, `"%IX0.1"`)
///   or a literal (`"100"`, `"-5"`, `"16#FF"`).
/// * `k`: block constants (SCALE: in_min,in_max,out_min,out_max; LIMIT: min,max;
///   PID: Kp×100, Ki×100, Kd×100, out_max).
/// * `mode`: sub-option (UartSend: 0 = low byte, 1 = ASCII decimal + CRLF).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FbParams {
    #[serde(default)]
    pub in1: String,
    #[serde(default)]
    pub in2: String,
    #[serde(default)]
    pub k: [i32; 4],
    #[serde(default)]
    pub mode: u8,
}

/// Parse an integer literal used as a function-block operand.
///
/// Accepts decimal (`"100"`, `"-5"`, `"+7"`), IEC based literals (`"16#FF"`, `"2#1010"`, `"8#17"`),
/// C-style hex (`"0x1F"`), FX-style `K` constants (`"K100"`) and `_` digit separators.
/// Returns `None` for register names / symbols (e.g. `"D5"`, `"%MW3"`).
pub fn parse_int_literal(s: &str) -> Option<i64> {
    let t = s.trim().trim_end_matches(';').replace('_', "");
    if t.is_empty() {
        return None;
    }
    let (neg, body) = match t.as_bytes()[0] {
        b'-' => (true, &t[1..]),
        b'+' => (false, &t[1..]),
        _ => (false, &t[..]),
    };
    let upper = body.to_ascii_uppercase();
    let val = if let Some(h) = upper.strip_prefix("16#").or_else(|| upper.strip_prefix("0X")) {
        i64::from_str_radix(h, 16).ok()?
    } else if let Some(b) = upper.strip_prefix("2#").or_else(|| upper.strip_prefix("0B")) {
        i64::from_str_radix(b, 2).ok()?
    } else if let Some(o) = upper.strip_prefix("8#") {
        i64::from_str_radix(o, 8).ok()?
    } else if let Some(d) = upper.strip_prefix("10#") {
        d.parse::<i64>().ok()?
    } else if let Some(d) = upper.strip_prefix('K') {
        if d.is_empty() || !d.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        d.parse::<i64>().ok()?
    } else {
        if !upper.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        upper.parse::<i64>().ok()?
    };
    Some(if neg { -val } else { val })
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LadderElement {
    pub id: u64,
    pub element_type: TypeTag,
    pub name: String,             // e.g. "X0", "Y0", "T0", "C0", "R0"
    pub symbol: String,           // User-friendly tag e.g. "START_PB", "MOTOR"
    pub comment: String,
    
    // Contact / Coil properties
    pub contact_mode: ContactMode,
    #[serde(default)]
    pub contact_style: Option<ContactStyle>,
    pub coil_mode: CoilMode,
    #[serde(default)]
    pub coil_style: Option<CoilStyle>,
    pub auto_clear: bool,
    
    // Timer / Counter / Math properties
    pub preset_value: i32,        // e.g. 5000 ms for TON, or 10 counts for CTU
    pub preset_value1: i32,       // e.g. low bound or secondary parameter
    pub compare_op: String,       // "==", "!=", ">", "<", ">=", "<="
    pub math_op: String,          // "+", "-", "*", "/", "%"
    pub target_reg: String,       // e.g. "D0"

    // Modbus Master Function Block parameters
    #[serde(default = "default_mb_slave_id")]
    pub mb_slave_id: u8,
    #[serde(default = "default_mb_function")]
    pub mb_function: u8,

    /// Generic function-block parameters (PID / SCALE / LIMIT / Bitwise / Shift / UART).
    #[serde(default)]
    pub fb: FbParams,
    
    // Parallel branch sub-paths (each path is a sequence of elements in series)
    pub branch_paths: Vec<Vec<LadderElement>>,
    
    // Runtime simulation states
    #[serde(default)]
    pub is_energized: bool,
    #[serde(default)]
    pub current_accum: i32,
    #[serde(default)]
    pub is_done: bool,
}

fn default_mb_slave_id() -> u8 {
    1
}

fn default_mb_function() -> u8 {
    3
}

impl Default for LadderElement {
    fn default() -> Self {
        Self {
            id: 0,
            element_type: TypeTag::Contact,
            name: "X00".to_string(),
            symbol: "X00".to_string(),
            comment: String::new(),
            contact_mode: ContactMode::NormallyOpen,
            contact_style: None,
            coil_mode: CoilMode::Normal,
            coil_style: None,
            auto_clear: false,
            preset_value: 1000,
            preset_value1: 0,
            compare_op: "==".to_string(),
            math_op: "+".to_string(),
            target_reg: "V00".to_string(),
            mb_slave_id: 1,
            mb_function: 3,
            fb: FbParams::default(),
            branch_paths: Vec::new(),
            is_energized: false,
            current_accum: 0,
            is_done: false,
        }
    }
}

impl LadderElement {
    pub fn new_contact(id: u64, name: impl Into<String>, symbol: impl Into<String>, mode: ContactMode) -> Self {
        Self {
            id,
            element_type: TypeTag::Contact,
            name: name.into(),
            symbol: symbol.into(),
            contact_mode: mode,
            ..Default::default()
        }
    }

    pub fn new_coil(id: u64, name: impl Into<String>, symbol: impl Into<String>, mode: CoilMode) -> Self {
        Self {
            id,
            element_type: TypeTag::Coil,
            name: name.into(),
            symbol: symbol.into(),
            coil_mode: mode,
            ..Default::default()
        }
    }

    pub fn new_ton(id: u64, name: impl Into<String>, symbol: impl Into<String>, preset_ms: i32) -> Self {
        Self {
            id,
            element_type: TypeTag::Ton,
            name: name.into(),
            symbol: symbol.into(),
            preset_value: preset_ms,
            ..Default::default()
        }
    }

    pub fn new_ctu(id: u64, name: impl Into<String>, symbol: impl Into<String>, preset_count: i32) -> Self {
        Self {
            id,
            element_type: TypeTag::Ctu,
            name: name.into(),
            symbol: symbol.into(),
            preset_value: preset_count,
            ..Default::default()
        }
    }

    pub fn new_wire(id: u64) -> Self {
        Self {
            id,
            element_type: TypeTag::Wire,
            name: "SHORT".to_string(),
            symbol: String::new(),
            ..Default::default()
        }
    }

    pub fn new_open(id: u64) -> Self {
        Self {
            id,
            element_type: TypeTag::OpenCircuit,
            name: "OPEN".to_string(),
            symbol: String::new(),
            ..Default::default()
        }
    }

    pub fn new_branch(id: u64, paths: Vec<Vec<LadderElement>>) -> Self {
        Self {
            id,
            element_type: TypeTag::Branch,
            name: "PARALLEL".to_string(),
            branch_paths: paths,
            ..Default::default()
        }
    }

    pub fn new_comment(id: u64, text: impl Into<String>) -> Self {
        Self {
            id,
            element_type: TypeTag::Comment,
            name: "COMMENT".to_string(),
            comment: text.into(),
            ..Default::default()
        }
    }

    /// Destination register of a `Math` / `Move` element.
    ///
    /// The element dialog edits `name` (the address field) while the IL parser fills both `name`
    /// and `target_reg`, so `name` is authoritative and `target_reg` is the fallback. The Simulator,
    /// the Bytecode compiler and the IL exporter all use this helper so they always agree.
    pub fn word_target(&self) -> &str {
        if !self.name.trim().is_empty() {
            self.name.trim()
        } else {
            self.target_reg.trim()
        }
    }

    /// Short human-readable parameter summary for the feature-level-2 function blocks
    /// (used by the Elements list). Returns `None` for other element types.
    pub fn fb_value_summary(&self) -> Option<String> {
        let fb = &self.fb;
        let s = match self.element_type {
            TypeTag::Pid => format!(
                "PV={} SP={} CV={} Kp={:.2} Ki={:.2} Kd={:.2} Max={}",
                fb.in1, fb.in2, self.target_reg,
                fb.k[0] as f64 / 100.0, fb.k[1] as f64 / 100.0, fb.k[2] as f64 / 100.0, fb.k[3]
            ),
            TypeTag::Scale => format!(
                "{}[{}..{}] → {}[{}..{}]",
                fb.in1, fb.k[0], fb.k[1], self.target_reg, fb.k[2], fb.k[3]
            ),
            TypeTag::Limit => format!("{} → {} [{}..{}]", fb.in1, self.target_reg, fb.k[0], fb.k[1]),
            TypeTag::Bitwise => {
                if self.math_op.eq_ignore_ascii_case("NOT") {
                    format!("{} := NOT {}", self.target_reg, fb.in1)
                } else {
                    format!("{} := {} {} {}", self.target_reg, fb.in1, self.math_op.to_ascii_uppercase(), fb.in2)
                }
            }
            TypeTag::Adc => format!("{} → {}", self.name, self.target_reg),
            TypeTag::ShiftRegisters => format!("len={} in={}", self.preset_value, fb.in1),
            TypeTag::ShiftRegInbit => format!("bit={}", fb.in1),
            TypeTag::ShiftLeft => format!("<< {}", self.preset_value),
            TypeTag::ShiftRight => format!(">> {}", self.preset_value),
            TypeTag::ShiftVariable => format!("shift by {}", fb.in1),
            TypeTag::ShiftRegReset => "reset".to_string(),
            TypeTag::UartSend => format!("{} ({})", fb.in1, if fb.mode == 1 { "ASCII" } else { "byte" }),
            TypeTag::UartRecv => format!("→ {}", self.target_reg),
            _ => return None,
        };
        Some(s)
    }

    /// Give this element (and all nested branch elements) fresh ids taken from `next_id`.
    /// Uses the same convention as `LadderProgram::alloc_id`: assign, then increment, so
    /// `next_id` always points at an unused id afterwards.
    pub fn reassign_ids(&mut self, next_id: &mut u64) {
        self.id = *next_id;
        *next_id += 1;
        for path in &mut self.branch_paths {
            for sub in path {
                sub.reassign_ids(next_id);
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rung {
    pub id: usize,
    pub comment: String,
    pub elements: Vec<LadderElement>,
    #[serde(default)]
    pub is_energized: bool,
}

impl Rung {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            comment: String::new(),
            elements: Vec::new(),
            is_energized: false,
        }
    }

    pub fn with_elements(id: usize, elements: Vec<LadderElement>) -> Self {
        let mut rung = Self {
            id,
            comment: String::new(),
            elements,
            is_energized: false,
        };
        rung.ensure_label_at_start();
        rung
    }

    /// Hybrid Left-Rail Anchor:
    /// Ensures that if a TypeTag::Label element exists in this rung,
    /// it is always placed at index 0 (attached to the Left Power Rail).
    pub fn ensure_label_at_start(&mut self) {
        if let Some(pos) = self.elements.iter().position(|e| e.element_type == TypeTag::Label) {
            if pos > 0 {
                let lbl = self.elements.remove(pos);
                self.elements.insert(0, lbl);
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IOPortMapping {
    pub plc_address: String,   // e.g. "X0", "X1", "Y0", "Y1" or "%IX0.0", "%QX0.0"
    #[serde(default)]
    pub iec_address: String,   // e.g. "%IX0.0", "%QX0.0" (Editable IEC address)
    pub symbol: String,        // e.g. "START_PB", "STOP_PB", "MOTOR"
    pub hw_pin: String,        // e.g. "D2", "D3", "D13", "A0"
    pub description: String,
    pub is_input: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IecVarEntry {
    pub name: String,
    pub address: String,
    pub data_type: String,
    pub hw_pin: String,
    pub symbol: String,
    pub description: String,
    pub is_input: bool,
}

impl IecVarEntry {
    /// Format as a single IEC 61131-3 variable declaration line.
    pub fn to_declaration_line(&self) -> String {
        format!(
            "    {} AT {} : {}; (* Pin: {} | Symbol: {} | {} *)",
            self.name, self.address, self.data_type, self.hw_pin, self.symbol, self.description
        )
    }
}

impl IOPortMapping {
    /// Return the standard IEC 61131-3 address (e.g. "%IX0.0", "%QX0.0").
    pub fn iec_address(&self, fallback_index: usize) -> String {
        if !self.iec_address.trim().is_empty() {
            return self.iec_address.trim().to_string();
        }

        let trimmed = self.plc_address.trim().to_uppercase();
        if trimmed.starts_with('%') {
            return trimmed;
        }

        let num_part = trimmed.trim_start_matches(|c: char| c.is_alphabetic());
        let idx = num_part.parse::<usize>().unwrap_or(fallback_index);
        let byte_idx = idx / 8;
        let bit_idx = idx % 8;

        if self.is_input {
            format!("%IX{}.{}", byte_idx, bit_idx)
        } else {
            format!("%QX{}.{}", byte_idx, bit_idx)
        }
    }

    /// Map this I/O port into an IEC 61131-3 variable entry.
    pub fn to_iec_entry(&self, fallback_index: usize, use_symbol_as_identifier: bool) -> IecVarEntry {
        let iec_addr = self.iec_address(fallback_index);
        let name = if use_symbol_as_identifier && !self.symbol.trim().is_empty() {
            let clean: String = self
                .symbol
                .trim()
                .chars()
                .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
                .collect();
            if clean.chars().next().map_or(false, |c| c.is_alphabetic() || c == '_') {
                clean
            } else {
                format!("TAG_{}", clean)
            }
        } else {
            self.plc_address.clone()
        };

        IecVarEntry {
            name,
            address: iec_addr,
            data_type: "BOOL".to_string(),
            hw_pin: self.hw_pin.clone(),
            symbol: self.symbol.clone(),
            description: self.description.clone(),
            is_input: self.is_input,
        }
    }
}

/// A record representing a symbol in the Global Symbol Table (Single Source of Truth).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SymbolRecord {
    pub address: String,       // canonical address or IEC / PLC address (e.g. "%IX0.0", "%QX0.0", "M0", "T0", etc.)
    pub symbol: String,        // user-friendly tag name e.g. "START_PB"
    #[serde(default)]
    pub comment: String,
    #[serde(default = "default_symbol_datatype")]
    pub data_type: String,
}

fn default_symbol_datatype() -> String {
    "BOOL".to_string()
}

/// Global Symbol Table serving as the Single Source of Truth for element symbols and tag names across the PLC project.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GlobalSymbolTable {
    #[serde(default)]
    pub records: Vec<SymbolRecord>,
}

impl GlobalSymbolTable {
    pub fn new() -> Self {
        Self { records: Vec::new() }
    }

    /// Retrieve symbol name by address (case-insensitive, checking raw and canonical address).
    pub fn get_symbol(&self, address: &str) -> Option<&str> {
        let clean = address.trim();
        if clean.is_empty() {
            return None;
        }
        let canon = LadderProgram::canonical_address(clean);
        for r in &self.records {
            if r.address.eq_ignore_ascii_case(clean) || r.address.eq_ignore_ascii_case(&canon) {
                if !r.symbol.is_empty() {
                    return Some(&r.symbol);
                }
            }
        }
        None
    }

    /// Retrieve full symbol record by address.
    pub fn get_record(&self, address: &str) -> Option<&SymbolRecord> {
        let clean = address.trim();
        if clean.is_empty() {
            return None;
        }
        let canon = LadderProgram::canonical_address(clean);
        self.records.iter().find(|r| {
            r.address.eq_ignore_ascii_case(clean) || r.address.eq_ignore_ascii_case(&canon)
        })
    }

    /// Check if a symbol name is already in use by another address.
    pub fn is_symbol_taken_by_other(&self, symbol: &str, current_address: &str) -> bool {
        let sym_trimmed = symbol.trim();
        if sym_trimmed.is_empty() {
            return false;
        }
        let clean_addr = current_address.trim();
        let canon_addr = LadderProgram::canonical_address(clean_addr);
        self.records.iter().any(|r| {
            r.symbol.eq_ignore_ascii_case(sym_trimmed)
                && !r.address.eq_ignore_ascii_case(clean_addr)
                && !r.address.eq_ignore_ascii_case(&canon_addr)
        })
    }

    /// Set or update the symbol for an address.
    pub fn set_symbol(&mut self, address: &str, symbol: &str, comment: Option<&str>) {
        let clean_addr = address.trim();
        if clean_addr.is_empty() {
            return;
        }
        let canon = LadderProgram::canonical_address(clean_addr);
        let sym_clean = symbol.trim().to_string();

        if let Some(existing) = self.records.iter_mut().find(|r| {
            r.address.eq_ignore_ascii_case(clean_addr) || r.address.eq_ignore_ascii_case(&canon)
        }) {
            existing.symbol = sym_clean;
            if let Some(c) = comment {
                existing.comment = c.to_string();
            }
        } else {
            self.records.push(SymbolRecord {
                address: canon,
                symbol: sym_clean,
                comment: comment.unwrap_or("").to_string(),
                data_type: default_symbol_datatype(),
            });
        }
    }

    /// Remove a symbol record by address.
    pub fn remove_symbol(&mut self, address: &str) -> bool {
        let clean = address.trim();
        let canon = LadderProgram::canonical_address(clean);
        let len_before = self.records.len();
        self.records.retain(|r| {
            !r.address.eq_ignore_ascii_case(clean) && !r.address.eq_ignore_ascii_case(&canon)
        });
        self.records.len() < len_before
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LadderProgram {
    pub title: String,
    pub hardware: String,      // e.g. "Arduino UNO R3"
    pub rungs: Vec<Rung>,
    #[serde(default)]
    pub symbols: GlobalSymbolTable,
    pub io_mappings: Vec<IOPortMapping>,
    pub next_element_id: u64,
}

impl Default for LadderProgram {
    fn default() -> Self {
        Self::new_demo_program()
    }
}

impl LadderProgram {
    pub fn new_empty() -> Self {
        Self {
            title: "Ladder Program By LaduinoPLC [PksofttecH]".to_string(),
            hardware: "Arduino UNO R3".to_string(),
            rungs: vec![Rung::new(0)],
            symbols: GlobalSymbolTable::default(),
            io_mappings: Vec::new(),
            next_element_id: 1,
        }
    }

    /// Map all program IO mappings into structured IEC 61131-3 variable entries.
    pub fn map_io_to_iec_var_entries(&self, use_symbol_as_identifier: bool) -> Vec<IecVarEntry> {
        let mut in_count = 0;
        let mut out_count = 0;
        let mut entries = Vec::with_capacity(self.io_mappings.len());

        for io in &self.io_mappings {
            let idx = if io.is_input {
                let cur = in_count;
                in_count += 1;
                cur
            } else {
                let cur = out_count;
                out_count += 1;
                cur
            };
            entries.push(io.to_iec_entry(idx, use_symbol_as_identifier));
        }

        entries
    }

    /// Generate standard IEC 61131-3 VAR ... END_VAR declaration table string.
    pub fn map_io_to_iec_var_table(&self) -> String {
        crate::il_format::IlParser::map_io_to_iec_var_table(self)
    }

    /// Generate IEC 61131-3 VAR ... END_VAR table with configurable identifier style.
    pub fn map_io_to_iec_var_table_with_options(&self, use_symbol_name: bool) -> String {
        crate::il_format::IlParser::map_io_to_iec_var_table_with_options(self, use_symbol_name)
    }

    pub fn alloc_id(&mut self) -> u64 {
        let id = self.next_element_id;
        self.next_element_id += 1;
        id
    }

    /// Normalize all rungs so any TypeTag::Label elements are anchored at index 0.
    pub fn ensure_labels_at_start(&mut self) {
        for rung in &mut self.rungs {
            rung.ensure_label_at_start();
        }
    }

    pub fn default_arduino_io() -> Vec<IOPortMapping> {
        vec![
            IOPortMapping {
                plc_address: "X0".to_string(),
                iec_address: "%IX0.0".to_string(),
                symbol: "START_PB".to_string(),
                hw_pin: "D2".to_string(),
                description: "Start Pushbutton (NO)".to_string(),
                is_input: true,
            },
            IOPortMapping {
                plc_address: "X1".to_string(),
                iec_address: "%IX0.1".to_string(),
                symbol: "STOP_PB".to_string(),
                hw_pin: "D3".to_string(),
                description: "Stop Pushbutton (NC)".to_string(),
                is_input: true,
            },
            IOPortMapping {
                plc_address: "X2".to_string(),
                iec_address: "%IX0.2".to_string(),
                symbol: "SENS_LIMIT".to_string(),
                hw_pin: "D4".to_string(),
                description: "Proximity Limit Switch".to_string(),
                is_input: true,
            },
            IOPortMapping {
                plc_address: "X3".to_string(),
                iec_address: "%IX0.3".to_string(),
                symbol: "EMERGENCY".to_string(),
                hw_pin: "D5".to_string(),
                description: "Emergency Stop".to_string(),
                is_input: true,
            },
            IOPortMapping {
                plc_address: "Y0".to_string(),
                iec_address: "%QX0.0".to_string(),
                symbol: "MOTOR_RUN".to_string(),
                hw_pin: "D8".to_string(),
                description: "Main Motor Contactor".to_string(),
                is_input: false,
            },
            IOPortMapping {
                plc_address: "Y1".to_string(),
                iec_address: "%QX0.1".to_string(),
                symbol: "LAMP_RUN".to_string(),
                hw_pin: "D9".to_string(),
                description: "Running Indicator Lamp (Green)".to_string(),
                is_input: false,
            },
            IOPortMapping {
                plc_address: "Y2".to_string(),
                iec_address: "%QX0.2".to_string(),
                symbol: "ALARM_BUZZER".to_string(),
                hw_pin: "D10".to_string(),
                description: "Warning Alarm Buzzer".to_string(),
                is_input: false,
            },
            IOPortMapping {
                plc_address: "Y3".to_string(),
                iec_address: "%QX0.3".to_string(),
                symbol: "LAMP_FAULT".to_string(),
                hw_pin: "D13".to_string(),
                description: "Fault Indicator Lamp (Red)".to_string(),
                is_input: false,
            },
        ]
    }

    pub fn new_demo_program() -> Self {
        let mut id_counter = 100u64;
        let mut next_id = || {
            id_counter += 1;
            id_counter
        };

        // Rung 0: [Parallel( [%IX0.0:START_PB], [%QX0.0:MOTOR_RUN] )] -> [%IX0.1:STOP_PB (NC)] -> (%QX0.0:MOTOR_RUN)
        let branch_start_seal = LadderElement::new_branch(
            next_id(),
            vec![
                vec![LadderElement::new_contact(next_id(), "%IX0.0", "START_PB", ContactMode::NormallyOpen)],
                vec![LadderElement::new_contact(next_id(), "%QX0.0", "MOTOR_RUN", ContactMode::NormallyOpen)],
            ],
        );
        let contact_stop = LadderElement::new_contact(next_id(), "%IX0.1", "STOP_PB", ContactMode::NormallyClosed);
        let coil_motor = LadderElement::new_coil(next_id(), "%QX0.0", "MOTOR_RUN", CoilMode::Normal);

        let rung0 = Rung {
            id: 0,
            comment: "Main Motor Start/Stop with Self-Holding Latch".to_string(),
            elements: vec![branch_start_seal, contact_stop, coil_motor],
            is_energized: false,
        };

        // Rung 1: [%QX0.0:MOTOR_RUN] -> (%QX0.1:LAMP_RUN)
        let contact_motor_aux = LadderElement::new_contact(next_id(), "%QX0.0", "MOTOR_RUN", ContactMode::NormallyOpen);
        let coil_lamp = LadderElement::new_coil(next_id(), "%QX0.1", "LAMP_RUN", CoilMode::Normal);

        let rung1 = Rung {
            id: 1,
            comment: "Running Indicator Lamp follows Motor Status".to_string(),
            elements: vec![contact_motor_aux, coil_lamp],
            is_energized: false,
        };

        // Rung 2: [%QX0.0:MOTOR_RUN] -> [TON T0 3000ms] -> (%QX0.2:ALARM_BUZZER)
        let contact_m_timer = LadderElement::new_contact(next_id(), "%QX0.0", "MOTOR_RUN", ContactMode::NormallyOpen);
        let ton_t0 = LadderElement::new_ton(next_id(), "T0", "DELAY_ALARM", 3000);
        let contact_t0_done = LadderElement::new_contact(next_id(), "T0", "T0_DONE", ContactMode::NormallyOpen);
        let coil_buzzer = LadderElement::new_coil(next_id(), "%QX0.2", "ALARM_BUZZER", CoilMode::Normal);

        let rung2 = Rung {
            id: 2,
            comment: "Timer On Delay (3s) activates Warning Alarm".to_string(),
            elements: vec![contact_m_timer, ton_t0, contact_t0_done, coil_buzzer],
            is_energized: false,
        };

        let mut prog = Self {
            title: "Ladder Program By LaduinoPLC [PksofttecH]".to_string(),
            hardware: "Arduino UNO R3".to_string(),
            rungs: vec![rung0, rung1, rung2],
            symbols: GlobalSymbolTable::default(),
            io_mappings: Self::default_arduino_io(),
            next_element_id: id_counter + 1,
        };
        prog.sync_symbols_from_elements_and_io();
        prog
    }

    pub fn add_rung(&mut self) -> usize {
        let id = self.rungs.len();
        self.rungs.push(Rung::new(id));
        id
    }

    pub fn insert_rung(&mut self, index: usize) -> usize {
        let new_id = index.min(self.rungs.len());
        self.rungs.insert(new_id, Rung::new(new_id));
        self.reindex_rungs();
        new_id
    }

    pub fn copy_rung_at(&mut self, src_idx: usize, insert_idx: usize) -> usize {
        if let Some(rung) = self.rungs.get(src_idx).cloned() {
            let mut cloned = rung;
            for el in &mut cloned.elements {
                el.reassign_ids(&mut self.next_element_id);
            }
            let target = insert_idx.min(self.rungs.len());
            self.rungs.insert(target, cloned);
            self.reindex_rungs();
            target
        } else {
            src_idx
        }
    }

    pub fn delete_rung(&mut self, index: usize) {
        if index < self.rungs.len() && self.rungs.len() > 1 {
            self.rungs.remove(index);
            self.reindex_rungs();
        } else if self.rungs.len() == 1 {
            self.rungs[0].elements.clear();
            self.rungs[0].comment.clear();
        }
    }

    pub fn move_rung_up(&mut self, index: usize) -> usize {
        if index > 0 && index < self.rungs.len() {
            self.rungs.swap(index, index - 1);
            self.reindex_rungs();
            index - 1
        } else {
            index
        }
    }

    pub fn move_rung_down(&mut self, index: usize) -> usize {
        if index + 1 < self.rungs.len() {
            self.rungs.swap(index, index + 1);
            self.reindex_rungs();
            index + 1
        } else {
            index
        }
    }

    pub fn reindex_rungs(&mut self) {
        for (idx, rung) in self.rungs.iter_mut().enumerate() {
            rung.id = idx;
        }
    }

    /// Return canonical IEC / standard address string (e.g. "X0" -> "%IX0.0", "Y1" -> "%QX0.1", "M0" -> "%MX0.0", "V0" -> "%MW0")
    pub fn canonical_address(addr: &str) -> String {
        let trimmed = addr.trim();
        let upper = trimmed.to_ascii_uppercase();
        if upper.starts_with('%') {
            upper
        } else if let Some(rest) = upper.strip_prefix('X').or_else(|| upper.strip_prefix('I')) {
            if let Ok(n) = rest.parse::<usize>() {
                format!("%IX{}.{}", n / 8, n % 8)
            } else {
                upper
            }
        } else if let Some(rest) = upper.strip_prefix('Y').or_else(|| upper.strip_prefix('Q')) {
            if let Ok(n) = rest.parse::<usize>() {
                format!("%QX{}.{}", n / 8, n % 8)
            } else {
                upper
            }
        } else if let Some(rest) = upper.strip_prefix('M') {
            if let Ok(n) = rest.parse::<usize>() {
                format!("%MX{}.{}", n / 8, n % 8)
            } else {
                upper
            }
        } else if let Some(rest) = upper.strip_prefix('V').or_else(|| upper.strip_prefix('R')).or_else(|| upper.strip_prefix('D')) {
            if let Ok(n) = rest.parse::<usize>() {
                format!("%MW{}", n)
            } else {
                upper
            }
        } else {
            upper
        }
    }

    /// Resolve symbol for an address: checks GlobalSymbolTable first, then io_mappings.
    pub fn resolve_symbol(&self, addr: &str) -> Option<&str> {
        let clean = addr.trim();
        if clean.is_empty() {
            return None;
        }
        if let Some(sym) = self.symbols.get_symbol(clean) {
            if !sym.is_empty() {
                return Some(sym);
            }
        }
        let canon = Self::canonical_address(clean);
        for io in &self.io_mappings {
            if io.plc_address.eq_ignore_ascii_case(clean)
                || io.iec_address.eq_ignore_ascii_case(clean)
                || io.plc_address.eq_ignore_ascii_case(&canon)
                || io.iec_address.eq_ignore_ascii_case(&canon)
            {
                if !io.symbol.is_empty() {
                    return Some(&io.symbol);
                }
            }
        }
        None
    }

    /// Harvest and synchronize symbols from `io_mappings` and all ladder elements
    /// into the GlobalSymbolTable (`self.symbols`).
    pub fn sync_symbols_from_elements_and_io(&mut self) {
        for io in &self.io_mappings {
            if !io.symbol.is_empty() {
                let addr = if !io.iec_address.is_empty() {
                    &io.iec_address
                } else {
                    &io.plc_address
                };
                if self.symbols.get_symbol(addr).is_none() {
                    self.symbols.set_symbol(addr, &io.symbol, Some(&io.description));
                }
            }
        }

        fn harvest_from_elem(el: &LadderElement, syms: &mut GlobalSymbolTable) {
            if el.element_type != TypeTag::Branch {
                if !el.symbol.is_empty() && !el.symbol.eq_ignore_ascii_case(&el.name) {
                    if syms.get_symbol(&el.name).is_none() {
                        syms.set_symbol(&el.name, &el.symbol, Some(&el.comment));
                    }
                }
            }
            for path in &el.branch_paths {
                for sub in path {
                    harvest_from_elem(sub, syms);
                }
            }
        }

        for rung in &self.rungs {
            for el in &rung.elements {
                harvest_from_elem(el, &mut self.symbols);
            }
        }
    }

    /// Resolve an element's address and symbol into standard IEC 61131-3 address and symbol.
    pub fn resolve_iec_and_symbol(&self, addr: &str, elem_symbol: &str) -> (String, String) {
        let trimmed = addr.trim();
        let canon = Self::canonical_address(trimmed);

        // 1. Check GlobalSymbolTable first (Single Source of Truth)
        if let Some(sym) = self.symbols.get_symbol(trimmed) {
            if !sym.is_empty() {
                return (canon, sym.to_string());
            }
        }

        // 2. Check I/O mappings
        let mut in_idx = 0;
        let mut out_idx = 0;
        for io in &self.io_mappings {
            let idx = if io.is_input {
                let c = in_idx;
                in_idx += 1;
                c
            } else {
                let c = out_idx;
                out_idx += 1;
                c
            };
            let io_iec = io.iec_address(idx);
            if io.plc_address.eq_ignore_ascii_case(trimmed)
                || io_iec.eq_ignore_ascii_case(trimmed)
                || (!io.symbol.is_empty() && io.symbol.eq_ignore_ascii_case(trimmed))
            {
                let sym = if !elem_symbol.trim().is_empty()
                    && !elem_symbol.eq_ignore_ascii_case(trimmed)
                    && !elem_symbol.eq_ignore_ascii_case(&io.plc_address)
                {
                    elem_symbol.trim().to_string()
                } else {
                    io.symbol.clone()
                };
                return (io_iec, sym);
            }
        }

        let sym = if !elem_symbol.trim().is_empty() {
            elem_symbol.trim().to_string()
        } else {
            canon.clone()
        };
        (canon, sym)
    }

    /// Collect all unique leaf elements in the program with resolved (raw_name, iec_address, symbol, el_type, val_str)
    pub fn collect_all_iec_elements(&self) -> Vec<(String, String, String, TypeTag, String)> {
        let mut raw_list = Vec::new();
        for rung in &self.rungs {
            for el in &rung.elements {
                Self::collect_elem_with_symbol_recursive(el, &mut raw_list);
            }
        }
        raw_list
            .into_iter()
            .map(|(name, sym, el_type, val_str)| {
                let (iec, resolved_sym) = self.resolve_iec_and_symbol(&name, &sym);
                (name, iec, resolved_sym, el_type, val_str)
            })
            .collect()
    }

    /// Collect all unique leaf elements in the program for the right-side Elements list
    pub fn collect_all_elements(&self) -> Vec<(String, TypeTag, String)> {
        let mut list = Vec::new();
        for rung in &self.rungs {
            for el in &rung.elements {
                Self::collect_elem_recursive(el, &mut list);
            }
        }
        list
    }

    fn collect_elem_with_symbol_recursive(
        el: &LadderElement,
        list: &mut Vec<(String, String, TypeTag, String)>,
    ) {
        match el.element_type {
            TypeTag::Branch => {
                for path in &el.branch_paths {
                    for sub in path {
                        Self::collect_elem_with_symbol_recursive(sub, list);
                    }
                }
            }
            TypeTag::Comment | TypeTag::Wire | TypeTag::OpenCircuit => {}
            _ => {
                if !list.iter().any(|(name, _, t, _)| name == &el.name && *t == el.element_type) {
                    let val_str = match el.element_type {
                        TypeTag::Ton | TypeTag::Tof | TypeTag::Tp | TypeTag::Ctu | TypeTag::Ctd | TypeTag::Ctud => {
                            format!("{}", el.preset_value)
                        }
                        TypeTag::Compare => format!("{} {}", el.compare_op, el.preset_value),
                        TypeTag::Math => format!("{} {}", el.math_op, el.preset_value),
                        TypeTag::Move => format!(":= {}", el.preset_value),
                        _ => match el.fb_value_summary() {
                            Some(s) => s,
                            None => if el.is_energized { "1".to_string() } else { "0".to_string() },
                        },
                    };
                    list.push((el.name.clone(), el.symbol.clone(), el.element_type, val_str));
                }
            }
        }
    }

    fn collect_elem_recursive(el: &LadderElement, list: &mut Vec<(String, TypeTag, String)>) {
        match el.element_type {
            TypeTag::Branch => {
                for path in &el.branch_paths {
                    for sub in path {
                        Self::collect_elem_recursive(sub, list);
                    }
                }
            }
            TypeTag::Comment | TypeTag::Wire | TypeTag::OpenCircuit => {}
            _ => {
                if !list.iter().any(|(name, t, _)| name == &el.name && *t == el.element_type) {
                    let val_str = match el.element_type {
                        TypeTag::Ton | TypeTag::Tof | TypeTag::Tp | TypeTag::Ctu | TypeTag::Ctd | TypeTag::Ctud => {
                            format!("{}", el.preset_value)
                        }
                        TypeTag::Compare => format!("{} {}", el.compare_op, el.preset_value),
                        TypeTag::Math => format!("{} {}", el.math_op, el.preset_value),
                        TypeTag::Move => format!(":= {}", el.preset_value),
                        _ => match el.fb_value_summary() {
                            Some(s) => s,
                            None => if el.is_energized { "1".to_string() } else { "0".to_string() },
                        },
                    };
                    list.push((el.name.clone(), el.element_type, val_str));
                }
            }
        }
    }
}
