use serde::{Deserialize, Serialize};
use crate::ladder::{
    CoilMode, CoilStyle, ContactMode, ContactStyle, InsertPosition, LadderElement, LadderProgram,
    Rung, SchematicStyle, TypeTag,
};
use crate::il_format::IlParser;
use crate::simulation::PlcSimulator;
use crate::bytecode::PlcStatusSnapshot;

const MAX_UNDO_HISTORY: usize = 64;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub program: LadderProgram,
    #[serde(default)]
    pub schematic_style: SchematicStyle,
    #[serde(default)]
    pub contact_style: ContactStyle,
    #[serde(default)]
    pub coil_style: CoilStyle,
    #[serde(skip)]
    pub simulator: PlcSimulator,
    #[serde(skip)]
    pub selected_rung_idx: Option<usize>,
    #[serde(skip)]
    pub selected_element_id: Option<u64>,
    #[serde(skip)]
    pub selected_element_ids: Vec<u64>,
    #[serde(skip)]
    pub file_path: Option<String>,
    #[serde(skip)]
    pub is_dirty: bool,
    #[serde(skip)]
    pub undo_stack: Vec<LadderProgram>,
    #[serde(skip)]
    pub redo_stack: Vec<LadderProgram>,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            program: LadderProgram::default(),
            schematic_style: SchematicStyle::default(),
            contact_style: ContactStyle::default(),
            coil_style: CoilStyle::default(),
            simulator: PlcSimulator::default(),
            selected_rung_idx: Some(0),
            selected_element_id: None,
            selected_element_ids: Vec::new(),
            file_path: None,
            is_dirty: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }
}

