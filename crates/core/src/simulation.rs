use std::collections::HashMap;
use crate::ladder::{CoilMode, ContactMode, LadderElement, LadderProgram, TypeTag};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimState {
    Stopped,
    Running,
    Step,
}

#[derive(Debug, Clone)]
pub struct PlcSimulator {
    pub state: SimState,
    pub simulation_enabled: bool,
    pub cycle_time_ms: u32,
    pub speed_multiplier: u32,
    pub scan_count: u64,
    pub simulation_time_ms: u64,
    
    // Digital inputs X0..X7
    pub inputs: HashMap<String, bool>,
    // Digital outputs Y0..Y7
    pub outputs: HashMap<String, bool>,
    // Internal relays / markers M0..M31 or R0..R31
    pub markers: HashMap<String, bool>,
    // Timer states: (accum_ms, done)
    pub timers: HashMap<String, (u32, bool)>,
    // Counter states: (count, done, last_input_edge)
    pub counters: HashMap<String, (i32, bool, bool)>,
    // Data registers D0..D15 / V0..V15
    pub registers: HashMap<String, i32>,
    // Edge detection states keyed by element ID
    pub edge_states: HashMap<u64, bool>,
}

impl Default for PlcSimulator {
    fn default() -> Self {
        let mut inputs = HashMap::new();
        for i in 0..8 {
            inputs.insert(format!("X{}", i), false);
            inputs.insert(format!("X{:02}", i), false);
        }
        
        let mut outputs = HashMap::new();
        for i in 0..8 {
            outputs.insert(format!("Y{}", i), false);
            outputs.insert(format!("Y{:02}", i), false);
        }

        let mut markers = HashMap::new();
        for i in 0..16 {
            markers.insert(format!("M{}", i), false);
            markers.insert(format!("R{}", i), false);
            markers.insert(format!("R{:02}", i), false);
        }

        let mut registers = HashMap::new();
        for i in 0..16 {
            registers.insert(format!("D{}", i), 0);
            registers.insert(format!("V{:02}", i), 0);
        }

        Self {
            state: SimState::Stopped,
            simulation_enabled: false,
            cycle_time_ms: 10,
            speed_multiplier: 1,
            scan_count: 0,
            simulation_time_ms: 0,
            inputs,
            outputs,
            markers,
            timers: HashMap::new(),
            counters: HashMap::new(),
            registers,
            edge_states: HashMap::new(),
        }
    }
}

impl PlcSimulator {
    pub fn new() -> Self {
        Self::default()
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
        self.inputs.insert(name.to_string(), value);
    }

    pub fn toggle_input(&mut self, name: &str) -> bool {
        let cur = self.get_var_value(name);
        let new_val = !cur;
        self.set_var_value(name, new_val);
        new_val
    }

    pub fn get_var_value(&self, name: &str) -> bool {
        let upper = name.to_uppercase();
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
        false
    }

    pub fn set_var_value(&mut self, name: &str, val: bool) {
        let upper = name.to_uppercase();
        if upper.starts_with('Y') {
            self.outputs.insert(upper, val);
        } else if upper.starts_with('M') || upper.starts_with('R') {
            self.markers.insert(upper, val);
        } else {
            self.inputs.insert(upper, val);
        }
    }

    pub fn reset(&mut self, program: &mut LadderProgram) {
        self.state = SimState::Stopped;
        self.scan_count = 0;
        self.simulation_time_ms = 0;
        for val in self.outputs.values_mut() {
            *val = false;
        }
        for val in self.markers.values_mut() {
            *val = false;
        }
        self.timers.clear();
        self.counters.clear();
        self.edge_states.clear();
        
        for rung in &mut program.rungs {
            rung.is_energized = false;
            for el in &mut rung.elements {
                el.is_energized = false;
                el.current_accum = 0;
                el.is_done = false;
                for path in &mut el.branch_paths {
                    for sub in path {
                        sub.is_energized = false;
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

        for rung in &mut program.rungs {
            let mut power_flow = true;

            for elem in &mut rung.elements {
                power_flow = self.eval_element(elem, power_flow, scaled_dt);
            }

            rung.is_energized = power_flow;
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
                power_in && entry.1
            }
            TypeTag::Tof => {
                let entry = self.timers.entry(elem.name.to_uppercase()).or_insert((0, false));
                if power_in {
                    entry.0 = elem.preset_value.max(0) as u32;
                    entry.1 = true;
                } else if entry.0 > 0 {
                    entry.0 = entry.0.saturating_sub(dt_ms);
                    entry.1 = entry.0 > 0;
                } else {
                    entry.1 = false;
                }
                elem.current_accum = entry.0 as i32;
                elem.is_done = entry.1;
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
                entry.1
            }
            TypeTag::Ctu | TypeTag::Ctud => {
                let entry = self.counters.entry(elem.name.to_uppercase()).or_insert((0, false, false));
                let last_edge = entry.2;
                entry.2 = power_in;
                if power_in && !last_edge {
                    entry.0 += 1;
                    if entry.0 >= elem.preset_value {
                        entry.1 = true;
                    }
                }
                elem.current_accum = entry.0;
                elem.is_done = entry.1;
                power_in && entry.1
            }
            TypeTag::Ctd => {
                let entry = self.counters.entry(elem.name.to_uppercase()).or_insert((elem.preset_value, false, false));
                let last_edge = entry.2;
                entry.2 = power_in;
                if power_in && !last_edge {
                    entry.0 = (entry.0 - 1).max(0);
                    entry.1 = entry.0 <= 0;
                }
                elem.current_accum = entry.0;
                elem.is_done = entry.1;
                power_in && entry.1
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
                let reg_val = *self.registers.get(&elem.name.to_uppercase()).unwrap_or(&0);
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
                elem.is_done = conducts;
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
                    let cur = *self.registers.get(&elem.target_reg.to_uppercase()).unwrap_or(&0);
                    let res = match elem.math_op.as_str() {
                        "+" => cur.saturating_add(elem.preset_value),
                        "-" => cur.saturating_sub(elem.preset_value),
                        "*" => cur.saturating_mul(elem.preset_value),
                        "/" => if elem.preset_value != 0 { cur / elem.preset_value } else { cur },
                        "%" => if elem.preset_value != 0 { cur % elem.preset_value } else { cur },
                        _ => cur,
                    };
                    self.registers.insert(elem.target_reg.to_uppercase(), res);
                    elem.current_accum = res;
                }
                power_in
            }
            TypeTag::Move => {
                if power_in {
                    self.registers.insert(elem.target_reg.to_uppercase(), elem.preset_value);
                    elem.current_accum = elem.preset_value;
                }
                power_in
            }
            _ => power_in,
        };

        elem.is_energized = power_out;
        power_out
    }
}
