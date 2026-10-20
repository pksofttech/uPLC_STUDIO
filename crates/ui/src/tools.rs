use my_canvas_core::{CoilMode, ContactMode, FbParams, LadderElement, TypeTag};

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
    ShiftVariable,
    ShiftRegReset,
    ShiftRegInbit,
    Move,
    Compare,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    BitAnd,
    BitOr,
    BitXor,
    BitNot,
    Adc,
    Pid,
    Scale,
    Limit,
    RTrig,
    FTrig,
    UartSend,
    UartRecv,
    Sr,
    Rs,
    Jump,
    Label,
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
            Self::ShiftReg => "Shift Register (Array)",
            Self::ShiftLeft => "Shift Left (SHL)",
            Self::ShiftRight => "Shift Right (SHR)",
            Self::ShiftVariable => "Shift Variable (SHV)",
            Self::ShiftRegReset => "Shift Register Reset (SRST)",
            Self::ShiftRegInbit => "Bit Shift In (SFT_BIT)",
            Self::Move => "Move [:=]",
            Self::Compare => "Compare [EQ, NE, GT, LT, GE, LE]",
            Self::Add => "Add [+=]",
            Self::Sub => "Sub [-=]",
            Self::Mul => "Mul [*=]",
            Self::Div => "Div [/=]",
            Self::Mod => "Mod [%=]",
            Self::BitAnd => "Bitwise AND [&]",
            Self::BitOr => "Bitwise OR [|]",
            Self::BitXor => "Bitwise XOR [^]",
            Self::BitNot => "Bitwise NOT [~]",
            Self::Adc => "A/D Converter Read (ADC)",
            Self::Pid => "PID Controller (PID)",
            Self::Scale => "Linear Scale (SCALE)",
            Self::Limit => "Clamp Limit (LIMIT)",
            Self::RTrig => "R_TRIG (Rising Edge Detector)",
            Self::FTrig => "F_TRIG (Falling Edge Detector)",
            Self::UartSend => "UART Send (TX)",
            Self::UartRecv => "UART Recv (RX)",
            Self::Sr => "SR (Set-Dominant Bistable FB)",
            Self::Rs => "RS (Reset-Dominant Bistable FB)",
            Self::Jump => "JMP (Jump to Label)",
            Self::Label => "LBL (Target Label)",
            Self::Wire => "Short Circuit Wire",
            Self::OpenCircuit => "Open Circuit",
            Self::Branch => "Parallel Branch",
        }
    }

    pub fn element_type(&self) -> Option<TypeTag> {
        match self {
            Self::Comment => Some(TypeTag::Comment),
            Self::ContactNo | Self::ContactNc | Self::ContactP | Self::ContactN => Some(TypeTag::Contact),
            Self::CoilNormal
            | Self::CoilNegated
            | Self::CoilSet
            | Self::CoilReset
            | Self::CoilP
            | Self::CoilN => Some(TypeTag::Coil),
            Self::MasterRelay => Some(TypeTag::MasterRelay),
            Self::Ton => Some(TypeTag::Ton),
            Self::Tof => Some(TypeTag::Tof),
            Self::Tp => Some(TypeTag::Tp),
            Self::Ctu => Some(TypeTag::Ctu),
            Self::Ctd => Some(TypeTag::Ctd),
            Self::Ctud => Some(TypeTag::Ctud),
            Self::ShiftReg => Some(TypeTag::ShiftRegisters),
            Self::ShiftLeft => Some(TypeTag::ShiftLeft),
            Self::ShiftRight => Some(TypeTag::ShiftRight),
            Self::ShiftVariable => Some(TypeTag::ShiftVariable),
            Self::ShiftRegReset => Some(TypeTag::ShiftRegReset),
            Self::ShiftRegInbit => Some(TypeTag::ShiftRegInbit),
            Self::Move => Some(TypeTag::Move),
            Self::Compare => Some(TypeTag::Compare),
            Self::Add | Self::Sub | Self::Mul | Self::Div | Self::Mod => Some(TypeTag::Math),
            Self::BitAnd | Self::BitOr | Self::BitXor | Self::BitNot => Some(TypeTag::Bitwise),
            Self::Adc => Some(TypeTag::Adc),
            Self::Pid => Some(TypeTag::Pid),
            Self::Scale => Some(TypeTag::Scale),
            Self::Limit => Some(TypeTag::Limit),
            Self::RTrig => Some(TypeTag::RTrig),
            Self::FTrig => Some(TypeTag::FTrig),
            Self::UartSend => Some(TypeTag::UartSend),
            Self::UartRecv => Some(TypeTag::UartRecv),
            Self::Sr => Some(TypeTag::Sr),
            Self::Rs => Some(TypeTag::Rs),
            Self::Jump => Some(TypeTag::Jump),
            Self::Label => Some(TypeTag::Label),
            Self::Wire => Some(TypeTag::Wire),
            Self::OpenCircuit => Some(TypeTag::OpenCircuit),
            Self::Branch => Some(TypeTag::Branch),
            _ => None,
        }
    }

    pub fn create_default_element(&self, id: u64) -> Option<LadderElement> {
        match self {
            Self::Comment => Some(LadderElement::new_comment(id, "COMMENT")),
            Self::ContactNo => Some(LadderElement::new_contact(id, "%IX0.0", "START_PB", ContactMode::NormallyOpen)),
            Self::ContactNc => Some(LadderElement::new_contact(id, "%IX0.1", "STOP_PB", ContactMode::NormallyClosed)),
            Self::ContactP => Some(LadderElement::new_contact(id, "%IX0.0", "START_PB", ContactMode::PositiveTransition)),
            Self::ContactN => Some(LadderElement::new_contact(id, "%IX0.0", "START_PB", ContactMode::NegativeTransition)),
            Self::CoilNormal => Some(LadderElement::new_coil(id, "%QX0.0", "MOTOR_RUN", CoilMode::Normal)),
            Self::CoilNegated => Some(LadderElement::new_coil(id, "%QX0.0", "MOTOR_RUN", CoilMode::Negated)),
            Self::CoilSet => Some(LadderElement::new_coil(id, "%QX0.0", "MOTOR_RUN", CoilMode::SetLatch)),
            Self::CoilReset => Some(LadderElement::new_coil(id, "%QX0.0", "MOTOR_RUN", CoilMode::ResetUnlatch)),
            Self::CoilP => Some(LadderElement::new_coil(id, "%QX0.0", "MOTOR_RUN", CoilMode::PositiveTransition)),
            Self::CoilN => Some(LadderElement::new_coil(id, "%QX0.0", "MOTOR_RUN", CoilMode::NegativeTransition)),
            Self::MasterRelay => Some(LadderElement {
                id,
                element_type: TypeTag::MasterRelay,
                name: "Master_Relay".to_string(),
                symbol: "MR".to_string(),
                ..Default::default()
            }),
            Self::Ton => Some(LadderElement::new_ton(id, "T0", "TIMER_DELAY", 1000)),
            Self::Tof => {
                let mut el = LadderElement::new_ton(id, "T0", "TIMER_OFF", 1000);
                el.element_type = TypeTag::Tof;
                Some(el)
            }
            Self::Tp => {
                let mut el = LadderElement::new_ton(id, "T0", "TIMER_PULSE", 500);
                el.element_type = TypeTag::Tp;
                el.preset_value1 = 500;
                Some(el)
            }
            Self::Ctu => Some(LadderElement::new_ctu(id, "C0", "COUNTER_UP", 10)),
            Self::Ctd => {
                let mut el = LadderElement::new_ctu(id, "C0", "COUNTER_DN", 10);
                el.element_type = TypeTag::Ctd;
                Some(el)
            }
            Self::Ctud => {
                let mut el = LadderElement::new_ctu(id, "C0", "COUNTER_UD", 10);
                el.element_type = TypeTag::Ctud;
                Some(el)
            }
            Self::ShiftReg => Some(LadderElement {
                id,
                element_type: TypeTag::ShiftRegisters,
                name: "%MW30".to_string(),
                symbol: "SHIFT_REG".to_string(),
                preset_value: 4,
                fb: FbParams {
                    in1: "%MW0".to_string(),
                    ..Default::default()
                },
                ..Default::default()
            }),
            Self::ShiftLeft => Some(LadderElement {
                id,
                element_type: TypeTag::ShiftLeft,
                name: "%MW30".to_string(),
                symbol: "SHL".to_string(),
                preset_value: 1,
                ..Default::default()
            }),
            Self::ShiftRight => Some(LadderElement {
                id,
                element_type: TypeTag::ShiftRight,
                name: "%MW30".to_string(),
                symbol: "SHR".to_string(),
                preset_value: 1,
                ..Default::default()
            }),
            Self::ShiftVariable => Some(LadderElement {
                id,
                element_type: TypeTag::ShiftVariable,
                name: "%MW30".to_string(),
                symbol: "SHV".to_string(),
                fb: FbParams {
                    in1: "%MW31".to_string(),
                    ..Default::default()
                },
                ..Default::default()
            }),
            Self::ShiftRegReset => Some(LadderElement {
                id,
                element_type: TypeTag::ShiftRegReset,
                name: "%MW30".to_string(),
                symbol: "SRST".to_string(),
                ..Default::default()
            }),
            Self::ShiftRegInbit => Some(LadderElement {
                id,
                element_type: TypeTag::ShiftRegInbit,
                name: "%MW30".to_string(),
                symbol: "SFT_BIT".to_string(),
                fb: FbParams {
                    in1: "%IX0.0".to_string(),
                    ..Default::default()
                },
                ..Default::default()
            }),
            Self::Move => Some(LadderElement {
                id,
                element_type: TypeTag::Move,
                name: "%MW0".to_string(),
                symbol: "MOV".to_string(),
                target_reg: "%MW0".to_string(),
                preset_value: 0,
                ..Default::default()
            }),
            Self::Compare => Some(LadderElement {
                id,
                element_type: TypeTag::Compare,
                name: "%MW0".to_string(),
                symbol: "CMP".to_string(),
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
                    name: "%MW0".to_string(),
                    symbol: format!("MATH {}", op),
                    target_reg: "%MW0".to_string(),
                    math_op: op.to_string(),
                    preset_value: 1,
                    ..Default::default()
                })
            }
            Self::BitAnd | Self::BitOr | Self::BitXor | Self::BitNot => {
                let op = match self {
                    Self::BitAnd => "AND",
                    Self::BitOr => "OR",
                    Self::BitXor => "XOR",
                    Self::BitNot => "NOT",
                    _ => "AND",
                };
                let in2 = if op == "NOT" { String::new() } else { "16#00FF".to_string() };
                Some(LadderElement {
                    id,
                    element_type: TypeTag::Bitwise,
                    name: "%MW1".to_string(),
                    symbol: format!("BIT_{}", op),
                    target_reg: "%MW1".to_string(),
                    math_op: op.to_string(),
                    fb: FbParams {
                        in1: "%MW0".to_string(),
                        in2,
                        ..Default::default()
                    },
                    ..Default::default()
                })
            }
            Self::Adc => Some(LadderElement {
                id,
                element_type: TypeTag::Adc,
                name: "%IW0".to_string(),
                symbol: "ADC_IN".to_string(),
                target_reg: "%MW0".to_string(),
                ..Default::default()
            }),
            Self::Pid => Some(LadderElement {
                id,
                element_type: TypeTag::Pid,
                name: "PID0".to_string(),
                symbol: "PID_LOOP".to_string(),
                target_reg: "%MW10".to_string(),
                fb: FbParams {
                    in1: "%IW0".to_string(),
                    in2: "500".to_string(),
                    k: [100, 10, 0, 1000],
                    mode: 0,
                },
                ..Default::default()
            }),
            Self::Scale => Some(LadderElement {
                id,
                element_type: TypeTag::Scale,
                name: "SCALE0".to_string(),
                symbol: "SCALE_ANALOG".to_string(),
                target_reg: "%MW20".to_string(),
                fb: FbParams {
                    in1: "%IW0".to_string(),
                    in2: String::new(),
                    k: [0, 4095, 0, 1000],
                    mode: 0,
                },
                ..Default::default()
            }),
            Self::Limit => Some(LadderElement {
                id,
                element_type: TypeTag::Limit,
                name: "LIMIT0".to_string(),
                symbol: "CLAMP_VAL".to_string(),
                target_reg: "%MW21".to_string(),
                fb: FbParams {
                    in1: "%MW20".to_string(),
                    in2: String::new(),
                    k: [0, 1000, 0, 0],
                    mode: 0,
                },
                ..Default::default()
            }),
            Self::RTrig => Some(LadderElement {
                id,
                element_type: TypeTag::RTrig,
                name: "%IX0.0".to_string(),
                symbol: "R_TRIG".to_string(),
                ..Default::default()
            }),
            Self::FTrig => Some(LadderElement {
                id,
                element_type: TypeTag::FTrig,
                name: "%IX0.0".to_string(),
                symbol: "F_TRIG".to_string(),
                ..Default::default()
            }),
            Self::UartSend => Some(LadderElement {
                id,
                element_type: TypeTag::UartSend,
                name: "U_TX0".to_string(),
                symbol: "UART_SEND".to_string(),
                fb: FbParams {
                    in1: "%MW0".to_string(),
                    in2: String::new(),
                    k: [0; 4],
                    mode: 1,
                },
                ..Default::default()
            }),
            Self::UartRecv => Some(LadderElement {
                id,
                element_type: TypeTag::UartRecv,
                name: "U_RX0".to_string(),
                symbol: "UART_RECV".to_string(),
                target_reg: "%MW1".to_string(),
                ..Default::default()
            }),
            Self::Sr => Some(LadderElement {
                id,
                element_type: TypeTag::Sr,
                name: "%MX0.0".to_string(),
                symbol: "SR1".to_string(),
                target_reg: "%IX0.1".to_string(),
                ..Default::default()
            }),
            Self::Rs => Some(LadderElement {
                id,
                element_type: TypeTag::Rs,
                name: "%MX0.0".to_string(),
                symbol: "RS1".to_string(),
                target_reg: "%IX0.1".to_string(),
                ..Default::default()
            }),
            Self::Jump => Some(LadderElement {
                id,
                element_type: TypeTag::Jump,
                name: "LBL1".to_string(),
                symbol: "JMP".to_string(),
                ..Default::default()
            }),
            Self::Label => Some(LadderElement {
                id,
                element_type: TypeTag::Label,
                name: "LBL1".to_string(),
                symbol: "LBL".to_string(),
                ..Default::default()
            }),
            Self::Wire => Some(LadderElement::new_wire(id)),
            Self::OpenCircuit => Some(LadderElement::new_open(id)),
            Self::Branch => Some(LadderElement::new_branch(
                id,
                vec![
                    vec![LadderElement::new_contact(id + 1, "%IX0.0", "START_PB", ContactMode::NormallyOpen)],
                    vec![LadderElement::new_contact(id + 2, "%QX0.0", "MOTOR_RUN", ContactMode::NormallyOpen)],
                ],
            )),
            Self::Select | Self::Pan => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_p4_tools_create_valid_default_elements() {
        let tools = [
            (ActiveTool::Pid, TypeTag::Pid),
            (ActiveTool::Scale, TypeTag::Scale),
            (ActiveTool::Limit, TypeTag::Limit),
            (ActiveTool::BitAnd, TypeTag::Bitwise),
            (ActiveTool::BitOr, TypeTag::Bitwise),
            (ActiveTool::BitXor, TypeTag::Bitwise),
            (ActiveTool::BitNot, TypeTag::Bitwise),
            (ActiveTool::ShiftReg, TypeTag::ShiftRegisters),
            (ActiveTool::ShiftLeft, TypeTag::ShiftLeft),
            (ActiveTool::ShiftRight, TypeTag::ShiftRight),
            (ActiveTool::ShiftVariable, TypeTag::ShiftVariable),
            (ActiveTool::ShiftRegReset, TypeTag::ShiftRegReset),
            (ActiveTool::ShiftRegInbit, TypeTag::ShiftRegInbit),
            (ActiveTool::Adc, TypeTag::Adc),
            (ActiveTool::UartSend, TypeTag::UartSend),
            (ActiveTool::UartRecv, TypeTag::UartRecv),
        ];
        for (idx, (tool, expected_tag)) in tools.iter().enumerate() {
            assert_eq!(tool.element_type(), Some(*expected_tag));
            let el = tool
                .create_default_element(100 + idx as u64)
                .expect("tool should create default element");
            assert_eq!(el.element_type, *expected_tag);
            assert!(!el.name.is_empty());
            assert!(
                el.fb_value_summary().is_some(),
                "Tool {:?} should produce an fb_value_summary",
                tool
            );
        }

        let pid = ActiveTool::Pid.create_default_element(1).unwrap();
        assert_eq!(pid.fb.k, [100, 10, 0, 1000]);
        assert_eq!(pid.fb.in1, "%IW0");
        assert_eq!(pid.fb.in2, "500");
        assert_eq!(pid.target_reg, "%MW10");

        let scale = ActiveTool::Scale.create_default_element(2).unwrap();
        assert_eq!(scale.fb.k, [0, 4095, 0, 1000]);
        assert_eq!(scale.fb.in1, "%IW0");
        assert_eq!(scale.target_reg, "%MW20");

        let limit = ActiveTool::Limit.create_default_element(3).unwrap();
        assert_eq!(limit.fb.k[0], 0);
        assert_eq!(limit.fb.k[1], 1000);
    }
}