impl Document {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn new_empty() -> Self {
        Self {
            program: LadderProgram::new_empty(),
            schematic_style: SchematicStyle::default(),
            contact_style: ContactStyle::default(),
            coil_style: CoilStyle::default(),
            simulator: PlcSimulator::default(),
            selected_rung_idx: Some(0),
            selected_element_id: None,
            selected_element_ids: Vec::new(),
            file_path: None,
            is_dirty: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    pub fn set_schematic_style(&mut self, style: SchematicStyle) {
        self.schematic_style = style;
        self.contact_style = style.contact_style();
        self.coil_style = style.coil_style();
        self.is_dirty = true;
    }

    /// Save current program state onto the undo stack before modifying the circuit
    pub fn push_undo_snapshot(&mut self) {
        if self.undo_stack.last() == Some(&self.program) {
            return;
        }
        if self.undo_stack.len() >= MAX_UNDO_HISTORY {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(self.program.clone());
        self.redo_stack.clear();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo(&mut self) -> bool {
        if let Some(prev) = self.undo_stack.pop() {
            self.redo_stack.push(self.program.clone());
            self.program = prev;
            self.clamp_selection();
            self.is_dirty = true;
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self) -> bool {
        if let Some(next) = self.redo_stack.pop() {
            self.undo_stack.push(self.program.clone());
            self.program = next;
            self.clamp_selection();
            self.is_dirty = true;
            true
        } else {
            false
        }
    }

    fn clamp_selection(&mut self) {
        if self.program.rungs.is_empty() {
            self.selected_rung_idx = None;
        } else if let Some(idx) = self.selected_rung_idx {
            if idx >= self.program.rungs.len() {
                self.selected_rung_idx = Some(self.program.rungs.len() - 1);
            }
        }
    }

    pub fn clear(&mut self) {
        let prev_hw = self.program.hardware.clone();
        self.program = LadderProgram::new_empty();
        if !prev_hw.is_empty() {
            self.program.hardware = prev_hw;
        }
        self.simulator.reset(&mut self.program);
        for val in self.simulator.inputs.values_mut() {
            *val = false;
        }
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.selected_rung_idx = Some(0);
        self.selected_element_id = None;
        self.selected_element_ids.clear();
        self.file_path = None;
        self.is_dirty = false;
    }

    pub fn clear_element_selection(&mut self) {
        self.selected_element_id = None;
        self.selected_element_ids.clear();
    }

    pub fn is_element_selected(&self, id: u64) -> bool {
        self.selected_element_id == Some(id) || self.selected_element_ids.contains(&id)
    }

    pub fn set_single_selection(&mut self, rung_idx: Option<usize>, id: u64) {
        if let Some(r) = rung_idx {
            self.selected_rung_idx = Some(r);
        }
        self.selected_element_id = Some(id);
        self.selected_element_ids = vec![id];
    }

    pub fn toggle_element_selection(&mut self, rung_idx: Option<usize>, id: u64) {
        if let Some(r) = rung_idx {
            self.selected_rung_idx = Some(r);
        }
        if self.selected_element_ids.is_empty() {
            if let Some(cur) = self.selected_element_id {
                self.selected_element_ids.push(cur);
            }
        }
        if let Some(pos) = self.selected_element_ids.iter().position(|&x| x == id) {
            self.selected_element_ids.remove(pos);
            self.selected_element_id = self.selected_element_ids.last().copied();
        } else {
            self.selected_element_ids.push(id);
            self.selected_element_id = Some(id);
        }
    }

    pub fn set_box_selection(&mut self, hits: &[(usize, u64)], additive: bool) {
        if !additive {
            self.selected_element_ids.clear();
            self.selected_element_id = None;
        } else if self.selected_element_ids.is_empty() {
            if let Some(cur) = self.selected_element_id {
                self.selected_element_ids.push(cur);
            }
        }

        for &(r_idx, id) in hits {
            if !self.selected_element_ids.contains(&id) {
                self.selected_element_ids.push(id);
            }
            if self.selected_element_id.is_none() {
                self.selected_rung_idx = Some(r_idx);
            }
        }
        self.selected_element_id = self.selected_element_ids.first().copied();
    }

    pub fn select_all_elements(&mut self) {
        let mut all_ids = Vec::new();
        for rung in &self.program.rungs {
            for el in &rung.elements {
                if el.element_type == TypeTag::Branch {
                    for path in &el.branch_paths {
                        for sub in path {
                            all_ids.push(sub.id);
                        }
                    }
                } else {
                    all_ids.push(el.id);
                }
            }
        }
        self.selected_element_id = all_ids.first().copied();
        self.selected_element_ids = all_ids;
    }

    pub fn add_rung(&mut self) -> usize {
        self.push_undo_snapshot();
        let idx = self.program.add_rung();
        self.selected_rung_idx = Some(idx);
        self.is_dirty = true;
        idx
    }

    pub fn insert_rung(&mut self, index: usize) -> usize {
        self.push_undo_snapshot();
        let idx = self.program.insert_rung(index);
        self.selected_rung_idx = Some(idx);
        self.is_dirty = true;
        idx
    }

    pub fn copy_rung_at(&mut self, src_idx: usize, insert_idx: usize) -> usize {
        self.push_undo_snapshot();
        let idx = self.program.copy_rung_at(src_idx, insert_idx);
        // `reassign_ids` leaves `next_element_id` on the last used id; keep alloc_id collision-free
        self.sync_next_element_id();
        self.selected_rung_idx = Some(idx);
        self.is_dirty = true;
        idx
    }

    pub fn delete_rung(&mut self, index: usize) {
        self.push_undo_snapshot();
        self.program.delete_rung(index);
        self.selected_rung_idx = Some(index.saturating_sub(1));
        self.is_dirty = true;
    }

    pub fn move_rung_up(&mut self, index: usize) -> usize {
        if index > 0 && index < self.program.rungs.len() {
            self.push_undo_snapshot();
            let new_idx = self.program.move_rung_up(index);
            self.selected_rung_idx = Some(new_idx);
            self.is_dirty = true;
            new_idx
        } else {
            index
        }
    }

    pub fn move_rung_down(&mut self, index: usize) -> usize {
        if index + 1 < self.program.rungs.len() {
            self.push_undo_snapshot();
            let new_idx = self.program.move_rung_down(index);
            self.selected_rung_idx = Some(new_idx);
            self.is_dirty = true;
            new_idx
        } else {
            index
        }
    }

    pub fn step_simulation(&mut self, dt_ms: u32) {
        self.simulator.step_scan(&mut self.program, dt_ms);
    }

    pub fn start_simulation(&mut self) {
        self.simulator.start(&mut self.program);
    }

    pub fn stop_simulation(&mut self) {
        self.simulator.stop(&mut self.program);
    }

    pub fn reset_simulation(&mut self) {
        self.simulator.reset(&mut self.program);
    }

    pub fn single_step_simulation(&mut self, dt_ms: u32) {
        self.simulator.step(&mut self.program, dt_ms);
    }

    pub fn toggle_simulation(&mut self) {
        if self.simulator.is_running() {
            self.stop_simulation();
        } else {
            self.start_simulation();
        }
    }

    pub fn toggle_input(&mut self, name: &str) -> bool {
        let res = self.simulator.toggle_input(name);
        if self.simulator.is_step() {
            self.simulator.step_scan(&mut self.program, 0);
        }
        res
    }

    pub fn set_input(&mut self, name: &str, val: bool) {
        self.simulator.set_input(name, val);
        if self.simulator.is_step() {
            self.simulator.step_scan(&mut self.program, 0);
        }
    }

    pub fn force_bit(&mut self, addr: u8, val: bool) {
        self.simulator.force_bit(addr, val);
        self.simulator.evaluate_conduction_only(&mut self.program);
    }

    pub fn unforce_bit(&mut self, addr: u8) {
        self.simulator.unforce_bit(addr);
        self.simulator.evaluate_conduction_only(&mut self.program);
    }

    pub fn clear_all_forces(&mut self) {
        self.simulator.clear_all_forces();
        self.simulator.evaluate_conduction_only(&mut self.program);
    }

    pub fn is_bit_forced(&self, addr: u8) -> Option<bool> {
        self.simulator.is_bit_forced(addr)
    }

    pub fn write_word_register(&mut self, addr: u8, val: i32) {
        self.simulator.registers.insert(format!("%MW{}", addr), val);
        self.simulator.registers.insert(format!("D{}", addr), val);
        self.simulator.registers.insert(format!("D{:02}", addr), val);
        self.simulator.registers.insert(format!("V{}", addr), val);
        self.simulator.registers.insert(format!("V{:02}", addr), val);
        self.simulator.evaluate_conduction_only(&mut self.program);
    }

    /// Update process image from an Online Monitoring Status Snapshot and re-evaluate ladder conduction
    pub fn apply_online_snapshot(&mut self, snapshot: &PlcStatusSnapshot) {
        self.simulator.apply_online_snapshot(snapshot);
        // STM32F103C6T6 Micro-VM: AI3 is the STM32F1 temp sensor (different formula than RP2040)
        if self.program.hardware.to_ascii_lowercase().contains("stm32f103") {
            let t = crate::simulation::PlcSimulator::cpu_temp_c01_from_raw(snapshot.data_reg[3], true);
            self.simulator.set_cpu_temp_registers(t);
        }
        if snapshot.running {
            self.simulator.evaluate_conduction_only(&mut self.program);
        } else {
            self.simulator.clear_conduction(&mut self.program);
        }
    }

    /// Capture status snapshot from current document/simulator state (including Bytecode signature)
    pub fn capture_status_snapshot(&self) -> PlcStatusSnapshot {
        let mut snap = self.simulator.capture_status_snapshot();
        let (prog_len, prog_crc) = self.compile_to_bytecode().program_signature();
        snap.prog_len = prog_len;
        snap.prog_crc = prog_crc;
        snap
    }

    /// Enable or disable Online Monitoring mode
    pub fn set_online_mode(&mut self, enabled: bool) {
        self.simulator.online_mode = enabled;
        if enabled {
            self.simulator.evaluate_conduction_only(&mut self.program);
        } else if self.simulator.is_stopped() {
            // Clear energized states if not simulating
            self.clear_conduction();
        }
    }

    /// Clear visual power flow and de-energize all elements on the canvas
    pub fn clear_conduction(&mut self) {
        self.simulator.clear_conduction(&mut self.program);
    }

    pub fn selected_rung_mut(&mut self) -> Option<&mut Rung> {
        let idx = self.selected_rung_idx?;
        self.program.rungs.get_mut(idx)
    }

    pub fn selected_element(&self) -> Option<&LadderElement> {
        let target_id = self.selected_element_id?;
        self.element_by_id(target_id)
    }

    pub fn element_by_id(&self, target_id: u64) -> Option<&LadderElement> {
        for rung in &self.program.rungs {
            for el in &rung.elements {
                if el.id == target_id {
                    return Some(el);
                }
                for path in &el.branch_paths {
                    for sub in path {
                        if sub.id == target_id {
                            return Some(sub);
                        }
                    }
                }
            }
        }
        None
    }

    pub fn selected_element_mut(&mut self) -> Option<&mut LadderElement> {
        let target_id = self.selected_element_id?;
        self.element_by_id_mut(target_id)
    }

    pub fn element_by_id_mut(&mut self, target_id: u64) -> Option<&mut LadderElement> {
        for rung in &mut self.program.rungs {
            for el in &mut rung.elements {
                if el.id == target_id {
                    return Some(el);
                }
                for path in &mut el.branch_paths {
                    for sub in path {
                        if sub.id == target_id {
                            return Some(sub);
                        }
                    }
                }
            }
        }
        None
    }

    /// Return all occurrences `(rung_idx, elem_id, element_type)` of a given PLC address
    pub fn find_address_usages(&self, addr: &str) -> Vec<(usize, u64, TypeTag)> {
        let clean = addr.trim();
        let canon = LadderProgram::canonical_address(clean);
        let mut usages = Vec::new();
        for (r_idx, rung) in self.program.rungs.iter().enumerate() {
            for el in &rung.elements {
                if el.element_type == TypeTag::Branch {
                    for path in &el.branch_paths {
                        for sub in path {
                            if sub.name.eq_ignore_ascii_case(clean)
                                || sub.name.eq_ignore_ascii_case(&canon)
                                || LadderProgram::canonical_address(&sub.name).eq_ignore_ascii_case(&canon)
                            {
                                usages.push((r_idx, sub.id, sub.element_type));
                            }
                        }
                    }
                } else if el.name.eq_ignore_ascii_case(clean)
                    || el.name.eq_ignore_ascii_case(&canon)
                    || LadderProgram::canonical_address(&el.name).eq_ignore_ascii_case(&canon)
                {
                    usages.push((r_idx, el.id, el.element_type));
                }
            }
        }
        usages
    }

    /// Resolve symbol name for an address from GlobalSymbolTable or IO mappings.
    pub fn resolve_symbol_for_address(&self, address: &str) -> Option<String> {
        self.program.resolve_symbol(address).map(|s| s.to_string())
    }

    /// Resolve display symbol for an element: checks GlobalSymbolTable, then IO table, then elem.symbol.
    pub fn resolve_element_symbol<'a>(&'a self, elem: &'a LadderElement) -> &'a str {
        if let Some(sym) = self.program.resolve_symbol(&elem.name) {
            sym
        } else if !elem.symbol.is_empty() {
            &elem.symbol
        } else {
            &elem.name
        }
    }

    /// Set symbol in Global Symbol Table (Single Source of Truth) and cascade update across all
    /// elements in the project that reference this address, as well as the IO mapping table.
    pub fn set_global_symbol(&mut self, address: &str, symbol: &str, comment: Option<&str>) -> usize {
        let clean_addr = address.trim();
        if clean_addr.is_empty() {
            return 0;
        }
        let canon = LadderProgram::canonical_address(clean_addr);
        let clean_sym = symbol.trim();

        // 1. Update GlobalSymbolTable
        self.program.symbols.set_symbol(clean_addr, clean_sym, comment);

        // 2. Synchronize IO mappings table
        for io in &mut self.program.io_mappings {
            if io.plc_address.eq_ignore_ascii_case(clean_addr)
                || io.iec_address.eq_ignore_ascii_case(clean_addr)
                || io.plc_address.eq_ignore_ascii_case(&canon)
                || io.iec_address.eq_ignore_ascii_case(&canon)
            {
                io.symbol = clean_sym.to_string();
                if let Some(c) = comment {
                    if !c.is_empty() {
                        io.description = c.to_string();
                    }
                }
            }
        }

        // 3. Cascade update all matching elements across all rungs (including parallel branches)
        let mut count = 0;
        fn update_elem_sym(el: &mut LadderElement, raw_addr: &str, canon_addr: &str, new_sym: &str, count: &mut usize) {
            if el.element_type != TypeTag::Branch {
                if el.name.eq_ignore_ascii_case(raw_addr)
                    || el.name.eq_ignore_ascii_case(canon_addr)
                    || LadderProgram::canonical_address(&el.name).eq_ignore_ascii_case(canon_addr)
                {
                    el.symbol = new_sym.to_string();
                    *count += 1;
                }
            }
            for path in &mut el.branch_paths {
                for sub in path {
                    update_elem_sym(sub, raw_addr, canon_addr, new_sym, count);
                }
            }
        }

        for rung in &mut self.program.rungs {
            for el in &mut rung.elements {
                update_elem_sym(el, clean_addr, &canon, clean_sym, &mut count);
            }
        }

        if count > 0 {
            self.is_dirty = true;
        }

        count
    }

    /// Navigate left/right across elements in the currently selected rung
    pub fn select_neighbor_element(&mut self, delta: isize) {
        let r_idx = self.selected_rung_idx.unwrap_or(0);
        let Some(rung) = self.program.rungs.get(r_idx) else {
            return;
        };
        let mut flat_ids = Vec::new();
        for el in &rung.elements {
            if el.element_type == TypeTag::Branch {
                for path in &el.branch_paths {
                    for sub in path {
                        flat_ids.push(sub.id);
                    }
                }
            } else {
                flat_ids.push(el.id);
            }
        }
        if flat_ids.is_empty() {
            self.clear_element_selection();
            return;
        }
        if let Some(cur_id) = self.selected_element_id {
            if let Some(pos) = flat_ids.iter().position(|&id| id == cur_id) {
                let next_pos = (pos as isize + delta).clamp(0, flat_ids.len() as isize - 1) as usize;
                let nid = flat_ids[next_pos];
                self.selected_element_id = Some(nid);
                self.selected_element_ids = vec![nid];
                return;
            }
        }
        let nid = flat_ids[0];
        self.selected_element_id = Some(nid);
        self.selected_element_ids = vec![nid];
    }

    /// Navigate up/down across rungs and select the first element in that rung
    pub fn select_neighbor_rung(&mut self, delta: isize) {
        if self.program.rungs.is_empty() {
            return;
        }
        let cur = self.selected_rung_idx.unwrap_or(0) as isize;
        let next = (cur + delta).clamp(0, self.program.rungs.len() as isize - 1) as usize;
        self.selected_rung_idx = Some(next);
        let rung = &self.program.rungs[next];
        let first_id = rung.elements.first().and_then(|el| {
            if el.element_type == TypeTag::Branch {
                el.branch_paths.first().and_then(|p| p.first()).map(|s| s.id)
            } else {
                Some(el.id)
            }
        });
        self.selected_element_id = first_id;
        self.selected_element_ids = first_id.into_iter().collect();
    }

    pub fn select_by_name(&mut self, name: &str) {
        for (r_idx, rung) in self.program.rungs.iter().enumerate() {
            for el in &rung.elements {
                if el.name == name {
                    self.set_single_selection(Some(r_idx), el.id);
                    return;
                }
                for path in &el.branch_paths {
                    for sub in path {
                        if sub.name == name {
                            self.set_single_selection(Some(r_idx), sub.id);
                            return;
                        }
                    }
                }
            }
        }
    }

    /// Remove and return an element by ID from anywhere in the program, cleaning up empty branches
    fn take_element_by_id(&mut self, target_id: u64) -> Option<LadderElement> {
        for rung in &mut self.program.rungs {
            if let Some(pos) = rung.elements.iter().position(|e| e.id == target_id) {
                return Some(rung.elements.remove(pos));
            }
            let mut removed: Option<LadderElement> = None;
            for el in &mut rung.elements {
                for path in &mut el.branch_paths {
                    if let Some(pos) = path.iter().position(|e| e.id == target_id) {
                        removed = Some(path.remove(pos));
                        break;
                    }
                }
                if removed.is_some() {
                    el.branch_paths.retain(|p| !p.is_empty());
                    break;
                }
            }
            if removed.is_some() {
                // Unwrap single-path branch back into series elements if only 1 path remains
                let mut i = 0;
                while i < rung.elements.len() {
                    if rung.elements[i].element_type == TypeTag::Branch {
                        if rung.elements[i].branch_paths.is_empty() {
                            rung.elements.remove(i);
                            continue;
                        } else if rung.elements[i].branch_paths.len() == 1 {
                            let single_path = rung.elements.remove(i).branch_paths.remove(0);
                            for (offset, item) in single_path.into_iter().enumerate() {
                                rung.elements.insert(i + offset, item);
                            }
                        }
                    }
                    i += 1;
                }
                return removed;
            }
        }
        None
    }

    pub fn delete_selected_element(&mut self) {
        let mut targets = self.selected_element_ids.clone();
        if let Some(id) = self.selected_element_id {
            if !targets.contains(&id) {
                targets.push(id);
            }
        }
        if targets.is_empty() {
            return;
        }
        self.push_undo_snapshot();
        let mut any_removed = false;
        for id in targets {
            if self.take_element_by_id(id).is_some() {
                any_removed = true;
            }
        }
        if any_removed {
            self.clear_element_selection();
            self.is_dirty = true;
        } else {
            self.undo_stack.pop();
        }
    }

    pub fn toggle_selected_elements_mode(&mut self) {
        let mut targets = self.selected_element_ids.clone();
        if let Some(id) = self.selected_element_id {
            if !targets.contains(&id) {
                targets.push(id);
            }
        }
        if targets.is_empty() {
            return;
        }
        self.push_undo_snapshot();
        for id in targets {
            if let Some(el) = self.element_by_id_mut(id) {
                match el.element_type {
                    TypeTag::Contact => {
                        el.contact_mode = match el.contact_mode {
                            ContactMode::NormallyOpen => ContactMode::NormallyClosed,
                            ContactMode::NormallyClosed => ContactMode::PositiveTransition,
                            ContactMode::PositiveTransition => ContactMode::NegativeTransition,
                            ContactMode::NegativeTransition => ContactMode::NormallyOpen,
                        };
                    }
                    TypeTag::Coil => {
                        el.coil_mode = match el.coil_mode {
                            CoilMode::Normal => CoilMode::Negated,
                            CoilMode::Negated => CoilMode::SetLatch,
                            CoilMode::SetLatch => CoilMode::ResetUnlatch,
                            CoilMode::ResetUnlatch => CoilMode::PositiveTransition,
                            CoilMode::PositiveTransition => CoilMode::NegativeTransition,
                            CoilMode::NegativeTransition => CoilMode::Normal,
                        };
                    }
                    _ => {}
                }
            }
        }
        self.is_dirty = true;
    }

    /// Drag & Drop: Move an existing element `dragged_id` to `target_rung_idx` (relative to `target_elem_id` or appended to rung)
    pub fn move_element(
        &mut self,
        dragged_id: u64,
        target_rung_idx: usize,
        target_elem_id: Option<u64>,
        pos: InsertPosition,
    ) -> bool {
        if Some(dragged_id) == target_elem_id {
            return false;
        }
        if target_rung_idx >= self.program.rungs.len() {
            return self.move_element_to_new_rung(dragged_id, None);
        }
        self.push_undo_snapshot();
        let elem = match self.take_element_by_id(dragged_id) {
            Some(el) => el,
            None => {
                self.undo_stack.pop();
                return false;
            }
        };

        self.selected_rung_idx = Some(target_rung_idx.min(self.program.rungs.len().saturating_sub(1)));
        self.selected_element_id = target_elem_id;
        self.insert_existing_element(elem, pos);
        self.selected_element_id = Some(dragged_id);
        self.selected_element_ids = vec![dragged_id];
        self.is_dirty = true;
        true
    }

    /// Move an existing element to a newly created rung (either at `insert_at_rung_idx` or appended).
    pub fn move_element_to_new_rung(&mut self, dragged_id: u64, insert_at_rung_idx: Option<usize>) -> bool {
        self.push_undo_snapshot();
        let elem = match self.take_element_by_id(dragged_id) {
            Some(el) => el,
            None => {
                self.undo_stack.pop();
                return false;
            }
        };
        let new_idx = match insert_at_rung_idx {
            Some(idx) => self.program.insert_rung(idx),
            None => self.program.add_rung(),
        };
        self.selected_rung_idx = Some(new_idx);
        self.selected_element_id = None;
        self.selected_element_ids.clear();
        self.insert_existing_element(elem, InsertPosition::AppendRung);
        self.selected_element_id = Some(dragged_id);
        self.selected_element_ids = vec![dragged_id];
        self.is_dirty = true;
        true
    }

    /// Insert a new element into a newly created rung (either at `insert_at_rung_idx` or appended).
    pub fn insert_element_to_new_rung(&mut self, mut elem: LadderElement, insert_at_rung_idx: Option<usize>) -> usize {
        self.push_undo_snapshot();
        if elem.id == 0 {
            elem.id = self.program.alloc_id();
        }
        for path in &mut elem.branch_paths {
            for sub in path {
                if sub.id == 0 {
                    sub.id = self.program.alloc_id();
                }
            }
        }
        let new_idx = match insert_at_rung_idx {
            Some(idx) => self.program.insert_rung(idx),
            None => self.program.add_rung(),
        };
        self.selected_rung_idx = Some(new_idx);
        self.selected_element_id = None;
        self.selected_element_ids.clear();
        self.insert_existing_element(elem, InsertPosition::AppendRung);
        new_idx
    }

    pub fn insert_element(&mut self, mut elem: LadderElement, pos: InsertPosition) {
        self.push_undo_snapshot();
        if elem.id == 0 {
            elem.id = self.program.alloc_id();
        }
        for path in &mut elem.branch_paths {
            for sub in path {
                if sub.id == 0 {
                    sub.id = self.program.alloc_id();
                }
            }
        }
        self.insert_existing_element(elem, pos);
    }

    pub fn insert_parallel_to_selection(&mut self, mut elem: LadderElement, top: bool) {
        self.push_undo_snapshot();
        if elem.id == 0 {
            elem.id = self.program.alloc_id();
        }
        for path in &mut elem.branch_paths {
            for sub in path {
                if sub.id == 0 {
                    sub.id = self.program.alloc_id();
                }
            }
        }
        let pos = if top {
            InsertPosition::TopParallel
        } else {
            InsertPosition::BottomParallel
        };
        self.insert_existing_element(elem, pos);
    }

    pub fn insert_series_to_selection(&mut self, mut elem: LadderElement, after: bool) {
        self.push_undo_snapshot();
        if elem.id == 0 {
            elem.id = self.program.alloc_id();
        }
        for path in &mut elem.branch_paths {
            for sub in path {
                if sub.id == 0 {
                    sub.id = self.program.alloc_id();
                }
            }
        }
        let pos = if after {
            InsertPosition::Right
        } else {
            InsertPosition::Left
        };
        self.insert_existing_element(elem, pos);
    }

    fn insert_existing_element(&mut self, mut elem: LadderElement, pos: InsertPosition) {
        // Resolve symbol from Global Symbol Table (Single Source of Truth) if present
        if let Some(sym) = self.program.resolve_symbol(&elem.name) {
            if !sym.is_empty() {
                elem.symbol = sym.to_string();
            }
        }

        let elem_id = elem.id;
        let branch_alloc_id = self.program.alloc_id();

        // Hybrid Left-Rail Anchor: A Target Label is always anchored at index 0 (Left Power Rail)
        if elem.element_type == TypeTag::Label {
            let target_rung_idx = self.selected_rung_idx.unwrap_or(0).min(self.program.rungs.len().saturating_sub(1));
            if let Some(rung) = self.program.rungs.get_mut(target_rung_idx) {
                rung.elements.retain(|e| e.element_type != TypeTag::Label);
                rung.elements.insert(0, elem);
                self.selected_rung_idx = Some(target_rung_idx);
                self.selected_element_id = Some(elem_id);
                self.selected_element_ids = vec![elem_id];
                self.is_dirty = true;
                return;
            }
        }

        let mut sel_ids = self.selected_element_ids.clone();
        if sel_ids.is_empty() {
            if let Some(tid) = self.selected_element_id {
                sel_ids.push(tid);
            }
        }

        if !sel_ids.is_empty() && pos != InsertPosition::AppendRung {
            for (r_idx, rung) in self.program.rungs.iter_mut().enumerate() {
                // Find all matching element indices in this rung
                let matching_indices: Vec<usize> = rung
                    .elements
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| sel_ids.contains(&e.id))
                    .map(|(i, _)| i)
                    .collect();

                if !matching_indices.is_empty() {
                    let min_idx = *matching_indices.iter().min().unwrap();
                    let max_idx = *matching_indices.iter().max().unwrap();

                    match pos {
                        InsertPosition::Left => {
                            rung.elements.insert(min_idx, elem);
                        }
                        InsertPosition::Right => {
                            let next_pos = (max_idx + 1).min(rung.elements.len());
                            rung.elements.insert(next_pos, elem);
                        }
                        InsertPosition::TopParallel => {
                            if matching_indices.len() == 1
                                && rung.elements[min_idx].element_type == TypeTag::Branch
                            {
                                rung.elements[min_idx].branch_paths.insert(0, vec![elem]);
                            } else {
                                let group: Vec<LadderElement> =
                                    rung.elements.drain(min_idx..=max_idx).collect();
                                let branch = LadderElement::new_branch(
                                    branch_alloc_id,
                                    vec![vec![elem], group],
                                );
                                rung.elements.insert(min_idx, branch);
                            }
                        }
                        InsertPosition::BottomParallel => {
                            if matching_indices.len() == 1
                                && rung.elements[min_idx].element_type == TypeTag::Branch
                            {
                                rung.elements[min_idx].branch_paths.push(vec![elem]);
                            } else {
                                let group: Vec<LadderElement> =
                                    rung.elements.drain(min_idx..=max_idx).collect();
                                let branch = LadderElement::new_branch(
                                    branch_alloc_id,
                                    vec![group, vec![elem]],
                                );
                                rung.elements.insert(min_idx, branch);
                            }
                        }
                        InsertPosition::AppendRung => {}
                    }
                    self.selected_rung_idx = Some(r_idx);
                    self.selected_element_id = Some(elem_id);
                    self.selected_element_ids = vec![elem_id];
                    self.is_dirty = true;
                    return;
                }

                // Check inside parallel branch paths
                for el in &mut rung.elements {
                    if el.element_type == TypeTag::Branch {
                        for p_idx in 0..el.branch_paths.len() {
                            let matching_sub: Vec<usize> = el.branch_paths[p_idx]
                                .iter()
                                .enumerate()
                                .filter(|(_, s)| sel_ids.contains(&s.id))
                                .map(|(i, _)| i)
                                .collect();

                            if !matching_sub.is_empty() {
                                let min_sub = *matching_sub.iter().min().unwrap();
                                let max_sub = *matching_sub.iter().max().unwrap();

                                match pos {
                                    InsertPosition::Left => {
                                        el.branch_paths[p_idx].insert(min_sub, elem);
                                    }
                                    InsertPosition::Right | InsertPosition::AppendRung => {
                                        let next_pos =
                                            (max_sub + 1).min(el.branch_paths[p_idx].len());
                                        el.branch_paths[p_idx].insert(next_pos, elem);
                                    }
                                    InsertPosition::TopParallel => {
                                        if min_sub == 0
                                            && max_sub == el.branch_paths[p_idx].len() - 1
                                        {
                                            el.branch_paths.insert(p_idx, vec![elem]);
                                        } else {
                                            let sub_group: Vec<LadderElement> =
                                                el.branch_paths[p_idx]
                                                    .drain(min_sub..=max_sub)
                                                    .collect();
                                            let nested = LadderElement::new_branch(
                                                branch_alloc_id,
                                                vec![vec![elem], sub_group],
                                            );
                                            el.branch_paths[p_idx].insert(min_sub, nested);
                                        }
                                    }
                                    InsertPosition::BottomParallel => {
                                        if min_sub == 0
                                            && max_sub == el.branch_paths[p_idx].len() - 1
                                        {
                                            el.branch_paths.insert(p_idx + 1, vec![elem]);
                                        } else {
                                            let sub_group: Vec<LadderElement> =
                                                el.branch_paths[p_idx]
                                                    .drain(min_sub..=max_sub)
                                                    .collect();
                                            let nested = LadderElement::new_branch(
                                                branch_alloc_id,
                                                vec![sub_group, vec![elem]],
                                            );
                                            el.branch_paths[p_idx].insert(min_sub, nested);
                                        }
                                    }
                                }
                                self.selected_rung_idx = Some(r_idx);
                                self.selected_element_id = Some(elem_id);
                                self.selected_element_ids = vec![elem_id];
                                self.is_dirty = true;
                                return;
                            }
                        }
                    }
                }
            }
        }

        // Default: insert on selected rung (before right-aligned coil if not right-aligned itself)
        let is_right = elem.element_type.is_right_aligned();
        if let Some(rung) = self.selected_rung_mut() {
            if !is_right {
                if let Some(coil_pos) = rung
                    .elements
                    .iter()
                    .position(|e| e.element_type.is_right_aligned())
                {
                    rung.elements.insert(coil_pos, elem);
                } else {
                    rung.elements.push(elem);
                }
            } else {
                rung.elements.push(elem);
            }
            self.selected_element_id = Some(elem_id);
            self.selected_element_ids = vec![elem_id];
            self.is_dirty = true;
        }
    }

    pub fn add_element_to_selected_rung(&mut self, elem: LadderElement) {
        self.insert_element(elem, InsertPosition::AppendRung);
    }

    pub fn load_from_il_str(&mut self, content: &str) -> Result<(), String> {
        let mut prog = IlParser::parse(content)?;
        prog.sync_symbols_from_elements_and_io();
        self.push_undo_snapshot();
        self.program = prog;
        self.simulator.reset(&mut self.program);
        self.selected_rung_idx = Some(0);
        self.selected_element_id = None;
        self.selected_element_ids.clear();
        self.is_dirty = false;
        Ok(())
    }

    pub fn export_to_il_str(&self) -> String {
        IlParser::export_il(&self.program)
    }

    pub fn load_from_ld_str(&mut self, content: &str) -> Result<(), String> {
        self.load_from_il_str(content)
    }

    pub fn export_to_ld_str(&self) -> String {
        self.export_to_il_str()
    }

    pub fn generate_il_code(&self) -> String {
        IlParser::generate_il_code(&self.program, self.selected_rung_idx)
    }

    pub fn generate_il_code_online(&self) -> String {
        IlParser::generate_il_code_online(&self.program, self.selected_rung_idx)
    }

    pub fn generate_plclib_code(&self) -> String {
        self.generate_il_code()
    }

    pub fn map_io_to_iec_var_table(&self) -> String {
        self.program.map_io_to_iec_var_table()
    }

    pub fn map_io_to_iec_var_table_with_options(&self, use_symbol_name: bool) -> String {
        self.program.map_io_to_iec_var_table_with_options(use_symbol_name)
    }

    /// Compile the current LadderProgram into 32-bit Micro-VM Bytecode
    pub fn compile_to_bytecode(&self) -> crate::bytecode::BytecodeProgram {
        crate::bytecode::BytecodeCompiler::compile_program(&self.program)
    }

    /// Compile an IEC 61131-3 IL string into 32-bit Micro-VM Bytecode
    pub fn compile_il_to_bytecode(il_text: &str) -> Result<crate::bytecode::BytecodeProgram, String> {
        crate::bytecode::BytecodeCompiler::compile_il(il_text)
    }

    // ------------------------------------------------------------------
    // Clipboard (element-level Copy / Cut / Paste, network paste)
    // ------------------------------------------------------------------

    /// Largest element id currently used anywhere in the program (0 if none).
    fn max_element_id(&self) -> u64 {
        fn walk(el: &LadderElement, max: &mut u64) {
            *max = (*max).max(el.id);
            for path in &el.branch_paths {
                for sub in path {
                    walk(sub, max);
                }
            }
        }
        let mut max = 0;
        for rung in &self.program.rungs {
            for el in &rung.elements {
                walk(el, &mut max);
            }
        }
        max
    }

    /// Make sure `next_element_id` is above every id in use, so `alloc_id` never hands out duplicates.
    fn sync_next_element_id(&mut self) {
        let min_next = self.max_element_id() + 1;
        if self.program.next_element_id < min_next {
            self.program.next_element_id = min_next;
        }
    }

    /// Give `el` (and all nested branch elements) brand-new unique ids and clear runtime state.
    fn assign_fresh_ids(&mut self, el: &mut LadderElement) {
        el.id = self.program.alloc_id();
        el.is_energized = false;
        el.current_accum = 0;
        el.is_done = false;
        for path in &mut el.branch_paths {
            for sub in path {
                self.assign_fresh_ids(sub);
            }
        }
    }

    /// All currently selected element ids (multi-selection plus primary selection).
    pub fn selected_ids(&self) -> Vec<u64> {
        let mut ids = self.selected_element_ids.clone();
        if let Some(id) = self.selected_element_id {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        ids
    }

    /// Clone the selected elements in program order (rung by rung, left to right).
    pub fn copy_selected(&self) -> Vec<LadderElement> {
        fn walk(el: &LadderElement, ids: &[u64], out: &mut Vec<LadderElement>) {
            if ids.contains(&el.id) {
                out.push(el.clone());
                return;
            }
            for path in &el.branch_paths {
                for sub in path {
                    walk(sub, ids, out);
                }
            }
        }
        let ids = self.selected_ids();
        let mut out = Vec::new();
        if ids.is_empty() {
            return out;
        }
        for rung in &self.program.rungs {
            for el in &rung.elements {
                walk(el, &ids, &mut out);
            }
        }
        out
    }

    /// Copy the selection and then delete it (one undo step). Returns the cut elements.
    pub fn cut_selected(&mut self) -> Vec<LadderElement> {
        let els = self.copy_selected();
        if !els.is_empty() {
            self.delete_selected_element();
        }
        els
    }

    /// Paste clones of `els` relative to the current selection (or appended to the selected rung).
    ///
    /// Every pasted element (including nested branch elements) gets a fresh unique id.
    /// The first element is placed at `pos`; the following ones are chained in series after it.
    /// Pushes exactly one undo snapshot. Returns the new top-level element ids (also selected).
    pub fn paste_elements(&mut self, els: &[LadderElement], pos: InsertPosition) -> Vec<u64> {
        if els.is_empty() {
            return Vec::new();
        }
        self.push_undo_snapshot();
        self.sync_next_element_id();
        if self.program.rungs.is_empty() {
            let idx = self.program.add_rung();
            self.selected_rung_idx = Some(idx);
        }
        if self.selected_rung_idx.map_or(true, |i| i >= self.program.rungs.len()) {
            self.selected_rung_idx = Some(self.program.rungs.len() - 1);
        }
        let mut new_ids = Vec::with_capacity(els.len());
        for (i, src) in els.iter().enumerate() {
            let mut el = src.clone();
            self.assign_fresh_ids(&mut el);
            new_ids.push(el.id);
            let this_pos = if i == 0 || pos == InsertPosition::AppendRung {
                pos
            } else {
                InsertPosition::Right
            };
            self.insert_existing_element(el, this_pos);
        }
        self.selected_element_id = new_ids.first().copied();
        self.selected_element_ids = new_ids.clone();
        self.is_dirty = true;
        new_ids
    }

    /// Insert a clone of `rung` (e.g. from a "Copy Network" clipboard) at `insert_idx`,
    /// with fresh element ids. Pushes one undo snapshot and selects the new rung.
    pub fn paste_rung(&mut self, rung: &Rung, insert_idx: usize) -> usize {
        self.push_undo_snapshot();
        self.sync_next_element_id();
        let mut cloned = rung.clone();
        cloned.is_energized = false;
        for el in &mut cloned.elements {
            self.assign_fresh_ids(el);
        }
        let target = insert_idx.min(self.program.rungs.len());
        self.program.rungs.insert(target, cloned);
        self.program.reindex_rungs();
        self.selected_rung_idx = Some(target);
        self.clear_element_selection();
        self.is_dirty = true;
        target
    }

    // ------------------------------------------------------------------
    // Find / Replace
    // ------------------------------------------------------------------

    /// Search all elements' `name / symbol / comment / target_reg / fb.in1 / fb.in2`.
    pub fn find_text(&self, query: &str, opts: FindOptions) -> Vec<FindHit> {
        fn walk(el: &LadderElement, r_idx: usize, query: &str, opts: FindOptions, out: &mut Vec<FindHit>) {
            if el.element_type != TypeTag::Branch {
                for (field, text) in [
                    (FindField::Name, &el.name),
                    (FindField::Symbol, &el.symbol),
                    (FindField::Comment, &el.comment),
                    (FindField::TargetReg, &el.target_reg),
                    (FindField::FbIn1, &el.fb.in1),
                    (FindField::FbIn2, &el.fb.in2),
                ] {
                    if !find_matches(text, query, opts).is_empty() {
                        out.push(FindHit {
                            rung_idx: r_idx,
                            elem_id: el.id,
                            element_type: el.element_type,
                            field,
                            text: text.clone(),
                        });
                    }
                }
            }
            for path in &el.branch_paths {
                for sub in path {
                    walk(sub, r_idx, query, opts, out);
                }
            }
        }
        let mut out = Vec::new();
        if query.is_empty() {
            return out;
        }
        for (r_idx, rung) in self.program.rungs.iter().enumerate() {
            for el in &rung.elements {
                walk(el, r_idx, query, opts, &mut out);
            }
        }
        out
    }

    /// Replace every occurrence of `query` with `replacement` in the searchable fields.
    /// Pushes a single undo snapshot (only if something changed). Returns the number of replacements.
    pub fn replace_all(&mut self, query: &str, replacement: &str, opts: FindOptions) -> usize {
        fn replace_in(text: &mut String, query: &str, replacement: &str, opts: FindOptions) -> usize {
            let matches = find_matches(text, query, opts);
            if matches.is_empty() {
                return 0;
            }
            let mut out = String::with_capacity(text.len());
            let mut last = 0;
            for &(start, end) in &matches {
                out.push_str(&text[last..start]);
                out.push_str(replacement);
                last = end;
            }
            out.push_str(&text[last..]);
            *text = out;
            matches.len()
        }
        fn walk(el: &mut LadderElement, query: &str, replacement: &str, opts: FindOptions) -> usize {
            let mut n = 0;
            if el.element_type != TypeTag::Branch {
                n += replace_in(&mut el.name, query, replacement, opts);
                n += replace_in(&mut el.symbol, query, replacement, opts);
                n += replace_in(&mut el.comment, query, replacement, opts);
                n += replace_in(&mut el.target_reg, query, replacement, opts);
                n += replace_in(&mut el.fb.in1, query, replacement, opts);
                n += replace_in(&mut el.fb.in2, query, replacement, opts);
            }
            for path in &mut el.branch_paths {
                for sub in path {
                    n += walk(sub, query, replacement, opts);
                }
            }
            n
        }
        if query.is_empty() || self.find_text(query, opts).is_empty() {
            return 0;
        }
        self.push_undo_snapshot();
        let mut total = 0;
        for rung in &mut self.program.rungs {
            for el in &mut rung.elements {
                total += walk(el, query, replacement, opts);
            }
        }
        if total > 0 {
            self.is_dirty = true;
        }
        total
    }
}

/// Options for [`Document::find_text`] / [`Document::replace_all`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FindOptions {
    pub match_case: bool,
    pub whole_word: bool,
}

/// Which element field a [`FindHit`] matched in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindField {
    Name,
    Symbol,
    Comment,
    TargetReg,
    FbIn1,
    FbIn2,
}

impl FindField {
    pub fn label(self) -> &'static str {
        match self {
            FindField::Name => "name",
            FindField::Symbol => "symbol",
            FindField::Comment => "comment",
            FindField::TargetReg => "target_reg",
            FindField::FbIn1 => "fb.in1",
            FindField::FbIn2 => "fb.in2",
        }
    }
}

/// A single Find result.
#[derive(Debug, Clone, PartialEq)]
pub struct FindHit {
    pub rung_idx: usize,
    pub elem_id: u64,
    pub element_type: TypeTag,
    pub field: FindField,
    pub text: String,
}

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Non-overlapping byte ranges of `needle` in `hay` honouring [`FindOptions`].
/// Case-insensitive matching is ASCII-only (keeps byte offsets valid for replacement).
pub fn find_matches(hay: &str, needle: &str, opts: FindOptions) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    if needle.is_empty() || needle.len() > hay.len() {
        return out;
    }
    let (h, n): (std::borrow::Cow<str>, std::borrow::Cow<str>) = if opts.match_case {
        (hay.into(), needle.into())
    } else {
        (hay.to_ascii_lowercase().into(), needle.to_ascii_lowercase().into())
    };
    let hb = h.as_bytes();
    let mut from = 0;
    while let Some(rel) = h[from..].find(n.as_ref()) {
        let start = from + rel;
        let end = start + n.len();
        let ok = !opts.whole_word
            || ((start == 0 || !is_word_byte(hb[start - 1])) && (end == hb.len() || !is_word_byte(hb[end])));
        if ok {
            out.push((start, end));
            from = end;
        } else {
            // advance by one char to keep slicing on a char boundary
            from = start + h[start..].chars().next().map_or(1, |c| c.len_utf8());
        }
        if from >= h.len() {
            break;
        }
    }
    out
}

#[cfg(test)]
mod clipboard_find_tests {
    use super::*;
    use crate::ladder::{CoilMode, ContactMode};

