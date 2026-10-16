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
    UartSend,
    UartRecv,
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
                | TypeTag::Adc
                | TypeTag::ShiftRegisters
                | TypeTag::ShiftVariable
                | TypeTag::ShiftLeft
                | TypeTag::ShiftRight
                | TypeTag::ShiftRegReset
                | TypeTag::ShiftRegInbit
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
    /// Industrial Field / Automation: Switch Toggle Switch / Coil Solenoid Valve —[ ⧖ ]—
    ToggleSolenoid,
}

impl SchematicStyle {
    pub const ALL: [Self; 5] = [
        Self::IecStandard,
        Self::JicNema,
        Self::DinEuropean,
        Self::PushBtnPilot,
        Self::ToggleSolenoid,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::IecStandard => "IEC 61131-3 Standard (—[ ]— / —( )—)",
            Self::JicNema => "JIC / NEMA American (—○/○— / —[ M ]—)",
            Self::DinEuropean => "European DIN / JIS (—●/ — / —○—)",
            Self::PushBtnPilot => "Push Button & Pilot Lamp",
            Self::ToggleSolenoid => "Toggle Switch & Solenoid Valve",
        }
    }

    pub fn short_name(&self) -> &'static str {
        match self {
            Self::IecStandard => "IEC —( )—",
            Self::JicNema => "JIC —[ ]—",
            Self::DinEuropean => "DIN —○—",
            Self::PushBtnPilot => "Panel —(⨂)—",
            Self::ToggleSolenoid => "Field —[⧖]—",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::IecStandard => "Standard PLC ladder symbols (Contacts: —[ ]—, Coils: —( )—)",
            Self::JicNema => "American JIC / NEMA (Contacts: —○/○—, Coils: —[ M ]—)",
            Self::DinEuropean => "European DIN EN 60617 & Japanese JIS (Contacts: —●/ —, Coils: —○—)",
            Self::PushBtnPilot => "Control panel representation (Push buttons & illuminated lamps)",
            Self::ToggleSolenoid => "Industrial automation (Toggle switches & solenoid valves)",
        }
    }

    pub fn contact_style(&self) -> ContactStyle {
        match self {
            Self::IecStandard => ContactStyle::IecStandard,
            Self::JicNema => ContactStyle::JicSchematic,
            Self::DinEuropean => ContactStyle::DinEuropean,
            Self::PushBtnPilot => ContactStyle::PushButton,
            Self::ToggleSolenoid => ContactStyle::ToggleSwitch,
        }
    }

    pub fn coil_style(&self) -> CoilStyle {
        match self {
            Self::IecStandard => CoilStyle::IecParentheses,
            Self::JicNema => CoilStyle::NemaBox,
            Self::DinEuropean => CoilStyle::CircleRelay,
            Self::PushBtnPilot => CoilStyle::PilotLight,
            Self::ToggleSolenoid => CoilStyle::SolenoidValve,
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

    pub fn reassign_ids(&mut self, next_id: &mut u64) {
        *next_id += 1;
        self.id = *next_id;
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
        Self {
            id,
            comment: String::new(),
            elements,
            is_energized: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IOPortMapping {
    pub plc_address: String,   // e.g. "X0", "X1", "Y0", "Y1"
    pub symbol: String,        // e.g. "START_PB", "STOP_PB", "MOTOR"
    pub hw_pin: String,        // e.g. "D2", "D3", "D13", "A0"
    pub description: String,
    pub is_input: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LadderProgram {
    pub title: String,
    pub hardware: String,      // e.g. "Arduino UNO R3"
    pub rungs: Vec<Rung>,
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
            title: "Ladder Program By uPLC Studio [PksofttecH]".to_string(),
            hardware: "Arduino UNO R3".to_string(),
            rungs: vec![Rung::new(0)],
            io_mappings: Self::default_arduino_io(),
            next_element_id: 1,
        }
    }

    pub fn alloc_id(&mut self) -> u64 {
        let id = self.next_element_id;
        self.next_element_id += 1;
        id
    }

    pub fn default_arduino_io() -> Vec<IOPortMapping> {
        vec![
            IOPortMapping {
                plc_address: "X0".to_string(),
                symbol: "START_PB".to_string(),
                hw_pin: "D2".to_string(),
                description: "Start Pushbutton (NO)".to_string(),
                is_input: true,
            },
            IOPortMapping {
                plc_address: "X1".to_string(),
                symbol: "STOP_PB".to_string(),
                hw_pin: "D3".to_string(),
                description: "Stop Pushbutton (NC)".to_string(),
                is_input: true,
            },
            IOPortMapping {
                plc_address: "X2".to_string(),
                symbol: "SENS_LIMIT".to_string(),
                hw_pin: "D4".to_string(),
                description: "Proximity Limit Switch".to_string(),
                is_input: true,
            },
            IOPortMapping {
                plc_address: "X3".to_string(),
                symbol: "EMERGENCY".to_string(),
                hw_pin: "D5".to_string(),
                description: "Emergency Stop".to_string(),
                is_input: true,
            },
            IOPortMapping {
                plc_address: "Y0".to_string(),
                symbol: "MOTOR_RUN".to_string(),
                hw_pin: "D8".to_string(),
                description: "Main Motor Contactor".to_string(),
                is_input: false,
            },
            IOPortMapping {
                plc_address: "Y1".to_string(),
                symbol: "LAMP_RUN".to_string(),
                hw_pin: "D9".to_string(),
                description: "Running Indicator Lamp (Green)".to_string(),
                is_input: false,
            },
            IOPortMapping {
                plc_address: "Y2".to_string(),
                symbol: "ALARM_BUZZER".to_string(),
                hw_pin: "D10".to_string(),
                description: "Warning Alarm Buzzer".to_string(),
                is_input: false,
            },
            IOPortMapping {
                plc_address: "Y3".to_string(),
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

        // Rung 0: [Parallel( [X0:START], [Y0:MOTOR] )] -> [X1:STOP (NC)] -> (Y0:MOTOR)
        let branch_start_seal = LadderElement::new_branch(
            next_id(),
            vec![
                vec![LadderElement::new_contact(next_id(), "X0", "START_PB", ContactMode::NormallyOpen)],
                vec![LadderElement::new_contact(next_id(), "Y0", "MOTOR_RUN", ContactMode::NormallyOpen)],
            ],
        );
        let contact_stop = LadderElement::new_contact(next_id(), "X1", "STOP_PB", ContactMode::NormallyClosed);
        let coil_motor = LadderElement::new_coil(next_id(), "Y0", "MOTOR_RUN", CoilMode::Normal);

        let rung0 = Rung {
            id: 0,
            comment: "Main Motor Start/Stop with Self-Holding Latch".to_string(),
            elements: vec![branch_start_seal, contact_stop, coil_motor],
            is_energized: false,
        };

        // Rung 1: [Y0:MOTOR] -> (Y1:LAMP_RUN)
        let contact_motor_aux = LadderElement::new_contact(next_id(), "Y0", "MOTOR_RUN", ContactMode::NormallyOpen);
        let coil_lamp = LadderElement::new_coil(next_id(), "Y1", "LAMP_RUN", CoilMode::Normal);

        let rung1 = Rung {
            id: 1,
            comment: "Running Indicator Lamp follows Motor Status".to_string(),
            elements: vec![contact_motor_aux, coil_lamp],
            is_energized: false,
        };

        // Rung 2: [Y0:MOTOR] -> [TON T0 3000ms] -> (Y2:ALARM_BUZZER)
        let contact_m_timer = LadderElement::new_contact(next_id(), "Y0", "MOTOR_RUN", ContactMode::NormallyOpen);
        let ton_t0 = LadderElement::new_ton(next_id(), "T0", "DELAY_ALARM", 3000);
        let contact_t0_done = LadderElement::new_contact(next_id(), "T0", "T0_DONE", ContactMode::NormallyOpen);
        let coil_buzzer = LadderElement::new_coil(next_id(), "Y2", "ALARM_BUZZER", CoilMode::Normal);

        let rung2 = Rung {
            id: 2,
            comment: "Timer On Delay (3s) activates Warning Alarm".to_string(),
            elements: vec![contact_m_timer, ton_t0, contact_t0_done, coil_buzzer],
            is_energized: false,
        };

        Self {
            title: "Ladder Program By uPLC Studio [PksofttecH]".to_string(),
            hardware: "Arduino UNO R3".to_string(),
            rungs: vec![rung0, rung1, rung2],
            io_mappings: Self::default_arduino_io(),
            next_element_id: id_counter + 1,
        }
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
                        _ => if el.is_energized { "1".to_string() } else { "0".to_string() },
                    };
                    list.push((el.name.clone(), el.element_type, val_str));
                }
            }
        }
    }
}
