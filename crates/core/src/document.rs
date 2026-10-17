use serde::{Deserialize, Serialize};
use crate::ladder::{
    CoilMode, CoilStyle, ContactMode, ContactStyle, InsertPosition, LadderElement, LadderProgram,
    Rung, SchematicStyle, TypeTag,
};
use crate::il_format::IlParser;
use crate::simulation::PlcSimulator;

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
        let mut usages = Vec::new();
        for (r_idx, rung) in self.program.rungs.iter().enumerate() {
            for el in &rung.elements {
                if el.element_type == TypeTag::Branch {
                    for path in &el.branch_paths {
                        for sub in path {
                            if sub.name.eq_ignore_ascii_case(addr) {
                                usages.push((r_idx, sub.id, sub.element_type));
                            }
                        }
                    }
                } else if el.name.eq_ignore_ascii_case(addr) {
                    usages.push((r_idx, el.id, el.element_type));
                }
            }
        }
        usages
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

    fn insert_existing_element(&mut self, elem: LadderElement, pos: InsertPosition) {
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
        let prog = IlParser::parse(content)?;
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
}