    fn sample_doc() -> Document {
        let mut doc = Document::new_empty();
        let a = doc.program.alloc_id();
        let b = doc.program.alloc_id();
        doc.program.rungs[0].elements.push(LadderElement::new_contact(a, "%IX0.0", "START", ContactMode::NormallyOpen));
        doc.program.rungs[0].elements.push(LadderElement::new_coil(b, "%QX0.0", "MOTOR", CoilMode::Normal));
        doc
    }

    fn all_ids(doc: &Document) -> Vec<u64> {
        fn walk(el: &LadderElement, out: &mut Vec<u64>) {
            out.push(el.id);
            for p in &el.branch_paths {
                for s in p {
                    walk(s, out);
                }
            }
        }
        let mut v = Vec::new();
        for r in &doc.program.rungs {
            for e in &r.elements {
                walk(e, &mut v);
            }
        }
        v
    }

    #[test]
    fn find_matches_options() {
        let cs = FindOptions { match_case: true, whole_word: false };
        let ci = FindOptions::default();
        let ww = FindOptions { match_case: false, whole_word: true };
        assert_eq!(find_matches("%MW3 %mw30", "%MW3", cs), vec![(0, 4)]);
        assert_eq!(find_matches("%MW3 %mw30", "%MW3", ci), vec![(0, 4), (5, 9)]);
        assert_eq!(find_matches("%MW3 %mw30", "%MW3", ww), vec![(0, 4)]);
        assert!(find_matches("abc", "", ci).is_empty());
    }

