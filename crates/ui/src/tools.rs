use my_canvas_core::{CoilMode, ContactMode, LadderElement, TypeTag};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveTool {
    Select,
    Pan,
    Comment,
    ContactNo,
    ContactNc,
    ContactP,
    ContactN,
    CoilNormal,
    CoilNegated,
    CoilSet,
    CoilReset,
    CoilP,
    CoilN,
    MasterRelay,
    Ton,
    Tof,
    Tp,
    Ctu,
    Ctd,
    Ctud,
    ShiftReg,
    ShiftLeft,
    ShiftRight,
    Move,
    Compare,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Adc,
    RTrig,
    FTrig,
    UartSend,
    UartRecv,
    Wire,
    OpenCircuit,
    Branch,
}

impl ActiveTool {
    #[allow(non_upper_case_globals)]
    pub const Tpc: Self = Self::Tp;
    #[allow(non_upper_case_globals)]
    pub const Ctc: Self = Self::Ctud;
    #[allow(non_upper_case_globals)]
    pub const Osr: Self = Self::RTrig;
    #[allow(non_upper_case_globals)]
    pub const Osf: Self = Self::FTrig;

    pub fn name(&self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Pan => "Pan Canvas",
            Self::Comment => "Comment",
            Self::ContactNo => "Contact (NO) —[ ]—",
            Self::ContactNc => "Contact (NC) —[/]—",
            Self::ContactP => "Contact (P) —[P]—",
            Self::ContactN => "Contact (N) —[N]—",
            Self::CoilNormal => "Coil —( )—",
            Self::CoilNegated => "Negated Coil —(/)—",
            Self::CoilSet => "Coil Set —(S)—",
            Self::CoilReset => "Coil Reset —(R)—",
            Self::CoilP => "Coil Pulse —(P)—",
            Self::CoilN => "Coil Pulse —(N)—",
            Self::MasterRelay => "Master Relay (MC)",
            Self::Ton => "TON (Delay On Timer)",
            Self::Tof => "TOF (Delay Off Timer)",
            Self::Tp => "TP (Pulse Timer)",
            Self::Ctu => "CTU (Count Up)",
            Self::Ctd => "CTD (Count Down)",
            Self::Ctud => "CTUD (Up/Down Counter)",
            Self::ShiftReg => "Shift Register",
            Self::ShiftLeft => "Shift Left (SHL)",
            Self::ShiftRight => "Shift Right (SHR)",
            Self::Move => "Move [:=]",
            Self::Compare => "Compare [EQ, NE, GT, LT, GE, LE]",
            Self::Add => "Add [+=]",
            Self::Sub => "Sub [-=]",
            Self::Mul => "Mul [*=]",
            Self::Div => "Div [/=]",
            Self::Mod => "Mod [%=]",
            Self::Adc => "A/D Converter Read (ADC)",
            Self::RTrig => "R_TRIG (Rising Edge Detector)",
            Self::FTrig => "F_TRIG (Falling Edge Detector)",
            Self::UartSend => "UART Send",
            Self::UartRecv => "UART Recv",
            Self::Wire => "Short Circuit Wire",
            Self::OpenCircuit => "Open Circuit",
            Self::Branch => "Parallel Branch",
        }
    }

    pub fn create_default_element(&self, id: u64) -> Option<LadderElement> {
        match self {
            Self::Comment => Some(LadderElement::new_comment(id, "COMMENT")),
            Self::ContactNo => Some(LadderElement::new_contact(id, "X00", "X00", ContactMode::NormallyOpen)),
            Self::ContactNc => Some(LadderElement::new_contact(id, "X01", "X01", ContactMode::NormallyClosed)),
            Self::ContactP => Some(LadderElement::new_contact(id, "X00", "X00", ContactMode::PositiveTransition)),
            Self::ContactN => Some(LadderElement::new_contact(id, "X00", "X00", ContactMode::NegativeTransition)),
            Self::CoilNormal => Some(LadderElement::new_coil(id, "Y00", "Y00", CoilMode::Normal)),
            Self::CoilNegated => Some(LadderElement::new_coil(id, "Y00", "Y00", CoilMode::Negated)),
            Self::CoilSet => Some(LadderElement::new_coil(id, "Y00", "Y00", CoilMode::SetLatch)),
            Self::CoilReset => Some(LadderElement::new_coil(id, "Y00", "Y00", CoilMode::ResetUnlatch)),
            Self::CoilP => Some(LadderElement::new_coil(id, "Y00", "Y00", CoilMode::PositiveTransition)),
            Self::CoilN => Some(LadderElement::new_coil(id, "Y00", "Y00", CoilMode::NegativeTransition)),
            Self::MasterRelay => Some(LadderElement {
                id,
                element_type: TypeTag::MasterRelay,
                name: "Master_Relay".to_string(),
                symbol: "MR".to_string(),
                ..Default::default()
            }),
            Self::Ton => Some(LadderElement::new_ton(id, "T00", "T00", 1000)),
            Self::Tof => {
                let mut el = LadderElement::new_ton(id, "T00", "T00", 1000);
                el.element_type = TypeTag::Tof;
                Some(el)
            }
            Self::Tp => {
                let mut el = LadderElement::new_ton(id, "T00", "T00", 500);
                el.element_type = TypeTag::Tp;
                el.preset_value1 = 500;
                Some(el)
            }
            Self::Ctu => Some(LadderElement::new_ctu(id, "C00", "C00", 10)),
            Self::Ctd => {
                let mut el = LadderElement::new_ctu(id, "C00", "C00", 10);
                el.element_type = TypeTag::Ctd;
                Some(el)
            }
            Self::Ctud => {
                let mut el = LadderElement::new_ctu(id, "C00", "C00", 10);
                el.element_type = TypeTag::Ctud;
                Some(el)
            }
            Self::ShiftReg => Some(LadderElement {
                id,
                element_type: TypeTag::ShiftRegisters,
                name: "S00".to_string(),
                symbol: "SHIFT_REG".to_string(),
                ..Default::default()
            }),
            Self::ShiftLeft => Some(LadderElement {
                id,
                element_type: TypeTag::ShiftLeft,
                name: "S00".to_string(),
                symbol: "SHL".to_string(),
                ..Default::default()
            }),
            Self::ShiftRight => Some(LadderElement {
                id,
                element_type: TypeTag::ShiftRight,
                name: "S00".to_string(),
                symbol: "SHR".to_string(),
                ..Default::default()
            }),
            Self::Move => Some(LadderElement {
                id,
                element_type: TypeTag::Move,
                name: "V00".to_string(),
                symbol: "MOV".to_string(),
                target_reg: "V00".to_string(),
                preset_value: 0,
                ..Default::default()
            }),
            Self::Compare => Some(LadderElement {
                id,
                element_type: TypeTag::Compare,
                name: "V00".to_string(),
                symbol: "FCP".to_string(),
                compare_op: "==".to_string(),
                preset_value: 10,
                ..Default::default()
            }),
            Self::Add | Self::Sub | Self::Mul | Self::Div | Self::Mod => {
                let op = match self {
                    Self::Add => "+",
                    Self::Sub => "-",
                    Self::Mul => "*",
                    Self::Div => "/",
                    Self::Mod => "%",
                    _ => "+",
                };
                Some(LadderElement {
                    id,
                    element_type: TypeTag::Math,
                    name: "V00".to_string(),
                    symbol: format!("MATH {}", op),
                    target_reg: "V00".to_string(),
                    math_op: op.to_string(),
                    preset_value: 1,
                    ..Default::default()
                })
            }
            Self::Adc => Some(LadderElement {
                id,
                element_type: TypeTag::Adc,
                name: "A00".to_string(),
                symbol: "ADC".to_string(),
                ..Default::default()
            }),
            Self::RTrig => Some(LadderElement {
                id,
                element_type: TypeTag::RTrig,
                name: "R_TRIG".to_string(),
                symbol: "R_TRIG".to_string(),
                ..Default::default()
            }),
            Self::FTrig => Some(LadderElement {
                id,
                element_type: TypeTag::FTrig,
                name: "F_TRIG".to_string(),
                symbol: "F_TRIG".to_string(),
                ..Default::default()
            }),
            Self::UartSend => Some(LadderElement {
                id,
                element_type: TypeTag::UartSend,
                name: "u_tx".to_string(),
                symbol: "UART_SEND".to_string(),
                ..Default::default()
            }),
            Self::UartRecv => Some(LadderElement {
                id,
                element_type: TypeTag::UartRecv,
                name: "u_rx".to_string(),
                symbol: "UART_RECV".to_string(),
                ..Default::default()
            }),
            Self::Wire => Some(LadderElement::new_wire(id)),
            Self::OpenCircuit => Some(LadderElement::new_open(id)),
            Self::Branch => Some(LadderElement::new_branch(
                id,
                vec![
                    vec![LadderElement::new_contact(id + 1, "X00", "X00", ContactMode::NormallyOpen)],
                    vec![LadderElement::new_contact(id + 2, "Y00", "Y00", ContactMode::NormallyOpen)],
                ],
            )),
            Self::Select | Self::Pan => None,
        }
    }
}