    #[test]
    fn copy_paste_assigns_unique_ids_and_single_undo() {
        let mut doc = sample_doc();
        let first = doc.program.rungs[0].elements[0].id;
        doc.set_single_selection(Some(0), first);
        let clip = doc.copy_selected();
        assert_eq!(clip.len(), 1);
        let undo_before = doc.undo_stack.len();
        let new_ids = doc.paste_elements(&clip, InsertPosition::Right);
        assert_eq!(new_ids.len(), 1);
        assert_eq!(doc.undo_stack.len(), undo_before + 1);
        let ids = all_ids(&doc);
        let mut dedup = ids.clone();
        dedup.sort();
        dedup.dedup();
        assert_eq!(ids.len(), dedup.len(), "ids must be unique: {:?}", ids);
        assert_eq!(doc.program.rungs[0].elements.len(), 3);
        // A subsequently allocated id must not collide either
        let next = doc.program.alloc_id();
        assert!(!ids.contains(&next));
        assert!(doc.undo());
        assert_eq!(doc.program.rungs[0].elements.len(), 2);
    }

    #[test]
    fn paste_rung_fresh_ids() {
        let mut doc = sample_doc();
        let rung = doc.program.rungs[0].clone();
        doc.program.rungs[0].elements.clear(); // rung clone survives source edits
        let idx = doc.paste_rung(&rung, 1);
        assert_eq!(idx, 1);
        assert_eq!(doc.program.rungs[1].elements.len(), 2);
        let new_ids: Vec<u64> = doc.program.rungs[1].elements.iter().map(|e| e.id).collect();
        for e in &rung.elements {
            assert!(!new_ids.contains(&e.id));
        }
        // Pasting the same network twice still yields unique ids
        doc.paste_rung(&rung, 2);
        let ids = all_ids(&doc);
        let mut dedup = ids.clone();
        dedup.sort();
        dedup.dedup();
        assert_eq!(ids.len(), dedup.len());
    }

    #[test]
    fn find_and_replace_all_single_undo() {
        let mut doc = sample_doc();
        let hits = doc.find_text("motor", FindOptions::default());
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].field, FindField::Symbol);
        assert!(doc.find_text("motor", FindOptions { match_case: true, whole_word: false }).is_empty());
        let undo_before = doc.undo_stack.len();
        let n = doc.replace_all("%IX0.0", "%IX0.5", FindOptions { match_case: false, whole_word: true });
        assert_eq!(n, 1);
        assert_eq!(doc.undo_stack.len(), undo_before + 1);
        assert_eq!(doc.program.rungs[0].elements[0].name, "%IX0.5");
        assert_eq!(doc.replace_all("nothing", "x", FindOptions::default()), 0);
        assert_eq!(doc.undo_stack.len(), undo_before + 1);
        doc.undo();
        assert_eq!(doc.program.rungs[0].elements[0].name, "%IX0.0");
    }
}

