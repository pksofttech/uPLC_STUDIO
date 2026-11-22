use egui::{
    pos2, vec2, Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Sense, Stroke,
    StrokeKind, Ui, Vec2,
};
use my_canvas_core::{
    branch_contains_any_coil, is_parallel_output_branch, validate_program, CoilMode, CoilStyle,
    ContactMode, ContactStyle, Document, InsertPosition, LadderElement, ProgramValidationReport,
    Rung, TypeTag, ValidationSeverity,
};
use crate::icons::{paint_icon, VectorIcon};
use crate::theme::ThemePalette;
use crate::tools::ActiveTool;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScrollbarDrag {
    Vertical { start_mouse_y: f32, start_pan_y: f32 },
    Horizontal { start_mouse_x: f32, start_pan_x: f32 },
}

#[derive(Clone, Debug)]
pub enum CanvasDebugEvent {
    ForceBit { addr: String, val: bool },
    UnforceBit { addr: String },
    ClearAllForces,
    SwitchToEditMode { open_elem_id: Option<u64> },
}

pub struct CanvasViewport {
    pub pan_offset: Vec2,
    pub zoom: f32,
    pub target_zoom: f32,
    pub zoom_anchor: Option<Pos2>,
    pub context_menu_elem: Option<(usize, u64)>,
    pub context_menu_pos: Option<Pos2>,
    pub drag_candidate: Option<(u64, String, Pos2)>,
    pub dragging_elem: Option<(u64, String)>,
    pub dragging_tool: Option<(ActiveTool, String)>,
    pub marquee_start: Option<Pos2>,
    pub dragging_scrollbar: Option<ScrollbarDrag>,
    pub is_edit_mode: bool,
    pub is_online_hw_mode: bool,
    pub pending_debug_event: Option<CanvasDebugEvent>,
    pub show_diagnostics_popover: bool,
    pub rejection_toast: Option<(String, f64)>,
}

impl Default for CanvasViewport {
    fn default() -> Self {
        Self {
            pan_offset: vec2(14.0, 16.0),
            zoom: 1.0,
            target_zoom: 1.0,
            zoom_anchor: None,
            context_menu_elem: None,
            context_menu_pos: None,
            drag_candidate: None,
            dragging_elem: None,
            dragging_tool: None,
            marquee_start: None,
            dragging_scrollbar: None,
            is_edit_mode: true,
            is_online_hw_mode: false,
            pending_debug_event: None,
            show_diagnostics_popover: false,
            rejection_toast: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PlacementViolation {
    rule_id: &'static str,
    message: &'static str,
    suggestion: &'static str,
}

impl PlacementViolation {
    fn formatted(&self) -> String {
        format!("[{}] {}", self.rule_id, self.message)
    }
}

#[derive(Clone, Debug)]
struct DropTarget {
    rung_idx: usize,
    target_elem_id: Option<u64>,
    insert_pos: InsertPosition,
    target_label: String,
    validation_rule_id: Option<&'static str>,
    validation_error: Option<String>,
    create_new_rung: bool,
}

#[derive(Clone, Copy, Debug, Default)]
struct PlacementContext {
    target_is_first: bool,
    target_is_last: bool,
    in_branch: bool,
    has_downstream_after_branch: bool,
}

fn find_placement_context(rung: &Rung, elem_id: u64) -> PlacementContext {
    fn walk(
        elements: &[LadderElement],
        elem_id: u64,
        in_branch: bool,
        has_downstream_after_branch: bool,
    ) -> Option<PlacementContext> {
        let last_idx = elements.len().saturating_sub(1);
        for (idx, el) in elements.iter().enumerate() {
            let is_first = idx == 0;
            let is_last = idx == last_idx;
            if el.id == elem_id {
                return Some(PlacementContext {
                    target_is_first: is_first,
                    target_is_last: is_last,
                    in_branch,
                    has_downstream_after_branch,
                });
            }
            if el.element_type == TypeTag::Branch {
                let child_downstream = has_downstream_after_branch || !is_last;
                for path in &el.branch_paths {
                    if let Some(found) = walk(path, elem_id, true, child_downstream) {
                        return Some(found);
                    }
                }
            }
        }
        None
    }

    walk(&rung.elements, elem_id, false, false).unwrap_or_default()
}

fn validate_placement(
    source_tag: TypeTag,
    target_elem: &LadderElement,
    insert_pos: InsertPosition,
    ctx: PlacementContext,
) -> Result<InsertPosition, PlacementViolation> {
    // R01: Label [LBL] must only be placed at the beginning of the network
    if source_tag == TypeTag::Label {
        if insert_pos == InsertPosition::TopParallel || insert_pos == InsertPosition::BottomParallel {
            return Err(PlacementViolation {
                rule_id: "R01",
                message: "LBL cannot be placed in a parallel branch",
                suggestion: "Place LBL at the left rail at the start of the network",
            });
        }
        if ctx.in_branch || !ctx.target_is_first || insert_pos != InsertPosition::Left {
            return Err(PlacementViolation {
                rule_id: "R01",
                message: "LBL must be placed at the left rail (start of network)",
                suggestion: "Drop LBL to the left of the first element",
            });
        }
    }

    // R01: Cannot place any components after Jump [JMP] or Return [RET] (Dead code)
    if matches!(target_elem.element_type, TypeTag::Jump | TypeTag::Return)
        && insert_pos == InsertPosition::Right
    {
        return Err(PlacementViolation {
            rule_id: "R01",
            message: "Cannot place elements after JMP/RET instruction",
            suggestion: "Place elements before JMP/RET or in another network",
        });
    }

    // R07 / R08 / R09 / R10: Strict rules around Output Coils
    if target_elem.element_type == TypeTag::Coil {
        if insert_pos == InsertPosition::Right {
            if source_tag == TypeTag::Coil {
                return Err(PlacementViolation {
                    rule_id: "R08",
                    message: "Coils cannot be placed in series",
                    suggestion: "Drop on Top/Bottom Parallel of the Coil instead",
                });
            } else if source_tag == TypeTag::Branch {
                return Err(PlacementViolation {
                    rule_id: "R10",
                    message: "Cannot branch from the output side of a Coil",
                    suggestion: "Move the branch point before the Coil",
                });
            } else {
                return Err(PlacementViolation {
                    rule_id: "R09",
                    message: "Coil terminates the branch; cannot place elements after a Coil",
                    suggestion: "Move input/logic elements before the Coil",
                });
            }
        }
        if insert_pos == InsertPosition::Left && source_tag == TypeTag::Coil {
            return Err(PlacementViolation {
                rule_id: "R08",
                message: "Coils cannot be placed in series; use Parallel Above/Below",
                suggestion: "Drop on Top/Bottom Parallel of the Coil instead",
            });
        }
        if (insert_pos == InsertPosition::TopParallel || insert_pos == InsertPosition::BottomParallel)
            && source_tag == TypeTag::Contact
        {
            return Err(PlacementViolation {
                rule_id: "R07",
                message: "Cannot parallel an input contact across an output coil",
                suggestion: "Place input contacts before the output coil",
            });
        }
        if (insert_pos == InsertPosition::TopParallel || insert_pos == InsertPosition::BottomParallel)
            && source_tag == TypeTag::Coil
        {
            if ctx.in_branch && ctx.has_downstream_after_branch {
                return Err(PlacementViolation {
                    rule_id: "R21",
                    message: "Coil cannot be placed inside a branch before a merge with downstream elements",
                    suggestion: "Place the Coil at the end of the network or in a terminal parallel output branch",
                });
            }
            return Ok(insert_pos);
        }
    }

    // R21: Forbid placing a Coil inside a branch before a merge with downstream elements
    if source_tag == TypeTag::Coil && ctx.in_branch && ctx.has_downstream_after_branch {
        return Err(PlacementViolation {
            rule_id: "R21",
            message: "Coil cannot be placed inside a branch before a merge with downstream elements",
            suggestion: "Place the Coil at the end of the network or in a terminal parallel output branch",
        });
    }

    // R06 / R07: Cannot parallel an output coil across an input contact, or place outputs before contacts
    if target_elem.element_type == TypeTag::Contact && source_tag == TypeTag::Coil {
        if insert_pos == InsertPosition::TopParallel || insert_pos == InsertPosition::BottomParallel {
            return Err(PlacementViolation {
                rule_id: "R07",
                message: "Cannot parallel an output coil across an input contact",
                suggestion: "Place output coils at the end of the network",
            });
        }
        if insert_pos == InsertPosition::Left {
            return Err(PlacementViolation {
                rule_id: "R06",
                message: "Cannot place an output coil before an input contact",
                suggestion: "Place output coils at the end of the network",
            });
        }
    }
    if target_elem.element_type == TypeTag::Contact
        && source_tag != TypeTag::Coil
        && source_tag.is_right_aligned()
        && insert_pos == InsertPosition::Left
    {
        return Err(PlacementViolation {
            rule_id: "R06",
            message: "Cannot place an output instruction before an input contact",
            suggestion: "Place output instructions after input contacts",
        });
    }
    if target_elem.element_type != TypeTag::Coil
        && target_elem.element_type.is_right_aligned()
        && source_tag == TypeTag::Contact
        && insert_pos == InsertPosition::Right
    {
        return Err(PlacementViolation {
            rule_id: "R09",
            message: "Cannot place an input contact after an output instruction",
            suggestion: "Move input contacts before output instructions",
        });
    }

    // R06: Cannot place a coil before a Function Block
    if matches!(
        target_elem.element_type,
        TypeTag::Ton
            | TypeTag::Tof
            | TypeTag::Tp
            | TypeTag::Ctu
            | TypeTag::Ctd
            | TypeTag::Ctud
            | TypeTag::Compare
            | TypeTag::RTrig
            | TypeTag::FTrig
            | TypeTag::Sr
            | TypeTag::Rs
            | TypeTag::UartRecv
            | TypeTag::InRange
    ) && source_tag == TypeTag::Coil
        && insert_pos == InsertPosition::Left
    {
        return Err(PlacementViolation {
            rule_id: "R06",
            message: "Cannot place an output coil before a function block",
            suggestion: "Place the coil after the function block",
        });
    }

    // R21: Coil must be the terminal element; cannot place Coil before downstream elements
    if source_tag == TypeTag::Coil
        && !ctx.target_is_last
        && matches!(insert_pos, InsertPosition::Left | InsertPosition::Right)
    {
        return Err(PlacementViolation {
            rule_id: "R21",
            message: "Coil must be the terminal element; cannot place Coil before downstream elements",
            suggestion: "Place the Coil at the end of the network or in a terminal parallel output branch",
        });
    }

    Ok(insert_pos)
}

fn validate_rung_append(source_tag: TypeTag, rung: &Rung) -> Result<(), PlacementViolation> {
    if source_tag == TypeTag::Label && !rung.elements.is_empty() {
        return Err(PlacementViolation {
            rule_id: "R01",
            message: "LBL must be placed at the start of an empty network",
            suggestion: "Drop LBL to the left of the first element or onto a new network",
        });
    }
    if let Some(last) = rung.elements.last() {
        if matches!(last.element_type, TypeTag::Jump | TypeTag::Return) {
            return Err(PlacementViolation {
                rule_id: "R01",
                message: "Cannot append elements after JMP/RET instruction",
                suggestion: "Insert before JMP/RET or create a new network",
            });
        }
        if source_tag == TypeTag::Coil
            && (last.element_type == TypeTag::Coil || is_parallel_output_branch(last))
        {
            return Err(PlacementViolation {
                rule_id: "R08",
                message: "Network already ends with a Coil; drop on Top/Bottom Parallel of the Coil instead",
                suggestion: "Use Parallel Above/Below on the existing Coil for R07 parallel outputs",
            });
        }
        if last.element_type == TypeTag::Branch && branch_contains_any_coil(last) {
            return Err(PlacementViolation {
                rule_id: "R21",
                message: "Cannot append elements after a branch containing a Coil",
                suggestion: "Coil must terminate directly at the right rail; add a new network or parallel output branch",
            });
        }
    }
    Ok(())
}

fn draw_dashed_rect(painter: &Painter, rect: Rect, stroke: Stroke, dash: f32, gap: f32) {
    let step = (dash + gap).max(1.0);
    let draw_edge = |p0: Pos2, p1: Pos2| {
        let delta = p1 - p0;
        let len = delta.length();
        if len <= 0.001 {
            return;
        }
        let dir = delta / len;
        let mut t = 0.0;
        while t < len {
            let t_end = (t + dash).min(len);
            painter.line_segment([p0 + dir * t, p0 + dir * t_end], stroke);
            t += step;
        }
    };
    let tl = rect.min;
    let tr = pos2(rect.max.x, rect.min.y);
    let br = rect.max;
    let bl = pos2(rect.min.x, rect.max.y);
    draw_edge(tl, tr);
    draw_edge(tr, br);
    draw_edge(br, bl);
    draw_edge(bl, tl);
}

fn type_tag_icon(tag: TypeTag) -> VectorIcon {
    match tag {
        TypeTag::Contact => VectorIcon::ContactNo,
        TypeTag::Coil => VectorIcon::Coil,
        TypeTag::Ton | TypeTag::Tof | TypeTag::Tp => VectorIcon::Timer,
        TypeTag::Ctu | TypeTag::Ctd | TypeTag::Ctud => VectorIcon::Counter,
        TypeTag::RTrig | TypeTag::FTrig => VectorIcon::PulseEdge,
        TypeTag::MasterRelay => VectorIcon::MasterRelay,
        TypeTag::ShiftRegisters
        | TypeTag::ShiftRegInbit
        | TypeTag::ShiftRegReset
        | TypeTag::ShiftLeft
        | TypeTag::ShiftRight
        | TypeTag::ShiftVariable => VectorIcon::ShiftReg,
        TypeTag::UartSend | TypeTag::UartRecv | TypeTag::MbMaster | TypeTag::MbSlave => {
            VectorIcon::SerialComm
        }
        TypeTag::Comment => VectorIcon::Comment,
        _ => VectorIcon::FunctionBlock,
    }
}

fn ctx_menu_item(
    ui: &mut Ui,
    label: &str,
    shortcut: &str,
    text_color: Color32,
    palette: ThemePalette,
) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::click());
    if resp.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(4), palette.rung_badge_hov_bg);
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    ui.painter().text(
        pos2(rect.min.x + 8.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(11.5),
        text_color,
    );
    if !shortcut.is_empty() {
        ui.painter().text(
            pos2(rect.max.x - 8.0, rect.center().y),
            Align2::RIGHT_CENTER,
            shortcut,
            FontId::monospace(10.0),
            palette.rung_text_dim,
        );
    }
    resp.clicked()
}

fn ctx_menu_item_disabled(
    ui: &mut Ui,
    label: &str,
    violation: PlacementViolation,
    palette: ThemePalette,
) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::hover());
    ui.painter().text(
        pos2(rect.min.x + 8.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(11.5),
        palette.rung_text_dim.gamma_multiply(0.65),
    );
    ui.painter().text(
        pos2(rect.max.x - 8.0, rect.center().y),
        Align2::RIGHT_CENTER,
        format!("[{}]", violation.rule_id),
        FontId::monospace(9.5),
        palette.accent_danger.gamma_multiply(0.85),
    );
    resp.on_hover_text(format!(
        "[{}] {}\nFix: {}",
        violation.rule_id, violation.message, violation.suggestion
    ));
}

fn ctx_menu_sep(ui: &mut Ui, palette: ThemePalette) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 7.0), Sense::hover());
    let y = rect.center().y;
    ui.painter().line_segment(
        [pos2(rect.min.x + 4.0, y), pos2(rect.max.x - 4.0, y)],
        Stroke::new(1.0, palette.card_border.gamma_multiply(0.7)),
    );
}

enum CanvasAction {
    SelectRung(usize),
    SelectElement {
        rung_idx: usize,
        elem_id: u64,
        elem_name: String,
        double_clicked: bool,
        additive: bool,
    },
    OpenConfigModal(u64),
    QuickAppendToRung(usize),
    AddParallelToSelection,
    AddSeriesToSelection(bool),
    ToggleElementMode(u64),
    ToggleSelectedModes,
    DeleteElement(u64),
    DeleteSelectedElements,
    DuplicateRung(usize),
    DeleteRung(usize),
    DebugToggleBit(String),
    DebugForceBit(String, bool),
    DebugUnforceBit(String),
    DebugClearAllForces,
    SwitchToEditMode(Option<u64>),
}

impl CanvasViewport {
    pub fn reset_view(&mut self) {
        self.pan_offset = vec2(14.0, 16.0);
        self.zoom = 1.0;
        self.target_zoom = 1.0;
        self.zoom_anchor = None;
    }

    pub fn fit_to_viewport(&mut self, doc: &Document, viewport_size: Vec2) {
        let world_sz = Self::compute_world_content_size(doc, viewport_size.x);
        let world_w = world_sz.x.max(1.0);
        let world_h = world_sz.y.max(1.0);
        let usable_w = (viewport_size.x - 28.0).max(120.0);
        let usable_h = (viewport_size.y - 32.0).max(120.0);
        let fit_z = (usable_w / world_w).min(usable_h / world_h).clamp(0.45, 1.25);
        self.zoom = fit_z;
        self.target_zoom = fit_z;
        self.zoom_anchor = None;
        self.pan_offset = vec2(14.0, 16.0);
    }

    pub fn set_target_zoom(&mut self, new_zoom: f32, anchor: Option<Pos2>) {
        self.target_zoom = new_zoom.clamp(0.45, 2.25);
        if anchor.is_some() {
            self.zoom_anchor = anchor;
        }
    }

    #[inline]
    fn font_prop(&self, base_size: f32) -> FontId {
        let q = ((base_size * self.zoom) * 2.0).round() * 0.5;
        FontId::proportional(q.max(6.0))
    }

    #[inline]
    fn font_mono(&self, base_size: f32) -> FontId {
        let q = ((base_size * self.zoom) * 2.0).round() * 0.5;
        FontId::monospace(q.max(6.0))
    }

    /// Draw centered monospace text inside `clip_rect`, shrinking font slightly if needed
    /// and truncating with `…` so long tag/parameter strings never overflow the card frame.
    #[allow(clippy::too_many_arguments)]
    fn draw_fitted_card_text(
        &self,
        painter: &Painter,
        clip_rect: Rect,
        center_pos: Pos2,
        text: &str,
        base_size: f32,
        max_w: f32,
        color: Color32,
    ) {
        if text.is_empty() || max_w <= 4.0 {
            return;
        }
        let mut galley = painter.layout_no_wrap(text.to_string(), self.font_mono(base_size), color);
        if galley.size().x > max_w {
            let small_size = (base_size * 0.82).max(6.5);
            let small_font = self.font_mono(small_size);
            galley = painter.layout_no_wrap(text.to_string(), small_font.clone(), color);
            if galley.size().x > max_w {
                let mut chars: Vec<char> = text.chars().collect();
                while chars.len() > 1 {
                    chars.pop();
                    let mut candidate: String = chars.iter().collect();
                    candidate.push('…');
                    let g = painter.layout_no_wrap(candidate, small_font.clone(), color);
                    galley = g;
                    if galley.size().x <= max_w {
                        break;
                    }
                }
            }
        }
        let top_left = pos2(
            center_pos.x - galley.size().x * 0.5,
            center_pos.y - galley.size().y * 0.5,
        );
        painter.with_clip_rect(clip_rect).galley(top_left, galley, color);
    }

    pub fn start_tool_drag(&mut self, tool: ActiveTool, label: impl Into<String>) {
        self.dragging_tool = Some((tool, label.into()));
        self.drag_candidate = None;
        self.marquee_start = None;
    }

    pub fn to_screen(&self, world_pos: Pos2, origin: Pos2) -> Pos2 {
        origin + (world_pos.to_vec2() * self.zoom) + self.pan_offset
    }

    pub fn to_world(&self, screen_pos: Pos2, origin: Pos2) -> Pos2 {
        Pos2::new(
            (screen_pos.x - origin.x - self.pan_offset.x) / self.zoom,
            (screen_pos.y - origin.y - self.pan_offset.y) / self.zoom,
        )
    }

    fn rung_required_cols(elements: &[LadderElement]) -> f32 {
        let mut cols = 0.0_f32;
        let mut has_right_aligned = false;
        for el in elements {
            if el.element_type.is_right_aligned() {
                has_right_aligned = true;
            }
            if el.element_type == TypeTag::Branch {
                let max_path: f32 = el
                    .branch_paths
                    .iter()
                    .map(|p| Self::rung_required_cols(p))
                    .fold(1.0_f32, f32::max);
                cols += max_path + 0.3;
            } else {
                cols += 1.0;
            }
        }
        if has_right_aligned {
            cols += 0.6;
        } else {
            cols += 1.0;
        }
        cols.max(4.0)
    }

    fn element_height(elem: &LadderElement, cell_h: f32) -> f32 {
        if elem.element_type == TypeTag::Branch {
            let total: f32 = elem
                .branch_paths
                .iter()
                .map(|path| {
                    let mut max_h = cell_h;
                    for sub in path {
                        let sub_h = Self::element_height(sub, cell_h);
                        if sub_h > max_h {
                            max_h = sub_h;
                        }
                    }
                    max_h
                })
                .sum();
            total.max(cell_h)
        } else {
            cell_h
        }
    }

    fn element_width(elem: &LadderElement, cell_w: f32) -> f32 {
        if elem.element_type == TypeTag::Branch {
            let pad_x = 14.0;
            let max_path_w = elem
                .branch_paths
                .iter()
                .map(|path| {
                    path.iter()
                        .map(|sub| Self::element_width(sub, cell_w))
                        .sum::<f32>()
                })
                .fold(cell_w, f32::max);
            max_path_w + pad_x * 2.0
        } else {
            cell_w
        }
    }

    fn rung_comment_lines(comment: &str) -> Vec<&str> {
        if comment.is_empty() {
            Vec::new()
        } else {
            comment.split('\n').map(|l| l.trim_end_matches('\r')).collect()
        }
    }

    fn rung_header_offset(rung: &my_canvas_core::Rung) -> f32 {
        let has_label = rung.elements.iter().any(|e| e.element_type == TypeTag::Label);
        let line_count = Self::rung_comment_lines(&rung.comment).len();
        if line_count == 0 && !has_label {
            0.0
        } else {
            let extra_lines = line_count.saturating_sub(1) as f32;
            26.0 + extra_lines * 15.0
        }
    }

    fn compute_world_content_size(doc: &Document, viewport_width: f32) -> Vec2 {
        let cell_h = 88.0;
        let rung_header_w = 76.0;
        let margin_left = 14.0 + rung_header_w;
        let margin_top = 24.0;

        let max_req_cols = doc
            .program
            .rungs
            .iter()
            .map(|r| Self::rung_required_cols(&r.elements))
            .fold(5.0_f32, f32::max);

        let base_world_right = (viewport_width - 60.0).max(520.0);
        let avail_span = (base_world_right - margin_left - 24.0).max(320.0);
        let cell_w = (avail_span / max_req_cols.max(5.5)).clamp(84.0, 108.0);
        let min_required_right = margin_left + 24.0 + max_req_cols * cell_w + 16.0;
        let margin_right = base_world_right.max(min_required_right);

        let mut cur_y = margin_top;
        for rung in &doc.program.rungs {
            let comment_offset = Self::rung_header_offset(rung);
            let rung_start_y = cur_y + comment_offset;
            let mut max_y = rung_start_y + cell_h;
            for elem in &rung.elements {
                let eh = rung_start_y + Self::element_height(elem, cell_h);
                if eh > max_y {
                    max_y = eh;
                }
            }
            cur_y = max_y + 18.0;
        }

        let rail_bottom_y = (cur_y - 8.0).max(margin_top + cell_h);
        let total_world_w = margin_right + 80.0;
        let total_world_h = rail_bottom_y + 60.0;

        vec2(total_world_w, total_world_h)
    }

    pub fn compute_content_size(&self, doc: &Document, viewport_width: f32) -> Vec2 {
        let world_sz = Self::compute_world_content_size(doc, viewport_width);
        vec2(
            (world_sz.x * self.zoom) + self.pan_offset.x.max(0.0),
            (world_sz.y * self.zoom) + self.pan_offset.y.max(0.0),
        )
    }

    /// World-space top Y of network `rung_idx` (same layout rules as [`Self::compute_content_size`]).
    pub fn rung_world_top(doc: &Document, rung_idx: usize) -> f32 {
        let cell_h = 88.0;
        let mut cur_y = 24.0;
        for (i, rung) in doc.program.rungs.iter().enumerate() {
            if i == rung_idx {
                return cur_y;
            }
            let rung_start_y = cur_y + Self::rung_header_offset(rung);
            let mut max_y = rung_start_y + cell_h;
            for elem in &rung.elements {
                max_y = max_y.max(rung_start_y + Self::element_height(elem, cell_h));
            }
            cur_y = max_y + 18.0;
        }
        cur_y
    }

    /// Pan vertically so network `rung_idx` is near the top of the viewport (used by Find).
    pub fn scroll_to_rung(&mut self, doc: &Document, rung_idx: usize) {
        let y = Self::rung_world_top(doc, rung_idx);
        self.pan_offset.y = 16.0 - (y - 24.0) * self.zoom;
    }

    /// Renders the Ladder Canvas and returns `Some(elem_id)` if the user requested opening the Element Configuration Dialog
    pub fn show(
        &mut self,
        ui: &mut Ui,
        doc: &mut Document,
        active_tool: &mut ActiveTool,
        default_insert_pos: InsertPosition,
        palette: ThemePalette,
    ) -> Option<u64> {
        let canvas_enabled = ui.is_enabled();
        if !canvas_enabled {
            self.context_menu_elem = None;
            self.context_menu_pos = None;
            self.dragging_scrollbar = None;
        }
        let sense = if canvas_enabled {
            Sense::click_and_drag()
        } else {
            Sense::hover()
        };
        let (response, painter) =
            ui.allocate_painter(ui.available_size_before_wrap(), sense);
        let origin = response.rect.min;
        let viewport_rect = response.rect;
        let painter = painter.with_clip_rect(viewport_rect);

        // Pan with Middle-Drag, Right-Drag (when not opening menu), or Space+Drag
        if canvas_enabled
            && (response.dragged_by(egui::PointerButton::Middle)
                || (response.dragged_by(egui::PointerButton::Secondary)
                    && response.drag_delta().length() > 2.0)
                || (response.dragged_by(egui::PointerButton::Primary)
                    && (ui.input(|i| i.key_down(egui::Key::Space))
                        || *active_tool == ActiveTool::Pan)))
        {
            self.pan_offset += response.drag_delta();
        }

        // Collect zoom input from Wheel, Touchpad Pinch, or Keyboard (Ctrl + =/-/0)
        let pointer_in_viewport = if canvas_enabled {
            ui.input(|i| i.pointer.hover_pos())
                .filter(|&p| viewport_rect.contains(p))
        } else {
            None
        };

        if let Some(pointer_screen) = pointer_in_viewport {
            let pinch_factor = ui.input(|i| i.zoom_delta());
            let scroll_y = ui.input(|i| i.smooth_scroll_delta.y);

            if (pinch_factor - 1.0).abs() > 0.001 {
                self.set_target_zoom(self.target_zoom * pinch_factor, Some(pointer_screen));
            } else if scroll_y.abs() > 0.01 {
                let zoom_factor = (scroll_y * 0.0018).exp();
                self.set_target_zoom(self.target_zoom * zoom_factor, Some(pointer_screen));
            }
        }

        if canvas_enabled && !ui.ctx().egui_wants_keyboard_input() {
            let zoom_in_key = ui.input(|i| {
                i.modifiers.command
                    && (i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals))
            });
            let zoom_out_key =
                ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Minus));
            let zoom_reset_key =
                ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Num0));

            if zoom_in_key {
                self.set_target_zoom(
                    self.target_zoom * 1.15,
                    Some(pointer_in_viewport.unwrap_or(viewport_rect.center())),
                );
            } else if zoom_out_key {
                self.set_target_zoom(
                    self.target_zoom / 1.15,
                    Some(pointer_in_viewport.unwrap_or(viewport_rect.center())),
                );
            } else if zoom_reset_key {
                self.reset_view();
            }
        }

        // Smooth frame-rate-independent exponential zoom interpolation anchored at cursor
        let zoom_diff = self.target_zoom - self.zoom;
        if zoom_diff.abs() > 0.0008 {
            let anchor_screen = self
                .zoom_anchor
                .or(pointer_in_viewport)
                .unwrap_or(viewport_rect.center());
            let world_before = self.to_world(anchor_screen, origin);
            let dt = ui.input(|i| i.stable_dt).clamp(0.001, 0.05);
            let alpha = (1.0 - (-22.0 * dt).exp()).clamp(0.18, 1.0);
            self.zoom = (self.zoom + zoom_diff * alpha).clamp(0.45, 2.25);
            let screen_after = self.to_screen(world_before, origin);
            self.pan_offset += anchor_screen - screen_after;
            ui.ctx().request_repaint();
        } else if zoom_diff.abs() > 0.0 {
            let anchor_screen = self
                .zoom_anchor
                .or(pointer_in_viewport)
                .unwrap_or(viewport_rect.center());
            let world_before = self.to_world(anchor_screen, origin);
            self.zoom = self.target_zoom;
            let screen_after = self.to_screen(world_before, origin);
            self.pan_offset += anchor_screen - screen_after;
            self.zoom_anchor = None;
        }

        // Theme-aware Background & Grid
        painter.rect_filled(viewport_rect, 0.0, palette.canvas_bg);
        self.draw_grid(&painter, viewport_rect, origin, palette.grid_dot);

        // Rigid World Geometry (independent of self.zoom / self.pan_offset so zooming never shifts layout)
        let cell_h = 88.0;
        let rung_header_w = 76.0;
        let margin_left = 14.0 + rung_header_w;
        let margin_top = 24.0;

        let max_req_cols = doc
            .program
            .rungs
            .iter()
            .map(|r| Self::rung_required_cols(&r.elements))
            .fold(5.0_f32, f32::max);

        let base_world_right = (viewport_rect.width() - 60.0).max(520.0);
        let avail_span = (base_world_right - margin_left - 24.0).max(320.0);
        let cell_w = (avail_span / max_req_cols.max(5.5)).clamp(84.0, 108.0);
        let min_required_right = margin_left + 24.0 + max_req_cols * cell_w + 16.0;
        let margin_right = base_world_right.max(min_required_right);

        let sim_active = doc.simulator.is_active();

        let content_sz = self.compute_content_size(doc, viewport_rect.width());
        let total_screen_w = content_sz.x;
        let total_screen_h = content_sz.y;

        let viewport_w = viewport_rect.width();
        let viewport_h = viewport_rect.height();

        let max_pan_x = 14.0_f32;
        let min_pan_x = (viewport_w - total_screen_w).min(max_pan_x);
        let pan_span_x = (max_pan_x - min_pan_x).max(0.0);

        let max_pan_y = 16.0_f32;
        let min_pan_y = (viewport_h - total_screen_h).min(max_pan_y);
        let pan_span_y = (max_pan_y - min_pan_y).max(0.0);

        let show_v_bar = pan_span_y > 4.0;
        let show_h_bar = pan_span_x > 4.0;
        let bar_thickness = 8.0_f32;

        let track_y_min = viewport_rect.min.y;
        let track_y_max = viewport_rect.max.y - (if show_h_bar { bar_thickness + 2.0 } else { 0.0 });
        let track_rect_v = Rect::from_min_max(
            pos2(viewport_rect.max.x - bar_thickness - 2.0, track_y_min),
            pos2(viewport_rect.max.x - 2.0, track_y_max),
        );

        let track_x_min = viewport_rect.min.x;
        let track_x_max = viewport_rect.max.x - (if show_v_bar { bar_thickness + 2.0 } else { 0.0 });
        let track_rect_h = Rect::from_min_max(
            pos2(track_x_min, viewport_rect.max.y - bar_thickness - 2.0),
            pos2(track_x_max, viewport_rect.max.y - 2.0),
        );

        let track_h = track_rect_v.height();
        let thumb_h = if total_screen_h > 0.0 {
            ((viewport_h / total_screen_h) * track_h).clamp(24.0, track_h)
        } else {
            track_h
        };
        let travel_h = (track_h - thumb_h).max(0.0);
        let fraction_y = if pan_span_y > 0.0 {
            ((max_pan_y - self.pan_offset.y) / pan_span_y).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let thumb_top_y = track_rect_v.min.y + fraction_y * travel_h;
        let thumb_rect_v = Rect::from_min_max(
            pos2(track_rect_v.min.x + 1.0, thumb_top_y),
            pos2(track_rect_v.max.x - 1.0, thumb_top_y + thumb_h),
        );

        let track_w = track_rect_h.width();
        let thumb_w = if total_screen_w > 0.0 {
            ((viewport_w / total_screen_w) * track_w).clamp(24.0, track_w)
        } else {
            track_w
        };
        let travel_w = (track_w - thumb_w).max(0.0);
        let fraction_x = if pan_span_x > 0.0 {
            ((max_pan_x - self.pan_offset.x) / pan_span_x).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let thumb_left_x = track_rect_h.min.x + fraction_x * travel_w;
        let thumb_rect_h = Rect::from_min_max(
            pos2(thumb_left_x, track_rect_h.min.y + 1.0),
            pos2(thumb_left_x + thumb_w, track_rect_h.max.y - 1.0),
        );

        let raw_mouse_pos = if canvas_enabled {
            response.interact_pointer_pos()
        } else {
            None
        };
        let raw_hover_pos = if canvas_enabled {
            ui.input(|i| i.pointer.hover_pos())
        } else {
            None
        };
        let raw_is_clicked = canvas_enabled && response.clicked();
        let raw_is_double_clicked = canvas_enabled && response.double_clicked();
        let raw_is_right_clicked = canvas_enabled && response.secondary_clicked();
        let raw_primary_pressed = canvas_enabled && ui.input(|i| i.pointer.primary_pressed());
        let raw_primary_down = canvas_enabled && ui.input(|i| i.pointer.primary_down());
        let raw_primary_released = canvas_enabled && ui.input(|i| i.pointer.primary_released());

        let v_bar_hovered = show_v_bar && raw_hover_pos.map_or(false, |hp| track_rect_v.contains(hp));
        let h_bar_hovered = show_h_bar && raw_hover_pos.map_or(false, |hp| track_rect_h.contains(hp));
        let scrollbar_hovered = v_bar_hovered || h_bar_hovered;

        // Process scrollbar dragging or track clicks
        if canvas_enabled {
            if raw_primary_released {
                self.dragging_scrollbar = None;
            }

            if let Some(drag) = self.dragging_scrollbar {
                match drag {
                    ScrollbarDrag::Vertical { start_mouse_y, start_pan_y } => {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                        if let Some(mp) = raw_mouse_pos.or(raw_hover_pos) {
                            let delta_screen_y = mp.y - start_mouse_y;
                            if travel_h > 0.0 {
                                let pan_delta_y = -(delta_screen_y / travel_h) * pan_span_y;
                                self.pan_offset.y = (start_pan_y + pan_delta_y).clamp(min_pan_y, max_pan_y);
                            }
                        }
                    }
                    ScrollbarDrag::Horizontal { start_mouse_x, start_pan_x } => {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                        if let Some(mp) = raw_mouse_pos.or(raw_hover_pos) {
                            let delta_screen_x = mp.x - start_mouse_x;
                            if travel_w > 0.0 {
                                let pan_delta_x = -(delta_screen_x / travel_w) * pan_span_x;
                                self.pan_offset.x = (start_pan_x + pan_delta_x).clamp(min_pan_x, max_pan_x);
                            }
                        }
                    }
                }
            } else if raw_primary_pressed {
                if show_v_bar && v_bar_hovered {
                    let v_thumb_hovered = raw_hover_pos.map_or(false, |hp| thumb_rect_v.contains(hp));
                    if v_thumb_hovered {
                        self.dragging_scrollbar = Some(ScrollbarDrag::Vertical {
                            start_mouse_y: raw_mouse_pos.map_or(thumb_rect_v.center().y, |p| p.y),
                            start_pan_y: self.pan_offset.y,
                        });
                    } else if let Some(mp) = raw_mouse_pos {
                        if mp.y < thumb_rect_v.min.y {
                            self.pan_offset.y = (self.pan_offset.y + viewport_h * 0.7).clamp(min_pan_y, max_pan_y);
                        } else if mp.y > thumb_rect_v.max.y {
                            self.pan_offset.y = (self.pan_offset.y - viewport_h * 0.7).clamp(min_pan_y, max_pan_y);
                        }
                    }
                } else if show_h_bar && h_bar_hovered {
                    let h_thumb_hovered = raw_hover_pos.map_or(false, |hp| thumb_rect_h.contains(hp));
                    if h_thumb_hovered {
                        self.dragging_scrollbar = Some(ScrollbarDrag::Horizontal {
                            start_mouse_x: raw_mouse_pos.map_or(thumb_rect_h.center().x, |p| p.x),
                            start_pan_x: self.pan_offset.x,
                        });
                    } else if let Some(mp) = raw_mouse_pos {
                        if mp.x < thumb_rect_h.min.x {
                            self.pan_offset.x = (self.pan_offset.x + viewport_w * 0.7).clamp(min_pan_x, max_pan_x);
                        } else if mp.x > thumb_rect_h.max.x {
                            self.pan_offset.x = (self.pan_offset.x - viewport_w * 0.7).clamp(min_pan_x, max_pan_x);
                        }
                    }
                }
            }
        }

        let is_interacting_with_scrollbar = self.dragging_scrollbar.is_some() || scrollbar_hovered;

        // Isolate canvas elements from mouse events when scrollbar is hovered or being dragged
        let mouse_pos = if is_interacting_with_scrollbar { None } else { raw_mouse_pos };
        let hover_pos = if is_interacting_with_scrollbar { None } else { raw_hover_pos };
        let is_clicked = !is_interacting_with_scrollbar && raw_is_clicked;
        let is_double_clicked = !is_interacting_with_scrollbar && raw_is_double_clicked;
        let is_right_clicked = !is_interacting_with_scrollbar && raw_is_right_clicked;
        let primary_pressed = !is_interacting_with_scrollbar && raw_primary_pressed;
        let primary_down = !is_interacting_with_scrollbar && raw_primary_down;
        let primary_released = !is_interacting_with_scrollbar && raw_primary_released;
        let additive_mod =
            canvas_enabled && ui.input(|i| i.modifiers.shift || i.modifiers.command || i.modifiers.ctrl);
        let pan_mode =
            canvas_enabled && (ui.input(|i| i.key_down(egui::Key::Space)) || *active_tool == ActiveTool::Pan);

        // Promote drag_candidate to active dragging_elem once pointer moves > 6px
        if primary_down && self.is_edit_mode && !sim_active && !pan_mode {
            if let Some((cid, ref cname, start_pos)) = self.drag_candidate {
                if self.dragging_elem.is_none() {
                    if let Some(hp) = hover_pos {
                        if hp.distance(start_pos) > 6.0 {
                            self.dragging_elem = Some((cid, cname.clone()));
                            self.marquee_start = None;
                        }
                    }
                }
            }
        }

        let mut is_dragging = self.dragging_elem.is_some() || self.dragging_tool.is_some();
        if is_dragging && ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
            self.dragging_elem = None;
            self.dragging_tool = None;
            self.drag_candidate = None;
            is_dragging = false;
        }
        let dragging_source_type: Option<TypeTag> = if let Some((tool, _)) = self.dragging_tool {
            tool.element_type()
        } else if let Some((drag_id, _)) = self.dragging_elem {
            doc.element_by_id(drag_id).map(|e| e.element_type)
        } else {
            None
        };

        // Compute active marquee selection rectangle if dragging from empty space
        let active_marquee_rect: Option<Rect> =
            if primary_down && !pan_mode && !is_dragging && self.drag_candidate.is_none() {
                if let Some(start_p) = self.marquee_start {
                    let cur_p = hover_pos.or(mouse_pos).unwrap_or(start_p);
                    if cur_p.distance(start_p) >= 4.0 {
                        Some(Rect::from_two_pos(start_p, cur_p))
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            };

        let mut action: Option<CanvasAction> = None;
        let mut drop_target: Option<DropTarget> = None;
        let mut any_interactive_hovered = is_interacting_with_scrollbar;
        let mut element_screen_bounds: Vec<(usize, u64, Rect)> = Vec::new();
        let mut rung_y_spans: Vec<(usize, f32, f32)> = Vec::new();
        let mut cur_y = margin_top;
        if let Some(msg) = doc.last_edit_rejection.take() {
            self.rejection_toast = Some((msg, ui.input(|i| i.time)));
        }
        let validation_report = validate_program(&doc.program);
        let mut pending_tooltip: Option<(Pos2, Vec<String>, Color32)> = None;

        // Collect all element IDs sharing the same address/variable as the selected element (Cross-Reference)
        let ref_element_ids: Vec<u64> = if let Some(sel_id) = doc.selected_element_id {
            if let Some(sel_el) = doc.element_by_id(sel_id) {
                let name = sel_el.name.trim();
                if !name.is_empty() {
                    doc.find_address_usages(name)
                        .into_iter()
                        .filter(|(_, uid, _)| *uid != sel_id)
                        .map(|(_, uid, _)| uid)
                        .collect()
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        // --- Render Rungs ---
        for (r_idx, rung) in doc.program.rungs.iter().enumerate() {
            let is_rung_selected = doc.selected_rung_idx == Some(r_idx);
            let label_elem = rung.elements.iter().find(|e| e.element_type == TypeTag::Label);
            let comment_lines = Self::rung_comment_lines(&rung.comment);
            let comment_offset = Self::rung_header_offset(rung);
            let has_header_bar = comment_offset > 0.0;

            // Network Header Bar (Shows Flag Badge for Target Label + Network Comment)
            if has_header_bar {
                let c_min = self.to_screen(pos2(margin_left + 8.0, cur_y + 2.0), origin);
                let c_max = self.to_screen(pos2(margin_right - 8.0, cur_y + comment_offset - 3.0), origin);
                let c_rect = Rect::from_two_pos(c_min, c_max);
                let comment_hovered = hover_pos.map_or(false, |hp| c_rect.contains(hp));
                painter.rect_filled(c_rect, 4.0 * self.zoom, palette.comment_bg);

                let accent_bar = Rect::from_min_max(
                    c_rect.min,
                    pos2(c_rect.min.x + 2.5 * self.zoom, c_rect.max.y),
                );
                painter.rect_filled(
                    accent_bar,
                    1.5 * self.zoom,
                    if is_rung_selected {
                        palette.accent_heading
                    } else {
                        palette.card_border
                    },
                );
                if comment_hovered {
                    painter.rect_stroke(
                        c_rect,
                        4.0 * self.zoom,
                        Stroke::new(1.0, palette.accent_heading.gamma_multiply(0.45)),
                        StrokeKind::Inside,
                    );
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }

                let first_line_y = self.to_screen(pos2(0.0, cur_y + 12.5), origin).y;
                let mut text_x = c_rect.min.x + 8.0 * self.zoom;

                // Prominent Flag Badge for Target Label
                if let Some(lbl) = label_elem {
                    let badge_text = format!("LBL {}", lbl.name);
                    let badge_w = (badge_text.len() as f32 * 7.5 + 16.0) * self.zoom;
                    let badge_h = 16.5 * self.zoom;
                    let badge_rect = Rect::from_min_size(
                        pos2(text_x, first_line_y - badge_h * 0.5),
                        vec2(badge_w, badge_h),
                    );

                    painter.rect_filled(badge_rect, 3.5 * self.zoom, palette.pill_bg);
                    painter.rect_stroke(
                        badge_rect,
                        3.5 * self.zoom,
                        Stroke::new(1.3 * self.zoom, palette.accent_heading),
                        StrokeKind::Inside,
                    );
                    painter.text(
                        badge_rect.center(),
                        Align2::CENTER_CENTER,
                        badge_text,
                        self.font_mono(10.0),
                        palette.accent_heading,
                    );

                    text_x += badge_w + 10.0 * self.zoom;
                }

                if !comment_lines.is_empty() {
                    for (line_idx, line_str) in comment_lines.iter().enumerate() {
                        let line_y = self
                            .to_screen(pos2(0.0, cur_y + 12.5 + line_idx as f32 * 15.0), origin)
                            .y;
                        painter.text(
                            pos2(text_x, line_y),
                            Align2::LEFT_CENTER,
                            format!("// {}", line_str),
                            self.font_prop(11.0),
                            palette.comment_fg,
                        );
                    }
                } else if label_elem.is_some() {
                    painter.text(
                        pos2(text_x, first_line_y),
                        Align2::LEFT_CENTER,
                        "// Jump Destination Network",
                        self.font_prop(10.5),
                        palette.rung_text_dim,
                    );
                }

                if comment_hovered && is_double_clicked {
                    if let Some(lbl) = label_elem {
                        action = Some(CanvasAction::OpenConfigModal(lbl.id));
                    } else if let Some(first_el) = rung.elements.first() {
                        action = Some(CanvasAction::OpenConfigModal(first_el.id));
                    } else {
                        action = Some(CanvasAction::SelectRung(r_idx));
                    }
                }
            }

            let rung_start_y = cur_y + comment_offset;
            let wire_y = rung_start_y + cell_h * 0.5;

            // 1. Rung Number Badge on Left of L1 Rail
            let r_badge_world = Rect::from_min_size(
                pos2(18.0, rung_start_y + 12.0),
                vec2(rung_header_w - 8.0, cell_h - 24.0),
            );
            let r_badge_screen = Rect::from_min_max(
                self.to_screen(r_badge_world.min, origin),
                self.to_screen(r_badge_world.max, origin),
            );

            let badge_hovered = hover_pos.map_or(false, |hp| r_badge_screen.contains(hp));
            if badge_hovered {
                any_interactive_hovered = true;
            }
            let rung_sev = validation_report.highest_severity_for_rung(r_idx);
            let rung_val_col = match rung_sev {
                Some(ValidationSeverity::Error) => Some(palette.accent_danger),
                Some(ValidationSeverity::Warning) => Some(palette.accent_warn),
                _ => None,
            };
            let badge_bg = if is_rung_selected {
                palette.rung_badge_sel_bg
            } else if badge_hovered {
                palette.rung_badge_hov_bg
            } else {
                palette.rung_badge_bg
            };
            let badge_border = if is_rung_selected {
                palette.rung_badge_sel_border
            } else if let Some(vc) = rung_val_col {
                vc
            } else {
                palette.rung_badge_border
            };

            painter.rect_filled(r_badge_screen, 6.0 * self.zoom, badge_bg);
            painter.rect_stroke(
                r_badge_screen,
                6.0 * self.zoom,
                Stroke::new(1.5 * self.zoom, badge_border),
                StrokeKind::Inside,
            );
            if let Some(vc) = rung_val_col {
                let dot_r = 4.5 * self.zoom;
                let dot_c = pos2(
                    r_badge_screen.max.x - 5.5 * self.zoom,
                    r_badge_screen.min.y + 5.5 * self.zoom,
                );
                painter.circle_filled(dot_c, dot_r, vc);
                if badge_hovered && !is_dragging {
                    let mut lines = Vec::new();
                    for iss in validation_report.issues_for_rung(r_idx).into_iter().take(4) {
                        lines.push(format!("[{}] {}", iss.rule_id, iss.message));
                        lines.push(format!("  Fix: {}", iss.suggestion));
                    }
                    if !lines.is_empty() {
                        pending_tooltip = Some((
                            pos2(r_badge_screen.max.x + 8.0, r_badge_screen.min.y),
                            lines,
                            vc,
                        ));
                    }
                }
            }
            let top_sub = if label_elem.is_some() { "TARGET" } else { "NETWORK" };
            painter.text(
                pos2(
                    r_badge_screen.center().x,
                    r_badge_screen.center().y - 7.0 * self.zoom,
                ),
                Align2::CENTER_CENTER,
                top_sub,
                self.font_mono(7.5),
                if label_elem.is_some() { palette.accent_heading } else { palette.rung_text_dim },
            );
            painter.text(
                pos2(
                    r_badge_screen.center().x,
                    r_badge_screen.center().y + 7.0 * self.zoom,
                ),
                Align2::CENTER_CENTER,
                format!("{:02}", r_idx + 1),
                self.font_mono(13.0),
                if is_rung_selected {
                    palette.rung_text_sel
                } else {
                    palette.rung_text_num
                },
            );

            if is_double_clicked && badge_hovered {
                if let Some(first_el) = rung.elements.first() {
                    action = Some(CanvasAction::OpenConfigModal(first_el.id));
                } else {
                    action = Some(CanvasAction::SelectRung(r_idx));
                }
            } else if is_clicked && badge_hovered && self.dragging_elem.is_none() {
                action = Some(CanvasAction::SelectRung(r_idx));
            }
            if is_right_clicked && badge_hovered {
                self.context_menu_elem = Some((r_idx, 0));
                self.context_menu_pos = mouse_pos.or(hover_pos);
            }

            // Initial Lead-in from Left Power Rail
            let lead_in_w = 24.0;
            self.draw_power_wire(
                &painter,
                self.to_screen(pos2(margin_left, wire_y), origin),
                self.to_screen(pos2(margin_left + lead_in_w, wire_y), origin),
                sim_active,
                palette,
            );
            // T-junction dot at L1 rail intersection
            let pt_l1 = self.to_screen(pos2(margin_left, wire_y), origin);
            if sim_active {
                painter.circle_filled(pt_l1, 5.5 * self.zoom, palette.wire_glow);
                painter.circle_filled(pt_l1, 3.4 * self.zoom, palette.wire_energized);
            } else {
                painter.circle_filled(pt_l1, 3.0 * self.zoom, palette.l1_rail);
            }

            let mut cur_x = margin_left + lead_in_w;
            let mut max_y = rung_start_y + cell_h;
            let mut power_in = sim_active;

            if rung.elements.is_empty() {
                self.draw_power_wire(
                    &painter,
                    self.to_screen(pos2(cur_x, wire_y), origin),
                    self.to_screen(pos2(margin_right, wire_y), origin),
                    false,
                    palette,
                );
            } else {
                let last_idx = rung.elements.len().saturating_sub(1);
                for (elem_idx, elem) in rung.elements.iter().enumerate() {
                    // Right-align terminal elements (COIL, right-aligned output, or parallel output branch at the end of the rung)
                    let is_last = elem_idx == last_idx;
                    let is_output_branch = is_parallel_output_branch(elem);
                    if (elem.element_type.is_right_aligned() || is_output_branch) && is_last {
                        let elem_w = Self::element_width(elem, cell_w);
                        let right_pad = if is_output_branch { 0.0 } else { 16.0 };
                        let target_x = (margin_right - elem_w - right_pad).max(cur_x + 36.0);
                        if target_x > cur_x {
                            self.draw_power_wire(
                                &painter,
                                self.to_screen(pos2(cur_x, wire_y), origin),
                                self.to_screen(pos2(target_x, wire_y), origin),
                                power_in && sim_active,
                                palette,
                            );
                            cur_x = target_x;
                        }
                    }

                    let (next_x, elem_max_y) = self.draw_element_tree(
                        &painter,
                        origin,
                        r_idx,
                        elem,
                        pos2(cur_x, rung_start_y),
                        cell_w,
                        cell_h,
                        doc.selected_element_id,
                        &doc.selected_element_ids,
                        &ref_element_ids,
                        active_marquee_rect,
                        sim_active,
                        power_in,
                        hover_pos,
                        mouse_pos,
                        is_clicked && self.dragging_elem.is_none(),
                        is_double_clicked,
                        is_right_clicked,
                        primary_pressed && !pan_mode,
                        additive_mod,
                        active_tool,
                        &mut action,
                        &mut drop_target,
                        &mut any_interactive_hovered,
                        &mut element_screen_bounds,
                        doc.contact_style,
                        doc.coil_style,
                        palette,
                        dragging_source_type,
                        elem_idx == 0,
                        is_last,
                        false,
                        false,
                        is_output_branch && is_last,
                        margin_right,
                        &validation_report,
                        &mut pending_tooltip,
                    );

                    cur_x = next_x;
                    if elem_max_y > max_y {
                        max_y = elem_max_y;
                    }
                    power_in = elem.is_energized && sim_active;
                }

                // If no terminal output yet, show [+] button at end of elements (only in Edit mode)
                let has_terminal_output = rung.elements.iter().any(|e| {
                    e.element_type.is_right_aligned()
                        || is_parallel_output_branch(e)
                        || branch_contains_any_coil(e)
                });
                if self.is_edit_mode && !sim_active && !has_terminal_output && cur_x < margin_right - 40.0 {
                    self.draw_add_slot_button(
                        &painter,
                        origin,
                        pos2(cur_x + 20.0, wire_y),
                        hover_pos,
                        is_clicked && self.dragging_elem.is_none(),
                        r_idx,
                        &mut action,
                        &mut any_interactive_hovered,
                        palette,
                    );
                }

                // Connect tail wire to Right Power Rail (skip if a terminal parallel output branch already connected each path to L2)
                if cur_x < margin_right {
                    self.draw_power_wire(
                        &painter,
                        self.to_screen(pos2(cur_x, wire_y), origin),
                        self.to_screen(pos2(margin_right, wire_y), origin),
                        rung.is_energized && sim_active,
                        palette,
                    );
                    // T-junction dot at L2 rail intersection
                    let pt_l2 = self.to_screen(pos2(margin_right, wire_y), origin);
                    if rung.is_energized && sim_active {
                        painter.circle_filled(pt_l2, 5.5 * self.zoom, palette.wire_glow);
                        painter.circle_filled(pt_l2, 3.4 * self.zoom, palette.wire_energized);
                    } else {
                        painter.circle_filled(pt_l2, 3.0 * self.zoom, palette.l2_rail);
                    }
                }
            }

            // Check if dragging over this rung's general wire area (when not hovering a specific element)
            if is_dragging && drop_target.is_none() {
                let rung_band_world =
                    Rect::from_min_max(pos2(18.0, rung_start_y), pos2(margin_right, max_y));
                let rung_band_screen = Rect::from_min_max(
                    self.to_screen(rung_band_world.min, origin),
                    self.to_screen(rung_band_world.max, origin),
                );
                if hover_pos.map_or(false, |hp| rung_band_screen.contains(hp)) {
                    let append_violation = dragging_source_type
                        .and_then(|src_type| validate_rung_append(src_type, &doc.program.rungs[r_idx]).err());
                    let append_rule = append_violation.map(|v| v.rule_id);
                    let append_err = append_violation.map(|v| v.formatted());
                    let slot_col = if append_err.is_some() {
                        palette.accent_danger
                    } else {
                        palette.accent_heading
                    };
                    let slot_rect = rung_band_screen.shrink(3.0 * self.zoom);
                    painter.rect_filled(
                        slot_rect,
                        CornerRadius::same(5),
                        slot_col.gamma_multiply(0.10),
                    );
                    draw_dashed_rect(
                        &painter,
                        slot_rect,
                        Stroke::new(1.6 * self.zoom, slot_col),
                        6.0 * self.zoom,
                        4.0 * self.zoom,
                    );
                    let item_lbl = self
                        .dragging_tool
                        .as_ref()
                        .map(|(_, l)| l.as_str())
                        .or_else(|| self.dragging_elem.as_ref().map(|(_, l)| l.as_str()))
                        .unwrap_or("Element");
                    let label = if let Some(rid) = append_rule {
                        format!("Blocked {} · Network #{:02}", rid, r_idx + 1)
                    } else {
                        format!("Append {} · Network #{:02}", item_lbl, r_idx + 1)
                    };
                    painter.text(
                        pos2(
                            rung_band_screen.center().x,
                            rung_band_screen.min.y + 12.0 * self.zoom,
                        ),
                        Align2::CENTER_CENTER,
                        label,
                        self.font_prop(11.0),
                        slot_col,
                    );
                    drop_target = Some(DropTarget {
                        rung_idx: r_idx,
                        target_elem_id: None,
                        insert_pos: InsertPosition::AppendRung,
                        target_label: format!("Network #{:02} (Append)", r_idx + 1),
                        validation_rule_id: append_rule,
                        validation_error: append_err,
                        create_new_rung: false,
                    });
                }
            }

            rung_y_spans.push((r_idx, rung_start_y, max_y));
            cur_y = max_y + 18.0;
        }

        // --- Detect Drop onto Empty Space Below All Networks or on Empty Document ---
        let empty_space_hovered = is_dragging && drop_target.is_none() && hover_pos.map_or(false, |hp| {
            if !viewport_rect.contains(hp) {
                return false;
            }
            let world_p = self.to_world(hp, origin);
            doc.program.rungs.is_empty() || world_p.y >= cur_y - 8.0
        });

        let new_rung_idx = doc.program.rungs.len();
        let new_rung_start_y = cur_y + 6.0;
        let new_rung_band_world = Rect::from_min_max(
            pos2(18.0, new_rung_start_y),
            pos2(margin_right, new_rung_start_y + cell_h),
        );
        let new_rung_band_screen = Rect::from_min_max(
            self.to_screen(new_rung_band_world.min, origin),
            self.to_screen(new_rung_band_world.max, origin),
        );

        if empty_space_hovered {
            // Draw Ghost Preview for the New Network
            let ghost_border = palette.accent_heading;
            let ghost_slot = new_rung_band_screen.shrink(2.0 * self.zoom);
            painter.rect_filled(
                ghost_slot,
                CornerRadius::same(5),
                ghost_border.gamma_multiply(0.09),
            );
            draw_dashed_rect(
                &painter,
                ghost_slot,
                Stroke::new(1.6 * self.zoom, ghost_border),
                6.0 * self.zoom,
                4.0 * self.zoom,
            );

            // Ghost Network Badge on Left
            let badge_w = (rung_header_w - 8.0) * self.zoom;
            let badge_h = (cell_h - 28.0) * self.zoom;
            let badge_rect = Rect::from_center_size(
                pos2(
                    new_rung_band_screen.min.x + (18.0 + (rung_header_w - 8.0) * 0.5) * self.zoom,
                    new_rung_band_screen.center().y,
                ),
                vec2(badge_w, badge_h),
            );
            painter.rect_filled(badge_rect, 5.0 * self.zoom, palette.pill_bg);
            painter.rect_stroke(
                badge_rect,
                5.0 * self.zoom,
                Stroke::new(1.3 * self.zoom, ghost_border),
                StrokeKind::Inside,
            );
            painter.text(
                badge_rect.center(),
                Align2::CENTER_CENTER,
                format!("+ NET\n#{:02}", new_rung_idx + 1),
                self.font_mono(10.0),
                palette.accent_heading,
            );

            // Ghost horizontal power wire from L1 to L2
            let wire_y_screen = new_rung_band_screen.center().y;
            let pt_l1_screen = pos2(self.to_screen(pos2(margin_left, 0.0), origin).x, wire_y_screen);
            let pt_l2_screen = pos2(self.to_screen(pos2(margin_right, 0.0), origin).x, wire_y_screen);
            painter.line_segment(
                [pt_l1_screen, pt_l2_screen],
                Stroke::new(1.8 * self.zoom, palette.accent_heading.gamma_multiply(0.7)),
            );

            // Center Callout Banner
            let item_lbl = self
                .dragging_tool
                .as_ref()
                .map(|(_, l)| l.as_str())
                .or_else(|| self.dragging_elem.as_ref().map(|(_, l)| l.as_str()))
                .unwrap_or("Element");
            let banner_msg = format!("+ New Network #{:02} · {}", new_rung_idx + 1, item_lbl);
            painter.text(
                pos2(new_rung_band_screen.center().x, new_rung_band_screen.center().y - 12.0 * self.zoom),
                Align2::CENTER_CENTER,
                banner_msg,
                self.font_prop(11.5),
                palette.accent_heading,
            );

            drop_target = Some(DropTarget {
                rung_idx: new_rung_idx,
                target_elem_id: None,
                insert_pos: InsertPosition::AppendRung,
                target_label: format!("New Network #{:02}", new_rung_idx + 1),
                validation_rule_id: None,
                validation_error: None,
                create_new_rung: true,
            });
        } else if is_dragging && drop_target.is_none() {
            // Hovering empty canvas area horizontally adjacent to an existing rung
            if let Some(hp) = hover_pos {
                if viewport_rect.contains(hp) {
                    let world_p = self.to_world(hp, origin);
                    for &(r_idx, r_start_y, r_end_y) in &rung_y_spans {
                        if world_p.y >= r_start_y - 6.0 && world_p.y <= r_end_y + 12.0 {
                            let append_violation = dragging_source_type.and_then(|src_type| {
                                doc.program
                                    .rungs
                                    .get(r_idx)
                                    .and_then(|r| validate_rung_append(src_type, r).err())
                            });
                            drop_target = Some(DropTarget {
                                rung_idx: r_idx,
                                target_elem_id: None,
                                insert_pos: InsertPosition::AppendRung,
                                target_label: format!("Network #{:02} (Append)", r_idx + 1),
                                validation_rule_id: append_violation.map(|v| v.rule_id),
                                validation_error: append_violation.map(|v| v.formatted()),
                                create_new_rung: false,
                            });
                            break;
                        }
                    }
                }
            }
        }

        // --- Draw Left Power Rail (L1 Hot) & Right Power Rail (L2 Neutral) ---
        let effective_bottom_y = if empty_space_hovered {
            new_rung_start_y + cell_h
        } else {
            cur_y
        };
        let rail_bottom_y = (effective_bottom_y - 8.0).max(margin_top + cell_h);
        let l1_top = self.to_screen(pos2(margin_left, margin_top), origin);
        let l1_bot = self.to_screen(pos2(margin_left, rail_bottom_y), origin);
        let l2_top = self.to_screen(pos2(margin_right, margin_top), origin);
        let l2_bot = self.to_screen(pos2(margin_right, rail_bottom_y), origin);

        if sim_active {
            // Soft outer glow on L1 rail when simulation is active
            painter.line_segment(
                [l1_top, l1_bot],
                Stroke::new(9.0 * self.zoom, palette.wire_glow),
            );
            painter.line_segment(
                [l1_top, l1_bot],
                Stroke::new(4.0 * self.zoom, palette.wire_energized),
            );
        } else {
            painter.line_segment(
                [l1_top, l1_bot],
                Stroke::new(4.0 * self.zoom, palette.l1_rail),
            );
        }
        painter.line_segment(
            [l2_top, l2_bot],
            Stroke::new(4.0 * self.zoom, palette.l2_rail),
        );

        // Bus bar bottom end caps
        let cap_w = 9.0 * self.zoom;
        painter.line_segment(
            [pos2(l1_bot.x - cap_w, l1_bot.y), pos2(l1_bot.x + cap_w, l1_bot.y)],
            Stroke::new(3.0 * self.zoom, if sim_active { palette.wire_energized } else { palette.l1_rail }),
        );
        painter.line_segment(
            [pos2(l2_bot.x - cap_w, l2_bot.y), pos2(l2_bot.x + cap_w, l2_bot.y)],
            Stroke::new(3.0 * self.zoom, palette.l2_rail),
        );

        // --- Live Debug / Force Floating HUD Bar (Simulator & Online Hardware Modes) ---
        if (!self.is_edit_mode || sim_active) && !is_dragging {
            if let (Some(_), Some(sel_id)) = (doc.selected_rung_idx, doc.selected_element_id) {
                if let Some(sel_elem) = doc.element_by_id(sel_id).cloned() {
                    if let Some(sel_rect) = self.find_selected_rect_on_screen(
                        doc,
                        origin,
                        margin_left,
                        margin_top,
                        margin_right,
                        cell_w,
                        cell_h,
                        sel_id,
                    ) {
                        let elem_addr = sel_elem.name.trim().to_string();
                        let hud_w = 356.0;
                        let hud_h = 26.0;
                        let hud_y = if sel_rect.min.y - hud_h - 6.0 > viewport_rect.min.y + 40.0 {
                            sel_rect.min.y - hud_h - 5.0
                        } else {
                            sel_rect.max.y + 5.0
                        };
                        let hud_cx = sel_rect.center().x.clamp(
                            viewport_rect.min.x + hud_w * 0.5 + 8.0,
                            (viewport_rect.max.x - hud_w * 0.5 - 8.0)
                                .max(viewport_rect.min.x + hud_w * 0.5 + 8.0),
                        );
                        let hud_rect = Rect::from_center_size(
                            pos2(hud_cx, hud_y + hud_h * 0.5),
                            vec2(hud_w, hud_h),
                        );
                        if hover_pos.map_or(false, |hp| hud_rect.contains(hp)) {
                            any_interactive_hovered = true;
                        }

                        let mode_border_col = if self.is_online_hw_mode {
                            palette.accent_warn
                        } else {
                            palette.accent_ok
                        };
                        painter.rect_filled(hud_rect, CornerRadius::same(6), palette.banner_bg);
                        painter.rect_stroke(
                            hud_rect,
                            CornerRadius::same(6),
                            Stroke::new(1.0, mode_border_col),
                            StrokeKind::Inside,
                        );

                        // Address & Live State Badge on left
                        let is_on = doc.simulator.get_var_value(&elem_addr);
                        let badge_w = 104.0;
                        let badge_rect = Rect::from_min_size(
                            pos2(hud_rect.min.x + 4.0, hud_rect.min.y + 3.0),
                            vec2(badge_w, hud_h - 6.0),
                        );
                        let state_col = if is_on {
                            palette.accent_ok
                        } else {
                            palette.accent_heading
                        };
                        painter.rect_filled(
                            badge_rect,
                            CornerRadius::same(4),
                            state_col.gamma_multiply(0.20),
                        );
                        painter.text(
                            badge_rect.center(),
                            Align2::CENTER_CENTER,
                            format!(
                                "{} · {}",
                                elem_addr,
                                if is_on { "ON" } else { "OFF" }
                            ),
                            FontId::monospace(9.5),
                            state_col,
                        );

                        let debug_btns: [(&str, Color32, CanvasAction); 4] = if self.is_online_hw_mode {
                            [
                                (
                                    "Force 1",
                                    palette.accent_ok,
                                    CanvasAction::DebugForceBit(elem_addr.clone(), true),
                                ),
                                (
                                    "Force 0",
                                    palette.accent_danger,
                                    CanvasAction::DebugForceBit(elem_addr.clone(), false),
                                ),
                                (
                                    "Unforce",
                                    palette.accent_warn,
                                    CanvasAction::DebugUnforceBit(elem_addr.clone()),
                                ),
                                (
                                    "Edit",
                                    palette.accent_heading,
                                    CanvasAction::SwitchToEditMode(Some(sel_id)),
                                ),
                            ]
                        } else {
                            [
                                (
                                    "Toggle",
                                    palette.accent_ok,
                                    CanvasAction::DebugToggleBit(elem_addr.clone()),
                                ),
                                (
                                    "Force 1",
                                    palette.accent_warn,
                                    CanvasAction::DebugForceBit(elem_addr.clone(), true),
                                ),
                                (
                                    "Force 0",
                                    palette.accent_danger,
                                    CanvasAction::DebugForceBit(elem_addr.clone(), false),
                                ),
                                (
                                    "Unforce",
                                    palette.text_primary,
                                    CanvasAction::DebugUnforceBit(elem_addr.clone()),
                                ),
                            ]
                        };

                        let rem_w = hud_w - badge_w - 10.0;
                        let btn_w = rem_w / 4.0;
                        for (i, (lbl, col, btn_act)) in debug_btns.into_iter().enumerate() {
                            let bx = hud_rect.min.x + badge_w + 5.0 + i as f32 * btn_w;
                            let b_rect = Rect::from_min_size(
                                pos2(bx + 1.0, hud_rect.min.y + 3.0),
                                vec2(btn_w - 2.0, hud_h - 6.0),
                            );
                            let b_hov = hover_pos.map_or(false, |hp| b_rect.contains(hp));
                            if b_hov {
                                painter.rect_filled(
                                    b_rect,
                                    CornerRadius::same(4),
                                    col.gamma_multiply(0.25),
                                );
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            painter.text(
                                b_rect.center(),
                                Align2::CENTER_CENTER,
                                lbl,
                                FontId::proportional(10.0),
                                if b_hov {
                                    if palette.is_light() {
                                        palette.text_primary
                                    } else {
                                        Color32::WHITE
                                    }
                                } else {
                                    col
                                },
                            );
                            if is_clicked && b_hov {
                                action = Some(btn_act);
                            }
                        }
                    }
                }
            }
        }

        // Precompute bottom-left STRICT_EDITOR Validation pill hit-box so clicking it never clears selection or starts marquee drag
        let val_err_count = validation_report.error_count();
        let val_warn_count = validation_report.warning_count();
        let val_pill_label = if val_err_count == 0 && val_warn_count == 0 {
            "✓ STRICT_EDITOR".to_string()
        } else if val_err_count > 0 && val_warn_count > 0 {
            format!("✕ {} Err · ⚠ {} Warn", val_err_count, val_warn_count)
        } else if val_err_count > 0 {
            format!("✕ {} Err", val_err_count)
        } else {
            format!("⚠ {} Warn", val_warn_count)
        };
        let validation_pill_rect: Option<Rect> =
            if viewport_rect.width() >= 320.0 && !is_dragging && self.context_menu_elem.is_none() {
                let pill_w = (val_pill_label.chars().count() as f32 * 7.2 + 20.0).clamp(118.0, 210.0);
                let pill_h = 24.0;
                let offset_x = 10.0;
                let offset_y = if show_h_bar { bar_thickness + 10.0 } else { 10.0 };
                let rect = Rect::from_min_size(
                    pos2(
                        viewport_rect.min.x + offset_x,
                        viewport_rect.max.y - pill_h - offset_y,
                    ),
                    vec2(pill_w, pill_h),
                );
                if hover_pos.map_or(false, |hp| rect.contains(hp)) {
                    any_interactive_hovered = true;
                }
                Some(rect)
            } else {
                None
            };

        // Precompute bottom-right Zoom & Fit CAD pill hit-box so clicking it never clears selection or starts marquee drag
        let zoom_pill_rect: Option<Rect> =
            if viewport_rect.width() >= 260.0 && !is_dragging && self.context_menu_elem.is_none() {
                let pill_w = 148.0;
                let pill_h = 24.0;
                let offset_x = if show_v_bar { bar_thickness + 10.0 } else { 10.0 };
                let offset_y = if show_h_bar { bar_thickness + 10.0 } else { 10.0 };
                let rect = Rect::from_min_size(
                    pos2(
                        viewport_rect.max.x - pill_w - offset_x,
                        viewport_rect.max.y - pill_h - offset_y,
                    ),
                    vec2(pill_w, pill_h),
                );
                if hover_pos.map_or(false, |hp| rect.contains(hp)) {
                    any_interactive_hovered = true;
                }
                Some(rect)
            } else {
                None
            };

        // --- Start Marquee Selection or Deselect when Pressing on Empty Space ---
        if primary_pressed && !pan_mode && !any_interactive_hovered && self.drag_candidate.is_none()
        {
            if let Some(p) = hover_pos.or(mouse_pos) {
                if viewport_rect.contains(p) {
                    self.marquee_start = Some(p);
                    if !additive_mod {
                        doc.clear_element_selection();
                    }
                }
            }
        }

        // --- Render Live CAD Marquee Selection Box ---
        if let Some(m_rect) = active_marquee_rect {
            let hit_count = element_screen_bounds
                .iter()
                .filter(|(_, _, erect)| m_rect.intersects(erect.shrink(6.0 * self.zoom)))
                .count();

            painter.rect_filled(
                m_rect,
                CornerRadius::same(4),
                Color32::from_rgba_unmultiplied(35, 165, 255, 38),
            );
            painter.rect_stroke(
                m_rect,
                CornerRadius::same(4),
                Stroke::new(1.6, palette.accent_heading),
                StrokeKind::Inside,
            );

            // CAD Corner ticks on the marquee box
            let tk = 8.0_f32.min(m_rect.width() * 0.4).min(m_rect.height() * 0.4);
            let ts = Stroke::new(2.4, palette.rung_badge_sel_border);
            painter.line_segment([m_rect.min, pos2(m_rect.min.x + tk, m_rect.min.y)], ts);
            painter.line_segment([m_rect.min, pos2(m_rect.min.x, m_rect.min.y + tk)], ts);
            painter.line_segment([pos2(m_rect.max.x - tk, m_rect.min.y), pos2(m_rect.max.x, m_rect.min.y)], ts);
            painter.line_segment([pos2(m_rect.max.x, m_rect.min.y), pos2(m_rect.max.x, m_rect.min.y + tk)], ts);
            painter.line_segment([pos2(m_rect.min.x, m_rect.max.y - tk), pos2(m_rect.min.x, m_rect.max.y)], ts);
            painter.line_segment([pos2(m_rect.min.x, m_rect.max.y), pos2(m_rect.min.x + tk, m_rect.max.y)], ts);
            painter.line_segment([pos2(m_rect.max.x - tk, m_rect.max.y), m_rect.max], ts);
            painter.line_segment([pos2(m_rect.max.x, m_rect.max.y - tk), m_rect.max], ts);

            // Live Selection Counter Badge near pointer
            if let Some(hp) = hover_pos {
                let tag_w = 148.0;
                let tag_x = (hp.x + 12.0).min((viewport_rect.max.x - tag_w - 6.0).max(viewport_rect.min.x + 6.0));
                let tag_rect = Rect::from_min_size(pos2(tag_x, hp.y + 12.0), vec2(tag_w, 24.0));
                painter.rect_filled(tag_rect, CornerRadius::same(5), palette.banner_bg);
                painter.rect_stroke(
                    tag_rect,
                    CornerRadius::same(5),
                    Stroke::new(1.2, palette.accent_heading),
                    StrokeKind::Inside,
                );
                painter.text(
                    tag_rect.center(),
                    Align2::CENTER_CENTER,
                    if hit_count > 0 {
                        format!("Select {} element(s)", hit_count)
                    } else {
                        "Drag to select...".to_string()
                    },
                    FontId::proportional(11.0),
                    if hit_count > 0 {
                        palette.accent_ok
                    } else {
                        palette.text_primary
                    },
                );
            }
        }

        // --- Floating Ghost Preview while Dragging an Element or Tool ---
        let drag_info = if let Some((_, ref drag_label)) = self.dragging_elem {
            Some(("MOVE", drag_label.clone(), palette.accent_heading))
        } else if let Some((_, ref tool_label)) = self.dragging_tool {
            Some(("PLACE", tool_label.clone(), palette.accent_ok))
        } else {
            None
        };

        if let Some((action_badge, label, accent_col)) = drag_info {
            let val_err = drop_target.as_ref().and_then(|dt| dt.validation_error.as_ref());
            ui.ctx().set_cursor_icon(if val_err.is_some() {
                egui::CursorIcon::NoDrop
            } else {
                egui::CursorIcon::Grabbing
            });
            if let Some(hp) = hover_pos {
                let ghost_w = 236.0;
                let ghost_h = 44.0;
                let ghost_x = (hp.x + 14.0)
                    .min((viewport_rect.max.x - ghost_w - 6.0).max(viewport_rect.min.x + 6.0));
                let ghost_y = (hp.y + 14.0)
                    .min((viewport_rect.max.y - ghost_h - 6.0).max(viewport_rect.min.y + 6.0));
                let ghost_rect = Rect::from_min_size(pos2(ghost_x, ghost_y), vec2(ghost_w, ghost_h));
                let border_col = if val_err.is_some() {
                    palette.accent_danger
                } else {
                    accent_col
                };

                painter.add(
                    egui::Shadow {
                        offset: [0, 4],
                        blur: 12,
                        spread: 0,
                        color: Color32::from_black_alpha(if palette.is_light() { 38 } else { 110 }),
                    }
                    .as_shape(ghost_rect, CornerRadius::same(6)),
                );
                painter.rect_filled(ghost_rect, CornerRadius::same(6), palette.card_bg);
                painter.rect_stroke(
                    ghost_rect,
                    CornerRadius::same(6),
                    Stroke::new(1.2, border_col),
                    StrokeKind::Inside,
                );

                // Left 30x30 tinted icon box
                let icon_box = Rect::from_min_size(
                    pos2(ghost_rect.min.x + 7.0, ghost_rect.min.y + 7.0),
                    vec2(30.0, 30.0),
                );
                painter.rect_filled(
                    icon_box,
                    CornerRadius::same(4),
                    border_col.gamma_multiply(0.14),
                );
                let drag_icon = dragging_source_type
                    .map(type_tag_icon)
                    .unwrap_or(VectorIcon::FunctionBlock);
                paint_icon(&painter, icon_box.shrink(4.0), drag_icon, border_col);

                // Right top row: compact uppercase pill badge + monospace label
                let val_rule = drop_target.as_ref().and_then(|dt| dt.validation_rule_id);
                let badge_text = if let Some(rid) = val_rule {
                    format!("BLOCKED · {}", rid)
                } else if val_err.is_some() {
                    "BLOCKED".to_string()
                } else {
                    action_badge.to_string()
                };
                let badge_w = if val_rule.is_some() {
                    76.0
                } else if val_err.is_some() {
                    50.0
                } else {
                    38.0
                };
                let badge_rect = Rect::from_center_size(
                    pos2(ghost_rect.min.x + 44.0 + badge_w * 0.5, ghost_rect.min.y + 14.0),
                    vec2(badge_w, 14.0),
                );
                painter.rect_filled(badge_rect, CornerRadius::same(3), border_col);
                painter.text(
                    badge_rect.center(),
                    Align2::CENTER_CENTER,
                    &badge_text,
                    FontId::proportional(8.5),
                    Color32::WHITE,
                );

                let label_clip = Rect::from_min_max(
                    pos2(badge_rect.max.x + 6.0, ghost_rect.min.y + 2.0),
                    pos2(ghost_rect.max.x - 8.0, ghost_rect.min.y + 24.0),
                );
                painter.with_clip_rect(label_clip).text(
                    pos2(badge_rect.max.x + 6.0, ghost_rect.min.y + 14.0),
                    Align2::LEFT_CENTER,
                    &label,
                    FontId::monospace(11.5),
                    palette.text_primary,
                );

                // Right bottom row: concise destination or validation error
                let (sub_msg, sub_color) = if let Some(ref dt) = drop_target {
                    if let Some(ref err) = dt.validation_error {
                        (err.clone(), palette.accent_danger)
                    } else {
                        (dt.target_label.clone(), palette.accent_ok)
                    }
                } else {
                    (
                        "Hover a network or empty space".to_string(),
                        palette.text_primary.gamma_multiply(0.72),
                    )
                };
                let sub_clip = Rect::from_min_max(
                    pos2(ghost_rect.min.x + 44.0, ghost_rect.min.y + 22.0),
                    pos2(ghost_rect.max.x - 8.0, ghost_rect.max.y - 2.0),
                );
                painter.with_clip_rect(sub_clip).text(
                    pos2(ghost_rect.min.x + 44.0, ghost_rect.min.y + 31.0),
                    Align2::LEFT_CENTER,
                    sub_msg,
                    FontId::proportional(10.0),
                    sub_color,
                );
            }
        }

        // --- Draw Vertical & Horizontal Scrollbars ---
        if show_v_bar {
            let is_dragging_v = matches!(self.dragging_scrollbar, Some(ScrollbarDrag::Vertical { .. }));
            let is_thumb_hov = raw_hover_pos.map_or(false, |hp| thumb_rect_v.contains(hp));
            let track_bg = palette.canvas_bg.gamma_multiply(0.85);
            painter.rect_filled(track_rect_v, CornerRadius::same(4), track_bg);
            let thumb_bg = if is_dragging_v {
                palette.accent_heading
            } else if is_thumb_hov {
                palette.banner_border.gamma_multiply(1.5)
            } else {
                palette.banner_border.gamma_multiply(0.9)
            };
            painter.rect_filled(thumb_rect_v, CornerRadius::same(4), thumb_bg);
        }

        if show_h_bar {
            let is_dragging_h = matches!(self.dragging_scrollbar, Some(ScrollbarDrag::Horizontal { .. }));
            let is_thumb_hov = raw_hover_pos.map_or(false, |hp| thumb_rect_h.contains(hp));
            let track_bg = palette.canvas_bg.gamma_multiply(0.85);
            painter.rect_filled(track_rect_h, CornerRadius::same(4), track_bg);
            let thumb_bg = if is_dragging_h {
                palette.accent_heading
            } else if is_thumb_hov {
                palette.banner_border.gamma_multiply(1.5)
            } else {
                palette.banner_border.gamma_multiply(0.9)
            };
            painter.rect_filled(thumb_rect_h, CornerRadius::same(4), thumb_bg);
        }

        // --- Bottom-Left Floating STRICT_EDITOR Validation Pill & Diagnostics Popover ---
        if let Some(pill_rect) = validation_pill_rect {
            let pill_accent = if val_err_count > 0 {
                palette.accent_danger
            } else if val_warn_count > 0 {
                palette.accent_warn
            } else {
                palette.accent_ok
            };
            let pill_hov = hover_pos.map_or(false, |hp| pill_rect.contains(hp));
            if pill_hov {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            painter.rect_filled(
                pill_rect,
                CornerRadius::same(5),
                if pill_hov || self.show_diagnostics_popover {
                    palette.rung_badge_hov_bg
                } else {
                    palette.card_bg.gamma_multiply(0.94)
                },
            );
            painter.rect_stroke(
                pill_rect,
                CornerRadius::same(5),
                Stroke::new(
                    1.1,
                    pill_accent.gamma_multiply(if val_err_count > 0 || val_warn_count > 0 || self.show_diagnostics_popover {
                        0.95
                    } else {
                        0.65
                    }),
                ),
                StrokeKind::Inside,
            );
            painter.text(
                pill_rect.center(),
                Align2::CENTER_CENTER,
                &val_pill_label,
                FontId::monospace(10.5),
                pill_accent,
            );
            if pill_hov && !self.show_diagnostics_popover {
                let mut lines = Vec::new();
                let visible_issues: Vec<_> = validation_report
                    .issues
                    .iter()
                    .filter(|i| i.severity >= ValidationSeverity::Unknown)
                    .collect();
                if visible_issues.is_empty() {
                    lines.push("STRICT_EDITOR: All networks valid (0 errors, 0 warnings)".to_string());
                } else {
                    for iss in visible_issues.iter().take(6) {
                        lines.push(format!(
                            "Net #{:02} [{}] {}",
                            iss.rung_idx + 1,
                            iss.rule_id,
                            iss.message
                        ));
                    }
                    if visible_issues.len() > 6 {
                        lines.push(format!("+{} more issue(s)...", visible_issues.len() - 6));
                    }
                    lines.push("Click to open Diagnostics Panel".to_string());
                }
                pending_tooltip = Some((
                    pos2(pill_rect.min.x, pill_rect.min.y - 8.0),
                    lines,
                    pill_accent,
                ));
            }
            if is_clicked && pill_hov {
                self.show_diagnostics_popover = !self.show_diagnostics_popover;
                action = None;
            }

            // Interactive Diagnostics Popover anchored above the bottom-left pill
            if self.show_diagnostics_popover {
                let pop_w = 360.0_f32.min((viewport_rect.width() - 20.0).max(240.0));
                let pop_max_h = 240.0_f32.min((viewport_rect.height() - 60.0).max(120.0));
                let pop_x = pill_rect
                    .min
                    .x
                    .clamp(viewport_rect.min.x + 8.0, (viewport_rect.max.x - pop_w - 8.0).max(viewport_rect.min.x + 8.0));
                let pop_y = (pill_rect.min.y - pop_max_h - 8.0).max(viewport_rect.min.y + 8.0);
                let mut close_popover = ui.ctx().input(|i| i.key_pressed(egui::Key::Escape));
                let mut jump_target: Option<(usize, Option<u64>)> = None;

                let area_resp = egui::Area::new(egui::Id::unique("strict_editor_diagnostics_popover"))
                    .order(egui::Order::Foreground)
                    .fixed_pos(pos2(pop_x, pop_y))
                    .show(ui.ctx(), |ui| {
                        egui::Frame::new()
                            .fill(palette.card_bg)
                            .stroke(Stroke::new(1.1, pill_accent.gamma_multiply(0.85)))
                            .corner_radius(CornerRadius::same(7))
                            .inner_margin(egui::Margin::symmetric(10, 8))
                            .shadow(egui::Shadow {
                                offset: [0, 4],
                                blur: 14,
                                spread: 0,
                                color: Color32::from_black_alpha(if palette.is_light() { 40 } else { 125 }),
                            })
                            .show(ui, |ui| {
                                ui.set_width(pop_w - 20.0);
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "STRICT_EDITOR Diagnostics ({} Err · {} Warn)",
                                            val_err_count, val_warn_count
                                        ))
                                        .strong()
                                        .size(11.5)
                                        .color(palette.text_primary),
                                    );
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui
                                                .add(
                                                    egui::Button::new(
                                                        egui::RichText::new("×")
                                                            .size(13.0)
                                                            .color(palette.rung_text_dim),
                                                    )
                                                    .frame(false),
                                                )
                                                .clicked()
                                            {
                                                close_popover = true;
                                            }
                                        },
                                    );
                                });
                                ui.add_space(4.0);
                                ui.separator();
                                ui.add_space(4.0);

                                let diag_issues: Vec<_> = validation_report
                                    .issues
                                    .iter()
                                    .filter(|i| i.severity >= ValidationSeverity::Warning)
                                    .collect();

                                if diag_issues.is_empty() {
                                    ui.label(
                                        egui::RichText::new("✓ All STRICT_EDITOR rules passed")
                                            .size(11.5)
                                            .color(palette.accent_ok),
                                    );
                                } else {
                                    egui::ScrollArea::vertical()
                                        .max_height((pop_max_h - 54.0).max(80.0))
                                        .show(ui, |ui| {
                                            for iss in diag_issues {
                                                let row_col = match iss.severity {
                                                    ValidationSeverity::Error => palette.accent_danger,
                                                    ValidationSeverity::Warning => palette.accent_warn,
                                                    _ => palette.accent_heading,
                                                };
                                                let frame_resp = egui::Frame::new()
                                                    .fill(palette.canvas_bg.gamma_multiply(0.55))
                                                    .stroke(Stroke::new(0.8, row_col.gamma_multiply(0.45)))
                                                    .corner_radius(CornerRadius::same(4))
                                                    .inner_margin(egui::Margin::symmetric(7, 5))
                                                    .show(ui, |ui| {
                                                        ui.set_width(ui.available_width());
                                                        ui.horizontal_wrapped(|ui| {
                                                            ui.label(
                                                                egui::RichText::new(format!("[{}]", iss.rule_id))
                                                                    .monospace()
                                                                    .strong()
                                                                    .size(10.5)
                                                                    .color(row_col),
                                                            );
                                                            ui.label(
                                                                egui::RichText::new(format!("Net #{:02}", iss.rung_idx + 1))
                                                                    .monospace()
                                                                    .size(10.0)
                                                                    .color(palette.rung_text_dim),
                                                            );
                                                            ui.label(
                                                                egui::RichText::new(&iss.message)
                                                                    .size(10.5)
                                                                    .color(palette.text_primary),
                                                            );
                                                        });
                                                        ui.label(
                                                            egui::RichText::new(format!("Fix: {}", iss.suggestion))
                                                                .size(9.5)
                                                                .color(palette.rung_text_dim),
                                                        );
                                                    });
                                                let click_resp = ui.interact(
                                                    frame_resp.response.rect,
                                                    ui.scope_id().with(("diag_row", iss.rung_idx, iss.rule_id, iss.element_ids.first().copied().unwrap_or(0))),
                                                    Sense::click(),
                                                );
                                                if click_resp.hovered() {
                                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                                }
                                                if click_resp.clicked() {
                                                    jump_target = Some((iss.rung_idx, iss.element_ids.first().copied()));
                                                }
                                                ui.add_space(4.0);
                                            }
                                        });
                                }
                            });
                    });

                let pop_rect = area_resp.response.rect;
                if hover_pos.map_or(false, |hp| pop_rect.contains(hp)) {
                    any_interactive_hovered = true;
                }
                if let Some((r_idx, elem_id)) = jump_target {
                    self.scroll_to_rung(doc, r_idx);
                    if let Some(eid) = elem_id {
                        doc.set_single_selection(Some(r_idx), eid);
                    } else {
                        doc.selected_rung_idx = Some(r_idx);
                        doc.clear_element_selection();
                    }
                    action = None;
                }
                if primary_pressed {
                    if let Some(p) = raw_mouse_pos.or(raw_hover_pos) {
                        if !pop_rect.contains(p) && !pill_rect.contains(p) {
                            close_popover = true;
                        }
                    }
                }
                if close_popover {
                    self.show_diagnostics_popover = false;
                }
            }
        }

        // --- Top-Center R30 Transactional Edit Rejection Toast ---
        if let Some((ref msg, start_t)) = self.rejection_toast {
            let elapsed = ui.input(|i| i.time) - start_t;
            if elapsed < 3.2 {
                let alpha = if elapsed > 2.7 {
                    ((3.2 - elapsed) / 0.5).clamp(0.0, 1.0) as f32
                } else {
                    1.0
                };
                let toast_text = format!("Edit Blocked (R30): {}", msg);
                let font = FontId::proportional(11.0);
                let text_w = painter
                    .layout_no_wrap(toast_text.clone(), font.clone(), palette.text_primary)
                    .size()
                    .x;
                let toast_w = (text_w + 28.0).clamp(180.0, (viewport_rect.width() - 24.0).max(180.0));
                let toast_h = 28.0;
                let toast_rect = Rect::from_center_size(
                    pos2(viewport_rect.center().x, viewport_rect.min.y + 24.0),
                    vec2(toast_w, toast_h),
                );
                painter.add(
                    egui::Shadow {
                        offset: [0, 3],
                        blur: 10,
                        spread: 0,
                        color: Color32::from_black_alpha((if palette.is_light() { 36.0 } else { 110.0 } * alpha) as u8),
                    }
                    .as_shape(toast_rect, CornerRadius::same(6)),
                );
                painter.rect_filled(
                    toast_rect,
                    CornerRadius::same(6),
                    palette.card_bg.gamma_multiply(alpha),
                );
                painter.rect_stroke(
                    toast_rect,
                    CornerRadius::same(6),
                    Stroke::new(1.3, palette.accent_danger.gamma_multiply(alpha)),
                    StrokeKind::Inside,
                );
                painter.with_clip_rect(toast_rect.shrink(4.0)).text(
                    toast_rect.center(),
                    Align2::CENTER_CENTER,
                    toast_text,
                    font,
                    palette.accent_danger.gamma_multiply(alpha),
                );
                ui.ctx().request_repaint();
            } else {
                self.rejection_toast = None;
            }
        }

        // --- Floating Validation Tooltip Overlay (drawn last so it stays above rails & cards) ---
        if let Some((anchor, lines, accent)) = pending_tooltip {
            self.draw_validation_tooltip(&painter, viewport_rect, anchor, &lines, accent, palette);
        }

        // --- Bottom-Right Floating Zoom & Fit CAD Pill ([-], 100%, [+], | [Fit]) ---
        if let Some(pill_rect) = zoom_pill_rect {
            painter.rect_filled(
                pill_rect,
                CornerRadius::same(5),
                palette.card_bg.gamma_multiply(0.94),
            );
            painter.rect_stroke(
                pill_rect,
                CornerRadius::same(5),
                Stroke::new(1.0, palette.card_border),
                StrokeKind::Inside,
            );

            let btn_y = pill_rect.min.y + 2.0;
            let btn_h = pill_rect.height() - 4.0;
            let minus_rect = Rect::from_min_size(pos2(pill_rect.min.x + 3.0, btn_y), vec2(24.0, btn_h));
            let pct_rect = Rect::from_min_size(pos2(minus_rect.max.x + 1.0, btn_y), vec2(48.0, btn_h));
            let plus_rect = Rect::from_min_size(pos2(pct_rect.max.x + 1.0, btn_y), vec2(24.0, btn_h));
            let div_x = plus_rect.max.x + 3.0;
            painter.line_segment(
                [pos2(div_x, pill_rect.min.y + 5.0), pos2(div_x, pill_rect.max.y - 5.0)],
                Stroke::new(1.0, palette.card_border.gamma_multiply(0.8)),
            );
            let fit_rect = Rect::from_min_size(pos2(div_x + 3.0, btn_y), vec2(38.0, btn_h));

            let pct_label = format!("{:.0}%", self.zoom * 100.0);
            for (btn_r, lbl, kind) in [
                (minus_rect, "-", -1i8),
                (pct_rect, pct_label.as_str(), 0i8),
                (plus_rect, "+", 1i8),
                (fit_rect, "Fit", 2i8),
            ] {
                let is_hov = hover_pos.map_or(false, |hp| btn_r.contains(hp));
                if is_hov {
                    painter.rect_filled(btn_r, CornerRadius::same(4), palette.rung_badge_hov_bg);
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                painter.text(
                    btn_r.center(),
                    Align2::CENTER_CENTER,
                    lbl,
                    FontId::monospace(10.5),
                    if is_hov {
                        if palette.is_light() { palette.text_primary } else { Color32::WHITE }
                    } else if kind == 0 {
                        palette.accent_heading
                    } else {
                        palette.text_primary
                    },
                );
                if is_clicked && is_hov {
                    match kind {
                        -1 => self.set_target_zoom(
                            self.target_zoom / 1.15,
                            Some(viewport_rect.center()),
                        ),
                        0 => self.reset_view(),
                        1 => self.set_target_zoom(
                            self.target_zoom * 1.15,
                            Some(viewport_rect.center()),
                        ),
                        _ => self.fit_to_viewport(doc, viewport_rect.size()),
                    }
                    action = None;
                }
            }
        }

        // --- Finalize Drag & Drop or Marquee Box Selection on Mouse Release ---
        if primary_released {
            if let Some((drag_id, _)) = self.dragging_elem.take() {
                if let Some(dt) = drop_target {
                    if dt.validation_error.is_none() {
                        if dt.create_new_rung {
                            doc.move_element_to_new_rung(drag_id, Some(dt.rung_idx));
                        } else {
                            doc.move_element(drag_id, dt.rung_idx, dt.target_elem_id, dt.insert_pos);
                        }
                    }
                } else if let Some(hp) = hover_pos {
                    if viewport_rect.contains(hp) {
                        let world_p = self.to_world(hp, origin);
                        if doc.program.rungs.is_empty() || world_p.y >= cur_y - 8.0 {
                            doc.move_element_to_new_rung(drag_id, None);
                        } else {
                            // Find closest rung by Y
                            let mut best_r = 0;
                            let mut min_dist = f32::MAX;
                            for &(r_idx, r_start_y, r_end_y) in &rung_y_spans {
                                let mid_y = (r_start_y + r_end_y) * 0.5;
                                let dist = (world_p.y - mid_y).abs();
                                if dist < min_dist {
                                    min_dist = dist;
                                    best_r = r_idx;
                                }
                            }
                            let allowed = doc
                                .program
                                .rungs
                                .get(best_r)
                                .map_or(true, |r| {
                                    dragging_source_type
                                        .map_or(true, |st| validate_rung_append(st, r).is_ok())
                                });
                            if allowed {
                                doc.move_element(drag_id, best_r, None, InsertPosition::AppendRung);
                            }
                        }
                    }
                }
                action = None;
            } else if let Some((tool, _)) = self.dragging_tool.take() {
                if let Some(dt) = drop_target {
                    if dt.validation_error.is_none() {
                        let next_id = doc.program.alloc_id();
                        if let Some(elem) = tool.create_default_element(next_id) {
                            if dt.create_new_rung {
                                doc.insert_element_to_new_rung(elem, Some(dt.rung_idx));
                            } else {
                                doc.selected_rung_idx = Some(dt.rung_idx);
                                doc.selected_element_id = dt.target_elem_id;
                                doc.insert_element(elem, dt.insert_pos);
                            }
                        }
                    }
                } else if let Some(hp) = hover_pos {
                    if viewport_rect.contains(hp) {
                        let world_p = self.to_world(hp, origin);
                        if doc.program.rungs.is_empty() || world_p.y >= cur_y - 8.0 {
                            let next_id = doc.program.alloc_id();
                            if let Some(elem) = tool.create_default_element(next_id) {
                                doc.insert_element_to_new_rung(elem, None);
                            }
                        } else {
                            // Snap to closest rung by Y
                            let mut best_r = 0;
                            let mut min_dist = f32::MAX;
                            for &(r_idx, r_start_y, r_end_y) in &rung_y_spans {
                                let mid_y = (r_start_y + r_end_y) * 0.5;
                                let dist = (world_p.y - mid_y).abs();
                                if dist < min_dist {
                                    min_dist = dist;
                                    best_r = r_idx;
                                }
                            }
                            let allowed = doc
                                .program
                                .rungs
                                .get(best_r)
                                .map_or(true, |r| {
                                    tool.element_type()
                                        .map_or(true, |st| validate_rung_append(st, r).is_ok())
                                });
                            if allowed {
                                let next_id = doc.program.alloc_id();
                                if let Some(elem) = tool.create_default_element(next_id) {
                                    doc.selected_rung_idx = Some(best_r);
                                    doc.selected_element_id = None;
                                    doc.insert_element(elem, default_insert_pos);
                                }
                            }
                        }
                    }
                }
                action = None;
            } else if let Some(start_p) = self.marquee_start.take() {
                let end_p = hover_pos.or(mouse_pos).unwrap_or(start_p);
                if end_p.distance(start_p) >= 4.0 {
                    let m_rect = Rect::from_two_pos(start_p, end_p);
                    let hits: Vec<(usize, u64)> = element_screen_bounds
                        .iter()
                        .filter(|(_, _, erect)| m_rect.intersects(erect.shrink(6.0 * self.zoom)))
                        .map(|&(r, id, _)| (r, id))
                        .collect();
                    doc.set_box_selection(&hits, additive_mod);
                    action = None;
                } else if !additive_mod && action.is_none() && !any_interactive_hovered {
                    doc.clear_element_selection();
                }
            } else if is_clicked && action.is_none() && !any_interactive_hovered && !additive_mod {
                doc.clear_element_selection();
            }
            self.drag_candidate = None;
            self.dragging_tool = None;
        } else if !primary_down {
            self.drag_candidate = None;
            self.dragging_elem = None;
            self.dragging_tool = None;
            self.marquee_start = None;
        }

        // --- Native Pro IDE Context Menu (Right-Click on Element or Network) ---
        if let Some((r_idx, el_id)) = self.context_menu_elem {
            let mut close_menu = false;
            let screen_rect = ui.ctx().content_rect();
            let raw_pos = self
                .context_menu_pos
                .or(mouse_pos)
                .unwrap_or(viewport_rect.center());
            let menu_est_w = 224.0;
            let menu_est_h = 260.0;
            let clamped_pos = pos2(
                raw_pos.x.clamp(
                    screen_rect.min.x + 6.0,
                    (screen_rect.max.x - menu_est_w - 6.0).max(screen_rect.min.x + 6.0),
                ),
                raw_pos.y.clamp(
                    screen_rect.min.y + 6.0,
                    (screen_rect.max.y - menu_est_h - 6.0).max(screen_rect.min.y + 6.0),
                ),
            );

            let area_resp = egui::Area::new(egui::Id::unique("canvas_context_menu"))
                .order(egui::Order::Foreground)
                .fixed_pos(clamped_pos)
                .show(ui.ctx(), |ui| {
                    egui::Frame::new()
                        .fill(palette.card_bg)
                        .stroke(Stroke::new(1.0, palette.card_border))
                        .corner_radius(CornerRadius::same(6))
                        .inner_margin(egui::Margin::symmetric(5, 5))
                        .shadow(egui::Shadow {
                            offset: [0, 4],
                            blur: 12,
                            spread: 0,
                            color: Color32::from_black_alpha(if palette.is_light() {
                                38
                            } else {
                                110
                            }),
                        })
                        .show(ui, |ui| {
                            ui.set_width(214.0);
                            ui.spacing_mut().item_spacing.y = 1.0;

                            if !self.is_edit_mode || sim_active {
                                if el_id != 0 {
                                    if let Some(el) = doc.element_by_id(el_id).cloned() {
                                        let addr = el.name.trim().to_string();
                                        let is_on = doc.simulator.get_var_value(&addr);
                                        ui.horizontal(|ui| {
                                            ui.add_space(6.0);
                                            ui.label(
                                                egui::RichText::new(&addr)
                                                    .monospace()
                                                    .size(11.5)
                                                    .strong()
                                                    .color(palette.accent_heading),
                                            );
                                            ui.label(
                                                egui::RichText::new(format!(
                                                    "· {}",
                                                    el.element_type.display_name()
                                                ))
                                                .size(10.5)
                                                .color(palette.rung_text_dim),
                                            );
                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    ui.add_space(6.0);
                                                    ui.label(
                                                        egui::RichText::new(if is_on {
                                                            "● ON"
                                                        } else {
                                                            "○ OFF"
                                                        })
                                                        .monospace()
                                                        .size(10.5)
                                                        .strong()
                                                        .color(if is_on {
                                                            palette.accent_ok
                                                        } else {
                                                            palette.rung_text_dim
                                                        }),
                                                    );
                                                },
                                            );
                                        });
                                        ctx_menu_sep(ui, palette);
                                        if !self.is_online_hw_mode
                                            && ctx_menu_item(
                                                ui,
                                                "Toggle State (ON / OFF)",
                                                "Space",
                                                palette.text_primary,
                                                palette,
                                            )
                                        {
                                            action = Some(CanvasAction::DebugToggleBit(addr.clone()));
                                            close_menu = true;
                                        }
                                        if ctx_menu_item(
                                            ui,
                                            "Force ON (1)",
                                            "",
                                            palette.accent_ok,
                                            palette,
                                        ) {
                                            action = Some(CanvasAction::DebugForceBit(
                                                addr.clone(),
                                                true,
                                            ));
                                            close_menu = true;
                                        }
                                        if ctx_menu_item(
                                            ui,
                                            "Force OFF (0)",
                                            "",
                                            palette.accent_danger,
                                            palette,
                                        ) {
                                            action = Some(CanvasAction::DebugForceBit(
                                                addr.clone(),
                                                false,
                                            ));
                                            close_menu = true;
                                        }
                                        if ctx_menu_item(
                                            ui,
                                            "Release Force",
                                            "",
                                            palette.accent_warn,
                                            palette,
                                        ) {
                                            action = Some(CanvasAction::DebugUnforceBit(addr));
                                            close_menu = true;
                                        }
                                        ctx_menu_sep(ui, palette);
                                    }
                                }
                                if !doc.simulator.forced_bits.is_empty() || self.is_online_hw_mode {
                                    if ctx_menu_item(
                                        ui,
                                        "Clear All Forced Bits",
                                        "",
                                        palette.text_primary,
                                        palette,
                                    ) {
                                        action = Some(CanvasAction::DebugClearAllForces);
                                        close_menu = true;
                                    }
                                    ctx_menu_sep(ui, palette);
                                }
                                if ctx_menu_item(
                                    ui,
                                    "Switch to Edit Mode",
                                    "",
                                    palette.accent_heading,
                                    palette,
                                ) {
                                    let open_id = if el_id != 0 { Some(el_id) } else { None };
                                    action = Some(CanvasAction::SwitchToEditMode(open_id));
                                    close_menu = true;
                                }
                            } else {
                                let is_multi = doc.selected_element_ids.len() > 1
                                    && doc.selected_element_ids.contains(&el_id);
                                if is_multi {
                                    ui.horizontal(|ui| {
                                        ui.add_space(6.0);
                                        ui.label(
                                            egui::RichText::new(format!(
                                                "Selected ({} elements)",
                                                doc.selected_element_ids.len()
                                            ))
                                            .size(11.0)
                                            .strong()
                                            .color(palette.accent_heading),
                                        );
                                    });
                                    ctx_menu_sep(ui, palette);
                                    if ctx_menu_item(
                                        ui,
                                        "Add Parallel (Below)",
                                        "",
                                        palette.text_primary,
                                        palette,
                                    ) {
                                        action = Some(CanvasAction::AddParallelToSelection);
                                        close_menu = true;
                                    }
                                    if ctx_menu_item(
                                        ui,
                                        "Add Series (After)",
                                        "",
                                        palette.text_primary,
                                        palette,
                                    ) {
                                        action = Some(CanvasAction::AddSeriesToSelection(true));
                                        close_menu = true;
                                    }
                                    if ctx_menu_item(
                                        ui,
                                        "Add Series (Before)",
                                        "",
                                        palette.text_primary,
                                        palette,
                                    ) {
                                        action = Some(CanvasAction::AddSeriesToSelection(false));
                                        close_menu = true;
                                    }
                                    ctx_menu_sep(ui, palette);
                                    if ctx_menu_item(
                                        ui,
                                        "Invert Mode",
                                        "",
                                        palette.text_primary,
                                        palette,
                                    ) {
                                        action = Some(CanvasAction::ToggleSelectedModes);
                                        close_menu = true;
                                    }
                                    if ctx_menu_item(
                                        ui,
                                        "Delete Selected",
                                        "Del",
                                        palette.accent_danger,
                                        palette,
                                    ) {
                                        action = Some(CanvasAction::DeleteSelectedElements);
                                        close_menu = true;
                                    }
                                    ctx_menu_sep(ui, palette);
                                } else if el_id != 0 {
                                    ui.horizontal(|ui| {
                                        ui.add_space(6.0);
                                        if let Some(el) = doc.element_by_id(el_id) {
                                            ui.label(
                                                egui::RichText::new(&el.name)
                                                    .monospace()
                                                    .size(11.5)
                                                    .strong()
                                                    .color(palette.accent_heading),
                                            );
                                            ui.label(
                                                egui::RichText::new(format!(
                                                    "· {}",
                                                    el.element_type.display_name()
                                                ))
                                                .size(10.5)
                                                .color(palette.rung_text_dim),
                                            );
                                        } else {
                                            ui.label(
                                                egui::RichText::new(format!("Element #{}", el_id))
                                                    .monospace()
                                                    .size(11.5)
                                                    .strong()
                                                    .color(palette.accent_heading),
                                            );
                                        }
                                    });
                                    ctx_menu_sep(ui, palette);
                                    if ctx_menu_item(
                                        ui,
                                        "Configure Element...",
                                        "Enter",
                                        palette.accent_heading,
                                        palette,
                                    ) {
                                        action = Some(CanvasAction::OpenConfigModal(el_id));
                                        close_menu = true;
                                    }
                                    ctx_menu_sep(ui, palette);
                                    let cand_tag = active_tool.element_type().unwrap_or(TypeTag::Contact);
                                    let (par_err, after_err, before_err) = if let Some(target_el) = doc.element_by_id(el_id) {
                                        let pctx = doc
                                            .program
                                            .rungs
                                            .get(r_idx)
                                            .map(|r| find_placement_context(r, el_id))
                                            .unwrap_or_default();
                                        (
                                            validate_placement(cand_tag, target_el, InsertPosition::BottomParallel, pctx).err(),
                                            validate_placement(cand_tag, target_el, InsertPosition::Right, pctx).err(),
                                            validate_placement(cand_tag, target_el, InsertPosition::Left, pctx).err(),
                                        )
                                    } else {
                                        (None, None, None)
                                    };
                                    if let Some(v) = par_err {
                                        ctx_menu_item_disabled(ui, "Add Parallel (Below)", v, palette);
                                    } else if ctx_menu_item(
                                        ui,
                                        "Add Parallel (Below)",
                                        "",
                                        palette.text_primary,
                                        palette,
                                    ) {
                                        doc.set_single_selection(Some(r_idx), el_id);
                                        action = Some(CanvasAction::AddParallelToSelection);
                                        close_menu = true;
                                    }
                                    if let Some(v) = after_err {
                                        ctx_menu_item_disabled(ui, "Add Series (After)", v, palette);
                                    } else if ctx_menu_item(
                                        ui,
                                        "Add Series (After)",
                                        "",
                                        palette.text_primary,
                                        palette,
                                    ) {
                                        doc.set_single_selection(Some(r_idx), el_id);
                                        action = Some(CanvasAction::AddSeriesToSelection(true));
                                        close_menu = true;
                                    }
                                    if let Some(v) = before_err {
                                        ctx_menu_item_disabled(ui, "Add Series (Before)", v, palette);
                                    } else if ctx_menu_item(
                                        ui,
                                        "Add Series (Before)",
                                        "",
                                        palette.text_primary,
                                        palette,
                                    ) {
                                        doc.set_single_selection(Some(r_idx), el_id);
                                        action = Some(CanvasAction::AddSeriesToSelection(false));
                                        close_menu = true;
                                    }
                                    ctx_menu_sep(ui, palette);
                                    if ctx_menu_item(
                                        ui,
                                        "Invert Mode",
                                        "Space",
                                        palette.text_primary,
                                        palette,
                                    ) {
                                        action = Some(CanvasAction::ToggleElementMode(el_id));
                                        close_menu = true;
                                    }
                                    if ctx_menu_item(
                                        ui,
                                        "Delete Element",
                                        "Del",
                                        palette.accent_danger,
                                        palette,
                                    ) {
                                        action = Some(CanvasAction::DeleteElement(el_id));
                                        close_menu = true;
                                    }
                                    ctx_menu_sep(ui, palette);
                                }

                                ui.horizontal(|ui| {
                                    ui.add_space(6.0);
                                    ui.label(
                                        egui::RichText::new(format!("Network #{:02}", r_idx + 1))
                                            .size(10.5)
                                            .strong()
                                            .color(palette.rung_text_dim),
                                    );
                                });
                                if ctx_menu_item(
                                    ui,
                                    "Duplicate Network Below",
                                    "",
                                    palette.text_primary,
                                    palette,
                                ) {
                                    action = Some(CanvasAction::DuplicateRung(r_idx));
                                    close_menu = true;
                                }
                                if ctx_menu_item(
                                    ui,
                                    "Delete Network",
                                    "",
                                    palette.accent_danger,
                                    palette,
                                ) {
                                    action = Some(CanvasAction::DeleteRung(r_idx));
                                    close_menu = true;
                                }
                            }
                        });
                });

            if ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
                close_menu = true;
            }
            if !close_menu && !is_right_clicked {
                let clicked_outside = ui.ctx().input(|i| {
                    i.pointer.any_pressed()
                        && i.pointer
                            .interact_pos()
                            .map_or(true, |p| !area_resp.response.rect.contains(p))
                });
                if clicked_outside {
                    close_menu = true;
                }
            }
            if close_menu {
                self.context_menu_elem = None;
                self.context_menu_pos = None;
            }
        }

        let mut open_modal_for_id: Option<u64> = None;

        // --- Process Canvas Actions ---
        if let Some(act) = action {
            match act {
                CanvasAction::SelectRung(r_idx) => {
                    doc.selected_rung_idx = Some(r_idx);
                    doc.clear_element_selection();
                }
                CanvasAction::SelectElement {
                    rung_idx,
                    elem_id,
                    elem_name,
                    double_clicked,
                    additive,
                } => {
                    if additive && self.is_edit_mode && !sim_active && *active_tool == ActiveTool::Select {
                        doc.toggle_element_selection(Some(rung_idx), elem_id);
                    } else {
                        doc.set_single_selection(Some(rung_idx), elem_id);
                    }

                    if !self.is_edit_mode || sim_active {
                        if self.is_online_hw_mode {
                            // In Online Hardware Debug mode, double-clicking toggles hardware Force ON/OFF
                            if double_clicked {
                                let cur_on = doc.simulator.get_var_value(&elem_name);
                                self.pending_debug_event = Some(CanvasDebugEvent::ForceBit {
                                    addr: elem_name,
                                    val: !cur_on,
                                });
                            }
                        } else {
                            // In Simulator mode, clicking toggles the bit immediately
                            doc.toggle_input(&elem_name);
                        }
                    } else if double_clicked {
                        // Double-clicking any element in Edit mode opens the Professional Configuration Dialog!
                        open_modal_for_id = Some(elem_id);
                    }
                }
                CanvasAction::DebugToggleBit(addr) => {
                    doc.toggle_input(&addr);
                }
                CanvasAction::DebugForceBit(addr, val) => {
                    self.pending_debug_event = Some(CanvasDebugEvent::ForceBit { addr, val });
                }
                CanvasAction::DebugUnforceBit(addr) => {
                    self.pending_debug_event = Some(CanvasDebugEvent::UnforceBit { addr });
                }
                CanvasAction::DebugClearAllForces => {
                    self.pending_debug_event = Some(CanvasDebugEvent::ClearAllForces);
                }
                CanvasAction::SwitchToEditMode(open_elem_id) => {
                    self.pending_debug_event = Some(CanvasDebugEvent::SwitchToEditMode { open_elem_id });
                }
                CanvasAction::OpenConfigModal(el_id) => {
                    doc.set_single_selection(doc.selected_rung_idx, el_id);
                    open_modal_for_id = Some(el_id);
                }
                CanvasAction::QuickAppendToRung(r_idx) => {
                    doc.selected_rung_idx = Some(r_idx);
                    let next_id = doc.program.alloc_id();
                    let new_elem = if let Some(el) = active_tool.create_default_element(next_id) {
                        el
                    } else {
                        LadderElement::new_contact(
                            next_id,
                            "%IX0.0",
                            "START_PB",
                            ContactMode::NormallyOpen,
                        )
                    };
                    doc.insert_element(new_elem, InsertPosition::AppendRung);
                }
                CanvasAction::AddParallelToSelection => {
                    let next_id = doc.program.alloc_id();
                    let new_elem = if let Some(el) = active_tool.create_default_element(next_id) {
                        el
                    } else {
                        LadderElement::new_contact(
                            next_id,
                            "%IX0.0",
                            "PAR",
                            ContactMode::NormallyOpen,
                        )
                    };
                    doc.insert_parallel_to_selection(new_elem, false);
                }
                CanvasAction::AddSeriesToSelection(after) => {
                    let next_id = doc.program.alloc_id();
                    let new_elem = if let Some(el) = active_tool.create_default_element(next_id) {
                        el
                    } else {
                        LadderElement::new_contact(
                            next_id,
                            "%IX0.0",
                            "SER",
                            ContactMode::NormallyOpen,
                        )
                    };
                    doc.insert_series_to_selection(new_elem, after);
                }
                CanvasAction::ToggleElementMode(el_id) => {
                    doc.push_undo_snapshot();
                    doc.set_single_selection(doc.selected_rung_idx, el_id);
                    if let Some(el) = doc.selected_element_mut() {
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
                        doc.is_dirty = true;
                    }
                }
                CanvasAction::ToggleSelectedModes => {
                    doc.toggle_selected_elements_mode();
                }
                CanvasAction::DeleteElement(el_id) => {
                    doc.set_single_selection(doc.selected_rung_idx, el_id);
                    doc.delete_selected_element();
                }
                CanvasAction::DeleteSelectedElements => {
                    doc.delete_selected_element();
                }
                CanvasAction::DuplicateRung(r_idx) => {
                    doc.copy_rung_at(r_idx, r_idx + 1);
                }
                CanvasAction::DeleteRung(r_idx) => {
                    doc.delete_rung(r_idx);
                }
            }
        }

        open_modal_for_id
    }

    fn find_elem_rect_recursive(
        &self,
        elem: &LadderElement,
        start_world: Pos2,
        cell_w: f32,
        cell_h: f32,
        target_id: u64,
        origin: Pos2,
    ) -> (Option<Rect>, f32, f32) {
        if elem.element_type == TypeTag::Branch {
            let pad_x = 14.0;
            let inner_start_x = start_world.x + pad_x;
            let mut max_inner_x = inner_start_x + cell_w;
            let mut cur_branch_y = start_world.y;
            let mut found = None;

            for path in &elem.branch_paths {
                let mut path_x = inner_start_x;
                let mut path_max_y = cur_branch_y + cell_h;
                for sub in path {
                    let (sub_found, nx, my) = self.find_elem_rect_recursive(
                        sub,
                        pos2(path_x, cur_branch_y),
                        cell_w,
                        cell_h,
                        target_id,
                        origin,
                    );
                    if sub_found.is_some() {
                        found = sub_found;
                    }
                    path_x = nx;
                    if my > path_max_y {
                        path_max_y = my;
                    }
                }
                if path_x > max_inner_x {
                    max_inner_x = path_x;
                }
                cur_branch_y = path_max_y;
            }
            (found, max_inner_x + pad_x, cur_branch_y)
        } else {
            let found = if elem.id == target_id {
                let rw = Rect::from_min_size(start_world, vec2(cell_w, cell_h));
                Some(Rect::from_min_max(
                    self.to_screen(rw.min, origin),
                    self.to_screen(rw.max, origin),
                ))
            } else {
                None
            };
            (found, start_world.x + cell_w, start_world.y + cell_h)
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn find_selected_rect_on_screen(
        &self,
        doc: &Document,
        origin: Pos2,
        margin_left: f32,
        margin_top: f32,
        margin_right: f32,
        cell_w: f32,
        cell_h: f32,
        target_id: u64,
    ) -> Option<Rect> {
        let mut cur_y = margin_top;
        for rung in &doc.program.rungs {
            let comment_offset = Self::rung_header_offset(rung);
            let rung_start_y = cur_y + comment_offset;
            let mut cur_x = margin_left + 24.0;
            let mut max_y = rung_start_y + cell_h;

            let last_idx = rung.elements.len().saturating_sub(1);
            for (elem_idx, elem) in rung.elements.iter().enumerate() {
                let is_last = elem_idx == last_idx;
                let is_output_branch = is_parallel_output_branch(elem);
                if (elem.element_type.is_right_aligned() || is_output_branch) && is_last {
                    let elem_w = Self::element_width(elem, cell_w);
                    let right_pad = if is_output_branch { 0.0 } else { 16.0 };
                    let target_x = (margin_right - elem_w - right_pad).max(cur_x + 36.0);
                    if target_x > cur_x {
                        cur_x = target_x;
                    }
                }
                let (found, next_x, elem_max_y) = self.find_elem_rect_recursive(
                    elem,
                    pos2(cur_x, rung_start_y),
                    cell_w,
                    cell_h,
                    target_id,
                    origin,
                );
                if found.is_some() {
                    return found;
                }
                cur_x = next_x;
                if elem_max_y > max_y {
                    max_y = elem_max_y;
                }
            }
            cur_y = max_y + 18.0;
        }
        None
    }

    fn draw_validation_tooltip(
        &self,
        painter: &Painter,
        viewport_rect: Rect,
        anchor: Pos2,
        lines: &[String],
        accent: Color32,
        palette: ThemePalette,
    ) {
        if lines.is_empty() {
            return;
        }
        let font = FontId::proportional(10.5);
        let line_h = 15.0;
        let pad_x = 10.0;
        let pad_y = 7.0;
        let max_line_w = lines
            .iter()
            .map(|l| painter.layout_no_wrap(l.clone(), font.clone(), palette.text_primary).size().x)
            .fold(120.0_f32, f32::max)
            .min(380.0);
        let box_w = max_line_w + pad_x * 2.0;
        let box_h = lines.len() as f32 * line_h + pad_y * 2.0;

        let bx = anchor
            .x
            .clamp(
                viewport_rect.min.x + 8.0,
                (viewport_rect.max.x - box_w - 8.0).max(viewport_rect.min.x + 8.0),
            );
        let by = if anchor.y - box_h >= viewport_rect.min.y + 8.0 {
            anchor.y - box_h
        } else {
            anchor.y.clamp(
                viewport_rect.min.y + 8.0,
                (viewport_rect.max.y - box_h - 8.0).max(viewport_rect.min.y + 8.0),
            )
        };
        let rect = Rect::from_min_size(pos2(bx, by), vec2(box_w, box_h));

        painter.add(
            egui::Shadow {
                offset: [0, 3],
                blur: 10,
                spread: 0,
                color: Color32::from_black_alpha(if palette.is_light() { 36 } else { 115 }),
            }
            .as_shape(rect, CornerRadius::same(6)),
        );
        painter.rect_filled(rect, CornerRadius::same(6), palette.card_bg);
        painter.rect_stroke(
            rect,
            CornerRadius::same(6),
            Stroke::new(1.2, accent),
            StrokeKind::Inside,
        );

        let text_clip = rect.shrink(3.0);
        for (i, line) in lines.iter().enumerate() {
            let y = rect.min.y + pad_y + (i as f32 + 0.5) * line_h;
            let col = if line.starts_with("  Fix:") || line.starts_with("Click to") {
                palette.rung_text_dim
            } else if i == 0 {
                accent
            } else {
                palette.text_primary
            };
            painter.with_clip_rect(text_clip).text(
                pos2(rect.min.x + pad_x, y),
                Align2::LEFT_CENTER,
                line,
                font.clone(),
                col,
            );
        }
    }

    fn sample_quadratic_bezier(p0: Pos2, p1: Pos2, p2: Pos2, steps: usize) -> Vec<Pos2> {
        let mut pts = Vec::with_capacity(steps + 1);
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let omt = 1.0 - t;
            let x = omt * omt * p0.x + 2.0 * omt * t * p1.x + t * t * p2.x;
            let y = omt * omt * p0.y + 2.0 * omt * t * p1.y + t * t * p2.y;
            pts.push(pos2(x, y));
        }
        pts
    }

    fn draw_power_wire(
        &self,
        painter: &Painter,
        p1: Pos2,
        p2: Pos2,
        energized: bool,
        palette: ThemePalette,
    ) {
        if energized {
            painter.line_segment([p1, p2], Stroke::new(6.0 * self.zoom, palette.wire_glow));
            painter.line_segment(
                [p1, p2],
                Stroke::new(2.6 * self.zoom, palette.wire_energized),
            );
        } else {
            painter.line_segment([p1, p2], Stroke::new(2.0 * self.zoom, palette.wire_idle));
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_add_slot_button(
        &self,
        painter: &Painter,
        origin: Pos2,
        world_center: Pos2,
        hover_pos: Option<Pos2>,
        is_clicked: bool,
        rung_idx: usize,
        action: &mut Option<CanvasAction>,
        any_interactive_hovered: &mut bool,
        palette: ThemePalette,
    ) {
        let screen_c = self.to_screen(world_center, origin);
        let r = 10.5 * self.zoom;
        let btn_rect = Rect::from_center_size(screen_c, vec2(r * 2.0, r * 2.0));
        let hovered = hover_pos.map_or(false, |hp| btn_rect.contains(hp));
        if hovered {
            *any_interactive_hovered = true;
        }

        let bg = if hovered {
            Color32::from_rgb(35, 115, 210)
        } else {
            palette.card_bg
        };
        let border = if hovered {
            palette.accent_heading
        } else {
            palette.card_border
        };

        painter.circle_filled(screen_c, r, bg);
        painter.circle_stroke(screen_c, r, Stroke::new(1.3 * self.zoom, border));
        painter.text(
            screen_c,
            Align2::CENTER_CENTER,
            "+",
            self.font_mono(13.0),
            if hovered {
                Color32::WHITE
            } else {
                palette.accent_heading
            },
        );

        if is_clicked && hovered {
            *action = Some(CanvasAction::QuickAppendToRung(rung_idx));
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_element_tree(
        &mut self,
        painter: &Painter,
        origin: Pos2,
        rung_idx: usize,
        elem: &LadderElement,
        start_world: Pos2,
        cell_w: f32,
        cell_h: f32,
        selected_id: Option<u64>,
        selected_ids: &[u64],
        ref_ids: &[u64],
        active_marquee_rect: Option<Rect>,
        sim_active: bool,
        power_in: bool,
        hover_pos: Option<Pos2>,
        mouse_pos: Option<Pos2>,
        is_clicked: bool,
        is_double_clicked: bool,
        is_right_clicked: bool,
        primary_pressed: bool,
        additive_mod: bool,
        active_tool: &ActiveTool,
        action: &mut Option<CanvasAction>,
        drop_target: &mut Option<DropTarget>,
        any_interactive_hovered: &mut bool,
        element_screen_bounds: &mut Vec<(usize, u64, Rect)>,
        default_contact_style: ContactStyle,
        default_coil_style: CoilStyle,
        palette: ThemePalette,
        dragging_source_type: Option<TypeTag>,
        target_is_first: bool,
        target_is_last: bool,
        in_branch: bool,
        has_downstream_after_branch: bool,
        is_terminal_output_branch: bool,
        margin_right: f32,
        validation_report: &ProgramValidationReport,
        pending_tooltip: &mut Option<(Pos2, Vec<String>, Color32)>,
    ) -> (f32, f32) {
        if elem.element_type == TypeTag::Branch {
            let pad_x = 14.0;
            let branch_left_x = start_world.x;
            let inner_start_x = branch_left_x + pad_x;
            let mut max_inner_x = inner_start_x + cell_w;
            let mut cur_branch_y = start_world.y;
            let top_wire_y = start_world.y + cell_h * 0.5;

            let mut branch_wire_ys = Vec::new();
            let mut branch_end_xs = Vec::new();
            let mut branch_energized = Vec::new();

            for path in &elem.branch_paths {
                let mut path_x = inner_start_x;
                let mut path_max_y = cur_branch_y + cell_h;
                let mut sub_power = power_in;
                let row_wire_y = cur_branch_y + cell_h * 0.5;
                branch_wire_ys.push(row_wire_y);

                // Lead wire from left branch rail into first element of this path
                self.draw_power_wire(
                    painter,
                    self.to_screen(pos2(branch_left_x, row_wire_y), origin),
                    self.to_screen(pos2(inner_start_x, row_wire_y), origin),
                    power_in && sim_active,
                    palette,
                );

                let sub_last_idx = path.len().saturating_sub(1);
                for (sub_idx, sub) in path.iter().enumerate() {
                    let sub_is_first = sub_idx == 0;
                    let sub_is_last = sub_idx == sub_last_idx;
                    let sub_is_terminal_out =
                        is_terminal_output_branch && sub_is_last && is_parallel_output_branch(sub);
                    let (nx, my) = self.draw_element_tree(
                        painter,
                        origin,
                        rung_idx,
                        sub,
                        pos2(path_x, cur_branch_y),
                        cell_w,
                        cell_h,
                        selected_id,
                        selected_ids,
                        ref_ids,
                        active_marquee_rect,
                        sim_active,
                        sub_power,
                        hover_pos,
                        mouse_pos,
                        is_clicked,
                        is_double_clicked,
                        is_right_clicked,
                        primary_pressed,
                        additive_mod,
                        active_tool,
                        action,
                        drop_target,
                        any_interactive_hovered,
                        element_screen_bounds,
                        default_contact_style,
                        default_coil_style,
                        palette,
                        dragging_source_type,
                        sub_is_first,
                        sub_is_last,
                        true,
                        has_downstream_after_branch || !target_is_last,
                        sub_is_terminal_out,
                        margin_right,
                        validation_report,
                        pending_tooltip,
                    );
                    path_x = nx;
                    if my > path_max_y {
                        path_max_y = my;
                    }
                    sub_power = sub.is_energized && sim_active;
                }

                if path_x > max_inner_x {
                    max_inner_x = path_x;
                }
                branch_end_xs.push(path_x);
                branch_energized.push(sub_power);
                cur_branch_y = path_max_y;
            }

            let branch_right_x = max_inner_x + pad_x;

            if let Some(&bottom_wire_y) = branch_wire_ys.last() {
                // Left vertical branch rail
                self.draw_power_wire(
                    painter,
                    self.to_screen(pos2(branch_left_x, top_wire_y), origin),
                    self.to_screen(pos2(branch_left_x, bottom_wire_y), origin),
                    power_in && sim_active,
                    palette,
                );

                // Junction dots on left rail:
                // Only true T-junctions get dots (paths 0..N-2).
                // The bottom-most path (N-1) is a 90° corner elbow, which does NOT get a junction dot in PLC schematics.
                if branch_wire_ys.len() > 1 {
                    for &wy in &branch_wire_ys[..branch_wire_ys.len() - 1] {
                        let pt_l = self.to_screen(pos2(branch_left_x, wy), origin);
                        if power_in && sim_active {
                            painter.circle_filled(pt_l, 5.5 * self.zoom, palette.wire_glow);
                            painter.circle_filled(pt_l, 3.4 * self.zoom, palette.wire_energized);
                        } else {
                            painter.circle_filled(pt_l, 3.2 * self.zoom, palette.accent_heading);
                        }
                    }
                }

                if is_terminal_output_branch {
                    // TC03 / R07: Each parallel output branch connects directly to the L2 rail (no floating right join bar)
                    for (i, &wy) in branch_wire_ys.iter().enumerate() {
                        let end_x = branch_end_xs[i];
                        if end_x < margin_right {
                            self.draw_power_wire(
                                painter,
                                self.to_screen(pos2(end_x, wy), origin),
                                self.to_screen(pos2(margin_right, wy), origin),
                                branch_energized[i] && sim_active,
                                palette,
                            );
                            let pt_l2 = self.to_screen(pos2(margin_right, wy), origin);
                            if branch_energized[i] && sim_active {
                                painter.circle_filled(pt_l2, 5.5 * self.zoom, palette.wire_glow);
                                painter.circle_filled(pt_l2, 3.4 * self.zoom, palette.wire_energized);
                            } else {
                                painter.circle_filled(pt_l2, 3.0 * self.zoom, palette.l2_rail);
                            }
                        }
                    }
                    return (margin_right, cur_branch_y);
                }

                // Right horizontal extensions to branch_right_x
                for (i, &wy) in branch_wire_ys.iter().enumerate() {
                    let end_x = branch_end_xs[i];
                    self.draw_power_wire(
                        painter,
                        self.to_screen(pos2(end_x, wy), origin),
                        self.to_screen(pos2(branch_right_x, wy), origin),
                        branch_energized[i] && sim_active,
                        palette,
                    );
                }

                // Right vertical join rail:
                // Render segment-by-segment from bottom to top, conducting upward!
                if branch_wire_ys.len() > 1 {
                    for i in 0..branch_wire_ys.len() - 1 {
                        let seg_top_y = branch_wire_ys[i];
                        let seg_bot_y = branch_wire_ys[i + 1];
                        // This segment conducts upward if any branch at or below (i + 1) is energized
                        let seg_energized = sim_active && branch_energized[i + 1..].iter().any(|&e| e);
                        self.draw_power_wire(
                            painter,
                            self.to_screen(pos2(branch_right_x, seg_top_y), origin),
                            self.to_screen(pos2(branch_right_x, seg_bot_y), origin),
                            seg_energized,
                            palette,
                        );
                    }

                    // Junction dots on right rail:
                    // Only true T-junctions get dots (paths 0..N-2).
                    // The bottom-most path (N-1) is a 90° corner elbow, which does NOT get a junction dot.
                    for (i, &wy) in branch_wire_ys[..branch_wire_ys.len() - 1].iter().enumerate() {
                        let pt_r = self.to_screen(pos2(branch_right_x, wy), origin);
                        let dot_energized = sim_active && (branch_energized[i] || branch_energized[i + 1..].iter().any(|&e| e));
                        if dot_energized {
                            painter.circle_filled(pt_r, 5.5 * self.zoom, palette.wire_glow);
                            painter.circle_filled(pt_r, 3.4 * self.zoom, palette.wire_energized);
                        } else {
                            painter.circle_filled(pt_r, 3.2 * self.zoom, palette.accent_heading);
                        }
                    }
                }
            }

            (branch_right_x, cur_branch_y)
        } else {
            let rect_world = Rect::from_min_size(start_world, vec2(cell_w, cell_h));
            let rect_screen = Rect::from_min_max(
                self.to_screen(rect_world.min, origin),
                self.to_screen(rect_world.max, origin),
            );
            element_screen_bounds.push((rung_idx, elem.id, rect_screen));

            let in_marquee = active_marquee_rect
                .map_or(false, |mr| mr.intersects(rect_screen.shrink(6.0 * self.zoom)));
            let is_selected =
                selected_id == Some(elem.id) || selected_ids.contains(&elem.id) || in_marquee;
            let is_cross_ref = !is_selected && ref_ids.contains(&elem.id);
            let is_hovered = hover_pos.map_or(false, |hp| rect_screen.contains(hp));
            if is_hovered {
                *any_interactive_hovered = true;
            }

            // Record drag candidate when pressing primary mouse button on an element
            if primary_pressed && is_hovered && self.is_edit_mode && !sim_active {
                self.drag_candidate = Some((
                    elem.id,
                    format!("{} ({})", elem.name, elem.element_type.display_name()),
                    hover_pos.unwrap_or(rect_screen.center()),
                ));
            }

            let is_being_dragged = self
                .dragging_elem
                .as_ref()
                .map_or(false, |(drag_id, _)| *drag_id == elem.id);
            let is_dragging_other = self
                .dragging_elem
                .as_ref()
                .map_or(false, |(drag_id, _)| *drag_id != elem.id)
                || self.dragging_tool.is_some();

            // Detect directional insertion zone when an insert tool OR element drag is active
            let mut hovered_zone: Option<InsertPosition> = None;
            let mut validation_result: Result<InsertPosition, PlacementViolation> =
                Ok(InsertPosition::Right);

            if let Some(hp) = hover_pos {
                if is_hovered && is_dragging_other {
                    let rel_x = (hp.x - rect_screen.min.x) / rect_screen.width();
                    let rel_y = (hp.y - rect_screen.min.y) / rect_screen.height();
                    let raw_zone = if rel_x < 0.25 {
                        InsertPosition::Left
                    } else if rel_x > 0.75 {
                        InsertPosition::Right
                    } else if rel_y < 0.30 {
                        InsertPosition::TopParallel
                    } else if rel_y > 0.70 {
                        InsertPosition::BottomParallel
                    } else {
                        InsertPosition::Right
                    };

                    if let Some(src_tag) = dragging_source_type {
                        let pctx = PlacementContext {
                            target_is_first,
                            target_is_last,
                            in_branch,
                            has_downstream_after_branch,
                        };
                        validation_result = validate_placement(src_tag, elem, raw_zone, pctx);
                    }
                    hovered_zone = Some(raw_zone);
                }
            }

            if is_dragging_other && is_hovered {
                if let Some(zone) = hovered_zone {
                    let (effective_zone, val_rule, val_err) = match validation_result {
                        Ok(adjusted_zone) => (adjusted_zone, None, None),
                        Err(v) => (zone, Some(v.rule_id), Some(v.formatted())),
                    };

                    let pos_name = match effective_zone {
                        InsertPosition::Left => "Left of",
                        InsertPosition::Right => "Right of",
                        InsertPosition::TopParallel => "Parallel Above",
                        InsertPosition::BottomParallel => "Parallel Below",
                        InsertPosition::AppendRung => "Append to",
                    };
                    *drop_target = Some(DropTarget {
                        rung_idx,
                        target_elem_id: Some(elem.id),
                        insert_pos: effective_zone,
                        target_label: format!(
                            "{} {} (Network #{:02})",
                            pos_name,
                            elem.name,
                            rung_idx + 1
                        ),
                        validation_rule_id: val_rule,
                        validation_error: val_err,
                        create_new_rung: false,
                    });
                }
            }

            if is_clicked && is_hovered {
                *action = Some(CanvasAction::SelectElement {
                    rung_idx,
                    elem_id: elem.id,
                    elem_name: elem.name.clone(),
                    double_clicked: is_double_clicked,
                    additive: additive_mod,
                });
            }
            if is_right_clicked && is_hovered {
                self.context_menu_elem = Some((rung_idx, elem.id));
                self.context_menu_pos = mouse_pos.or(hover_pos);
            }

            let blocked_rule = validation_result.err().map(|v| v.rule_id);
            let hovered_zone_with_validity = hovered_zone.map(|z| (z, blocked_rule));

            let elem_issues = validation_report.issues_for_element(elem.id);
            let elem_validation = elem_issues
                .iter()
                .max_by_key(|i| i.severity)
                .filter(|i| i.severity >= ValidationSeverity::Unknown)
                .map(|i| (i.severity, i.rule_id));

            if is_hovered && !is_dragging_other && !is_being_dragged && !elem_issues.is_empty() {
                if let Some((top_sev, _)) = elem_validation {
                    let accent = match top_sev {
                        ValidationSeverity::Error => palette.accent_danger,
                        ValidationSeverity::Warning => palette.accent_warn,
                        _ => palette.accent_heading,
                    };
                    let mut lines = Vec::new();
                    for iss in elem_issues.iter().take(3) {
                        lines.push(format!("[{}] {}", iss.rule_id, iss.message));
                        lines.push(format!("  Fix: {}", iss.suggestion));
                    }
                    *pending_tooltip = Some((
                        pos2(rect_screen.min.x, rect_screen.min.y - 6.0),
                        lines,
                        accent,
                    ));
                }
            }

            self.draw_modern_element(
                painter,
                rect_screen,
                elem,
                is_selected,
                is_cross_ref,
                is_hovered,
                sim_active,
                power_in,
                hovered_zone_with_validity,
                elem_validation,
                default_contact_style,
                default_coil_style,
                palette,
            );

            if is_being_dragged {
                painter.rect_filled(
                    rect_screen.shrink(2.0 * self.zoom),
                    CornerRadius::same(5),
                    palette.canvas_bg.gamma_multiply(0.58),
                );
                draw_dashed_rect(
                    painter,
                    rect_screen.shrink(2.5 * self.zoom),
                    Stroke::new(1.4 * self.zoom, palette.accent_heading.gamma_multiply(0.85)),
                    5.0 * self.zoom,
                    3.5 * self.zoom,
                );
            }

            (start_world.x + cell_w, start_world.y + cell_h)
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_modern_element(
        &self,
        painter: &Painter,
        rect_screen: Rect,
        elem: &LadderElement,
        is_selected: bool,
        is_cross_ref: bool,
        is_hovered: bool,
        sim_active: bool,
        power_in: bool,
        hovered_zone: Option<(InsertPosition, Option<&'static str>)>,
        elem_validation: Option<(ValidationSeverity, &'static str)>,
        default_contact_style: ContactStyle,
        default_coil_style: CoilStyle,
        palette: ThemePalette,
    ) {
        let w = rect_screen.width();
        let h = rect_screen.height();
        let x0 = rect_screen.min.x;
        let y0 = rect_screen.min.y;
        let mid_y = y0 + h * 0.5;
        let center = rect_screen.center();

        // Professional CAD Corner-Bracket Selection & Hover Frame
        if is_selected {
            let sel_box = rect_screen.shrink(2.5 * self.zoom);
            painter.rect_filled(
                sel_box,
                6.0 * self.zoom,
                Color32::from_rgba_unmultiplied(255, 175, 30, 26),
            );
            painter.rect_stroke(
                sel_box,
                6.0 * self.zoom,
                Stroke::new(1.4 * self.zoom, palette.rung_badge_sel_border.gamma_multiply(0.65)),
                StrokeKind::Inside,
            );
            // Crisp CAD corner brackets
            let brk = 10.0 * self.zoom;
            let bs = Stroke::new(2.6 * self.zoom, palette.rung_badge_sel_border);
            // Top-left
            painter.line_segment([sel_box.min, pos2(sel_box.min.x + brk, sel_box.min.y)], bs);
            painter.line_segment([sel_box.min, pos2(sel_box.min.x, sel_box.min.y + brk)], bs);
            // Top-right
            painter.line_segment([pos2(sel_box.max.x - brk, sel_box.min.y), pos2(sel_box.max.x, sel_box.min.y)], bs);
            painter.line_segment([pos2(sel_box.max.x, sel_box.min.y), pos2(sel_box.max.x, sel_box.min.y + brk)], bs);
            // Bottom-left
            painter.line_segment([pos2(sel_box.min.x, sel_box.max.y - brk), pos2(sel_box.min.x, sel_box.max.y)], bs);
            painter.line_segment([pos2(sel_box.min.x, sel_box.max.y), pos2(sel_box.min.x + brk, sel_box.max.y)], bs);
            // Bottom-right
            painter.line_segment([pos2(sel_box.max.x - brk, sel_box.max.y), sel_box.max], bs);
            painter.line_segment([pos2(sel_box.max.x, sel_box.max.y - brk), sel_box.max], bs);
        } else if is_cross_ref {
            // Automatic Cross-Reference Highlight (Shows all other elements sharing this address)
            let ref_box = rect_screen.shrink(2.0 * self.zoom);
            let highlight_color = Color32::from_rgb(0, 190, 255); // Vibrant Cyan/Neon Blue
            painter.rect_filled(
                ref_box,
                6.0 * self.zoom,
                Color32::from_rgba_unmultiplied(0, 190, 255, 45), // Clear luminous glow fill
            );
            painter.rect_stroke(
                ref_box,
                6.0 * self.zoom,
                Stroke::new(2.4 * self.zoom, highlight_color), // Thick distinct border
                StrokeKind::Inside,
            );
            // Distinct "REF" badge at top-right
            let badge_w = 34.0 * self.zoom;
            let badge_h = 13.0 * self.zoom;
            let badge_rect = Rect::from_min_size(
                pos2(ref_box.max.x - badge_w - 2.0 * self.zoom, ref_box.min.y + 2.0 * self.zoom),
                vec2(badge_w, badge_h),
            );
            painter.rect_filled(
                badge_rect,
                3.0 * self.zoom,
                highlight_color,
            );
            painter.text(
                badge_rect.center(),
                Align2::CENTER_CENTER,
                "REF",
                FontId::monospace(8.0 * self.zoom),
                Color32::BLACK,
            );
        } else if is_hovered {
            let hov_box = rect_screen.shrink(3.5 * self.zoom);
            painter.rect_filled(
                hov_box,
                6.0 * self.zoom,
                Color32::from_rgba_unmultiplied(80, 140, 220, 22),
            );
            painter.rect_stroke(
                hov_box,
                6.0 * self.zoom,
                Stroke::new(1.2 * self.zoom, palette.accent_heading),
                StrokeKind::Inside,
            );
        }

        // Live STRICT_EDITOR Validation Overlay (subtle outline + compact top-left rule pill)
        if let Some((sev, rule_id)) = elem_validation {
            let val_col = match sev {
                ValidationSeverity::Error => palette.accent_danger,
                ValidationSeverity::Warning => palette.accent_warn,
                _ => palette.accent_heading,
            };
            let val_box = rect_screen.shrink(2.5 * self.zoom);
            if !is_selected && !is_cross_ref {
                painter.rect_filled(
                    val_box,
                    6.0 * self.zoom,
                    val_col.gamma_multiply(0.08),
                );
                painter.rect_stroke(
                    val_box,
                    6.0 * self.zoom,
                    Stroke::new(1.6 * self.zoom, val_col.gamma_multiply(0.85)),
                    StrokeKind::Inside,
                );
            }
            let pill_w = 26.0 * self.zoom;
            let pill_h = 13.0 * self.zoom;
            let pill_rect = Rect::from_min_size(
                pos2(val_box.min.x + 2.0 * self.zoom, val_box.min.y + 2.0 * self.zoom),
                vec2(pill_w, pill_h),
            );
            painter.rect_filled(pill_rect, 3.0 * self.zoom, val_col);
            painter.text(
                pill_rect.center(),
                Align2::CENTER_CENTER,
                rule_id,
                FontId::monospace((8.0 * self.zoom).max(6.0)),
                Color32::WHITE,
            );
        }

        // Precision CAD Insertion Snap Guides
        if let Some((zone, blocked_rule)) = hovered_zone {
            let is_valid = blocked_rule.is_none();
            let z = self.zoom;
            let guide_col = if is_valid {
                palette.accent_heading
            } else {
                palette.accent_danger
            };

            match zone {
                InsertPosition::Left | InsertPosition::Right => {
                    let is_left = matches!(zone, InsertPosition::Left);
                    let wash_rect = if is_left {
                        Rect::from_min_max(rect_screen.min, pos2(x0 + w * 0.24, rect_screen.max.y))
                    } else {
                        Rect::from_min_max(pos2(x0 + w * 0.76, y0), rect_screen.max)
                    };
                    painter.rect_filled(wash_rect, 4.0 * z, guide_col.gamma_multiply(0.14));

                    let beam_x = if is_left { x0 + 2.0 * z } else { x0 + w - 2.0 * z };
                    let top_pt = pos2(beam_x, y0 + 10.0 * z);
                    let bot_pt = pos2(beam_x, y0 + h - 10.0 * z);
                    painter.line_segment([top_pt, bot_pt], Stroke::new(3.0 * z, guide_col));
                    painter.circle_filled(top_pt, 3.5 * z, guide_col);
                    painter.circle_filled(bot_pt, 3.5 * z, guide_col);

                    let pill_txt = if let Some(rid) = blocked_rule {
                        format!("Blocked {}", rid)
                    } else if is_left {
                        "Before".to_string()
                    } else {
                        "After".to_string()
                    };
                    let pill_w = if blocked_rule.is_some() { 64.0 * z } else { 42.0 * z };
                    let pill_rect = Rect::from_center_size(pos2(beam_x, mid_y), vec2(pill_w, 16.0 * z));
                    painter.rect_filled(pill_rect, 8.0 * z, guide_col);
                    painter.text(
                        pill_rect.center(),
                        Align2::CENTER_CENTER,
                        pill_txt,
                        self.font_prop(9.0),
                        Color32::WHITE,
                    );
                }
                InsertPosition::TopParallel | InsertPosition::BottomParallel => {
                    let is_top = matches!(zone, InsertPosition::TopParallel);
                    let branch_y = if is_top {
                        y0 + 12.0 * z
                    } else {
                        y0 + h - 12.0 * z
                    };
                    let lx = x0 + 5.0 * z;
                    let rx = x0 + w - 5.0 * z;
                    painter.add(egui::epaint::PathShape::line(
                        vec![
                            pos2(lx, mid_y),
                            pos2(lx, branch_y),
                            pos2(rx, branch_y),
                            pos2(rx, mid_y),
                        ],
                        Stroke::new(1.8 * z, guide_col),
                    ));
                    painter.circle_filled(pos2(lx, mid_y), 3.0 * z, guide_col);
                    painter.circle_filled(pos2(rx, mid_y), 3.0 * z, guide_col);

                    let pill_txt = if let Some(rid) = blocked_rule {
                        format!("Blocked {}", rid)
                    } else {
                        "+ Parallel".to_string()
                    };
                    let pill_w = if blocked_rule.is_some() { 64.0 * z } else { 56.0 * z };
                    let pill_rect = Rect::from_center_size(pos2(center.x, branch_y), vec2(pill_w, 16.0 * z));
                    painter.rect_filled(pill_rect, 8.0 * z, guide_col);
                    painter.text(
                        pill_rect.center(),
                        Align2::CENTER_CENTER,
                        pill_txt,
                        self.font_prop(9.0),
                        Color32::WHITE,
                    );
                }
                InsertPosition::AppendRung => {
                    let slot_rect = rect_screen.shrink(2.5 * z);
                    painter.rect_filled(slot_rect, 4.0 * z, guide_col.gamma_multiply(0.10));
                    draw_dashed_rect(
                        painter,
                        slot_rect,
                        Stroke::new(1.5 * z, guide_col),
                        5.0 * z,
                        3.5 * z,
                    );
                    let pill_txt = if let Some(rid) = blocked_rule {
                        format!("Blocked {}", rid)
                    } else {
                        "+ Append".to_string()
                    };
                    let pill_w = if blocked_rule.is_some() { 64.0 * z } else { 54.0 * z };
                    let pill_rect = Rect::from_center_size(center, vec2(pill_w, 16.0 * z));
                    painter.rect_filled(pill_rect, 8.0 * z, guide_col);
                    painter.text(
                        pill_rect.center(),
                        Align2::CENTER_CENTER,
                        pill_txt,
                        self.font_prop(9.0),
                        Color32::WHITE,
                    );
                }
            }
        }

        // Address Color Accent based on IEC / prefix
        let prefix = elem
            .name
            .trim_start_matches('%')
            .chars()
            .next()
            .unwrap_or('I')
            .to_ascii_uppercase();
        let badge_color = match prefix {
            'X' | 'I' => palette.accent_warn,
            'Y' | 'Q' => palette.accent_heading,
            'M' => if palette.is_light() {
                Color32::from_rgb(130, 40, 200)
            } else {
                Color32::from_rgb(190, 130, 255)
            },
            'T' => palette.accent_ok,
            'C' => if palette.is_light() {
                Color32::from_rgb(108, 42, 190)
            } else {
                Color32::from_rgb(165, 120, 245)
            },
            _ => palette.rung_text_num,
        };

        match elem.element_type {
            TypeTag::Contact => {
                let active_style = default_contact_style;
                let is_closed = elem.is_done && sim_active;
                let is_energized = elem.is_energized && sim_active;
                let is_nc = elem.contact_mode == ContactMode::NormallyClosed;

                let contact_color = if is_energized {
                    palette.wire_energized
                } else if is_closed {
                    palette.symbol_closed
                } else if is_selected {
                    palette.rung_text_sel
                } else {
                    palette.symbol_idle
                };

                match active_style {
                    ContactStyle::IecStandard => {
                        // --- Crisp Industrial Contact Symbol (Standard IEC 61131-3) ---
                        let gap_half = 11.0 * self.zoom;
                        let bar_half_h = 15.0 * self.zoom;
                        let left_bar_x = center.x - gap_half;
                        let right_bar_x = center.x + gap_half;

                        // Left & Right Horizontal Wires
                        self.draw_power_wire(painter, pos2(x0, mid_y), pos2(left_bar_x, mid_y), power_in && sim_active, palette);
                        self.draw_power_wire(painter, pos2(right_bar_x, mid_y), pos2(x0 + w, mid_y), is_energized, palette);

                        // Glow between contact plates when conducting
                        if is_closed {
                            let glow_rect = Rect::from_min_max(
                                pos2(left_bar_x + 2.0 * self.zoom, mid_y - bar_half_h * 0.75),
                                pos2(right_bar_x - 2.0 * self.zoom, mid_y + bar_half_h * 0.75),
                            );
                            painter.rect_filled(
                                glow_rect,
                                3.0 * self.zoom,
                                if is_energized { palette.wire_glow } else { Color32::from_rgba_unmultiplied(45, 200, 95, 65) },
                            );
                        }

                        // Parallel Vertical Bars
                        painter.line_segment(
                            [pos2(left_bar_x, mid_y - bar_half_h), pos2(left_bar_x, mid_y + bar_half_h)],
                            Stroke::new(3.0 * self.zoom, contact_color),
                        );
                        painter.line_segment(
                            [pos2(right_bar_x, mid_y - bar_half_h), pos2(right_bar_x, mid_y + bar_half_h)],
                            Stroke::new(3.0 * self.zoom, contact_color),
                        );

                        // NC Diagonal Slash / or Transition P / N
                        if is_nc {
                            painter.line_segment(
                                [
                                    pos2(left_bar_x - 4.0 * self.zoom, mid_y + bar_half_h - 1.0 * self.zoom),
                                    pos2(right_bar_x + 4.0 * self.zoom, mid_y - bar_half_h + 1.0 * self.zoom),
                                ],
                                Stroke::new(2.2 * self.zoom, contact_color),
                            );
                        } else if elem.contact_mode == ContactMode::PositiveTransition {
                            painter.text(
                                center,
                                Align2::CENTER_CENTER,
                                "P",
                                self.font_mono(13.5),
                                contact_color,
                            );
                        } else if elem.contact_mode == ContactMode::NegativeTransition {
                            painter.text(
                                center,
                                Align2::CENTER_CENTER,
                                "N",
                                self.font_mono(13.5),
                                contact_color,
                            );
                        }
                    }

                    ContactStyle::JicSchematic => {
                        // --- JIC / NEMA Electrical Schematic (Mechanical Knife Switch) ---
                        let term_r = 3.6 * self.zoom;
                        let gap = 16.0 * self.zoom;
                        let c_left = pos2(center.x - gap, mid_y);
                        let c_right = pos2(center.x + gap, mid_y);

                        self.draw_power_wire(painter, pos2(x0, mid_y), pos2(c_left.x - term_r, mid_y), power_in && sim_active, palette);
                        self.draw_power_wire(painter, pos2(c_right.x + term_r, mid_y), pos2(x0 + w, mid_y), is_energized, palette);

                        let term_stroke = Stroke::new(1.8 * self.zoom, contact_color);
                        painter.circle_filled(c_left, term_r, palette.card_bg);
                        painter.circle_stroke(c_left, term_r, term_stroke);
                        painter.circle_filled(c_right, term_r, palette.card_bg);
                        painter.circle_stroke(c_right, term_r, term_stroke);

                        let blade_stroke = Stroke::new(2.5 * self.zoom, contact_color);
                        let blade_connected = if is_nc {
                            if sim_active { is_closed } else { true }
                        } else {
                            if sim_active { is_closed } else { false }
                        };

                        if blade_connected {
                            painter.line_segment([pos2(c_left.x, mid_y), pos2(c_right.x, mid_y)], blade_stroke);
                            painter.circle_filled(pos2(c_right.x, mid_y), 2.2 * self.zoom, contact_color);
                            if is_nc {
                                painter.line_segment(
                                    [pos2(center.x - 4.0 * self.zoom, mid_y + 6.0 * self.zoom), pos2(center.x + 4.0 * self.zoom, mid_y - 6.0 * self.zoom)],
                                    Stroke::new(1.8 * self.zoom, contact_color),
                                );
                            }
                        } else {
                            let tip_x = c_left.x + 28.0 * self.zoom;
                            let tip_y = mid_y - 14.0 * self.zoom;
                            painter.line_segment([pos2(c_left.x, mid_y), pos2(tip_x, tip_y)], blade_stroke);
                            painter.circle_filled(pos2(tip_x, tip_y), 2.2 * self.zoom, contact_color);
                            if is_nc {
                                painter.line_segment(
                                    [pos2(tip_x - 3.0 * self.zoom, tip_y + 4.0 * self.zoom), pos2(tip_x + 3.0 * self.zoom, tip_y - 4.0 * self.zoom)],
                                    Stroke::new(1.8 * self.zoom, contact_color),
                                );
                            }
                        }
                    }

                    ContactStyle::PushButton => {
                        // --- Industrial Push Button / Spring Plunger Switch ---
                        let term_r = 3.6 * self.zoom;
                        let gap = 16.0 * self.zoom;
                        let c_left = pos2(center.x - gap, mid_y);
                        let c_right = pos2(center.x + gap, mid_y);

                        self.draw_power_wire(painter, pos2(x0, mid_y), pos2(c_left.x - term_r, mid_y), power_in && sim_active, palette);
                        self.draw_power_wire(painter, pos2(c_right.x + term_r, mid_y), pos2(x0 + w, mid_y), is_energized, palette);

                        let term_stroke = Stroke::new(1.8 * self.zoom, contact_color);
                        painter.circle_filled(c_left, term_r, palette.card_bg);
                        painter.circle_stroke(c_left, term_r, term_stroke);
                        painter.circle_filled(c_right, term_r, palette.card_bg);
                        painter.circle_stroke(c_right, term_r, term_stroke);

                        let is_pressed = if sim_active { elem.is_done } else { false };
                        let (bar_y, cap_y) = if !is_nc {
                            if is_pressed {
                                (mid_y, mid_y - 8.0 * self.zoom)
                            } else {
                                (mid_y - 7.5 * self.zoom, mid_y - 15.5 * self.zoom)
                            }
                        } else {
                            if is_pressed {
                                (mid_y + 7.5 * self.zoom, mid_y - 0.5 * self.zoom)
                            } else {
                                (mid_y, mid_y - 8.0 * self.zoom)
                            }
                        };

                        let bar_half_w = 17.5 * self.zoom;
                        let pb_stroke = Stroke::new(2.4 * self.zoom, contact_color);

                        painter.line_segment([pos2(center.x - bar_half_w, bar_y), pos2(center.x + bar_half_w, bar_y)], pb_stroke);
                        painter.line_segment([pos2(center.x, bar_y), pos2(center.x, cap_y)], Stroke::new(2.0 * self.zoom, contact_color));

                        let cap_w = 14.0 * self.zoom;
                        let cap_h = 3.6 * self.zoom;
                        let cap_rect = Rect::from_center_size(pos2(center.x, cap_y), vec2(cap_w, cap_h));
                        painter.rect_filled(cap_rect, CornerRadius::same(2), contact_color);

                        if is_nc {
                            painter.line_segment(
                                [pos2(center.x - 4.0 * self.zoom, cap_y + 4.0 * self.zoom), pos2(center.x + 4.0 * self.zoom, cap_y - 4.0 * self.zoom)],
                                Stroke::new(1.8 * self.zoom, palette.accent_warn),
                            );
                        }
                    }

                    ContactStyle::DinEuropean => {
                        // --- European Standard DIN EN 60617 / Industrial Ladder (Reference: RRun_Auto & Cclose_gate) ---
                        // Left terminal has filled hinge dot (●).
                        // NO: Blade angles UP-RIGHT (/) towards vertical contact stub pointing UP.
                        // NC: Blade angles DOWN-RIGHT (\) towards vertical contact stub pointing DOWN.
                        let gap = 11.0 * self.zoom;
                        let c_left = pos2(center.x - gap, mid_y);
                        let c_right = pos2(center.x + gap, mid_y);

                        self.draw_power_wire(painter, pos2(x0, mid_y), c_left, power_in && sim_active, palette);
                        self.draw_power_wire(painter, c_right, pos2(x0 + w, mid_y), is_energized, palette);

                        let is_closed_now = if is_nc {
                            if sim_active { is_closed } else { true }
                        } else {
                            if sim_active { is_closed } else { false }
                        };

                        let blade_stroke = Stroke::new(2.4 * self.zoom, contact_color);
                        let stub_stroke = Stroke::new(2.0 * self.zoom, contact_color);
                        let stub_h = 7.5 * self.zoom;

                        // Left hinge terminal dot (●)
                        let dot_color = if power_in && sim_active {
                            palette.wire_energized
                        } else {
                            contact_color
                        };
                        painter.circle_filled(c_left, 3.0 * self.zoom, dot_color);

                        if is_nc {
                            // --- Normally Closed (NC): Blade angles DOWN-RIGHT (\), stub points DOWN ---
                            let stub_bottom = pos2(c_right.x, mid_y + stub_h);
                            // Right vertical stub pointing DOWN
                            painter.line_segment([c_right, stub_bottom], stub_stroke);

                            if is_closed_now {
                                // Resting / Closed: Blade connects from left dot to bottom of stub
                                painter.line_segment([c_left, pos2(stub_bottom.x + 1.5 * self.zoom, stub_bottom.y)], blade_stroke);
                            } else {
                                // Actuated / Open: Blade lifts UP away from the bottom stub
                                let blade_tip = pos2(c_right.x - 3.0 * self.zoom, mid_y - 6.0 * self.zoom);
                                painter.line_segment([c_left, blade_tip], blade_stroke);
                            }
                        } else {
                            // --- Normally Open (NO): Blade angles UP-RIGHT (/), stub points UP ---
                            let stub_top = pos2(c_right.x, mid_y - stub_h);
                            // Right vertical stub pointing UP
                            painter.line_segment([c_right, stub_top], stub_stroke);

                            if is_closed_now {
                                // Actuated / Closed: Blade connects from left dot to top of stub
                                painter.line_segment([c_left, pos2(stub_top.x + 1.5 * self.zoom, stub_top.y)], blade_stroke);
                            } else {
                                // Resting / Open: Blade tilted up, open above the stub
                                let blade_tip = pos2(c_right.x - 2.5 * self.zoom, mid_y - stub_h - 7.0 * self.zoom);
                                painter.line_segment([c_left, blade_tip], blade_stroke);
                            }
                        }

                        if elem.contact_mode == ContactMode::PositiveTransition {
                            painter.text(
                                pos2(center.x, mid_y + 11.0 * self.zoom),
                                Align2::CENTER_CENTER,
                                "P",
                                self.font_mono(11.0),
                                contact_color,
                            );
                        } else if elem.contact_mode == ContactMode::NegativeTransition {
                            painter.text(
                                pos2(center.x, mid_y + 11.0 * self.zoom),
                                Align2::CENTER_CENTER,
                                "N",
                                self.font_mono(11.0),
                                contact_color,
                            );
                        }
                    }

                    ContactStyle::ToggleSwitch => {
                        // --- Modern Electronic HMI Toggle / Rocker Switch ---
                        let sw_w = 40.0 * self.zoom;
                        let sw_h = 18.0 * self.zoom;
                        let sw_rect = Rect::from_center_size(center, vec2(sw_w, sw_h));

                        self.draw_power_wire(painter, pos2(x0, mid_y), pos2(sw_rect.min.x, mid_y), power_in && sim_active, palette);
                        self.draw_power_wire(painter, pos2(sw_rect.max.x, mid_y), pos2(x0 + w, mid_y), is_energized, palette);

                        let is_on = if is_nc {
                            if sim_active { is_closed } else { true }
                        } else {
                            if sim_active { is_closed } else { false }
                        };

                        let bg_col = if is_on {
                            if palette.is_light() {
                                Color32::from_rgb(208, 244, 218)
                            } else {
                                Color32::from_rgb(22, 54, 38)
                            }
                        } else {
                            palette.pill_bg
                        };
                        let border_col = if is_on {
                            palette.accent_ok
                        } else if is_selected {
                            palette.rung_badge_sel_border
                        } else {
                            palette.card_border
                        };

                        let corner_r = ((sw_h * 0.5) as u8).max(1);
                        painter.rect_filled(sw_rect, CornerRadius::same(corner_r), bg_col);
                        painter.rect_stroke(sw_rect, CornerRadius::same(corner_r), Stroke::new(1.4 * self.zoom, border_col), StrokeKind::Inside);

                        let knob_r = 6.2 * self.zoom;
                        let (knob_x, knob_col, tag_txt, tag_pos) = if is_on {
                            (
                                sw_rect.max.x - knob_r - 2.5 * self.zoom,
                                if is_energized { Color32::from_rgb(50, 240, 110) } else { palette.accent_ok },
                                if is_nc { "NC:1" } else { "ON" },
                                pos2(sw_rect.min.x + 9.5 * self.zoom, mid_y),
                            )
                        } else {
                            (
                                sw_rect.min.x + knob_r + 2.5 * self.zoom,
                                Color32::from_rgb(140, 150, 165),
                                if is_nc { "NC:0" } else { "OFF" },
                                pos2(sw_rect.max.x - 9.5 * self.zoom, mid_y),
                            )
                        };

                        painter.text(
                            tag_pos,
                            Align2::CENTER_CENTER,
                            tag_txt,
                            self.font_mono(8.0),
                            if is_on { palette.accent_ok } else { palette.rung_text_dim },
                        );

                        if is_on {
                            painter.circle_filled(pos2(knob_x, mid_y), knob_r + 2.0 * self.zoom, Color32::from_rgba_unmultiplied(45, 210, 95, 60));
                        }
                        painter.circle_filled(pos2(knob_x, mid_y), knob_r, knob_col);
                        painter.circle_stroke(pos2(knob_x, mid_y), knob_r, Stroke::new(1.0 * self.zoom, Color32::WHITE));
                    }
                }

                // Top Address Pill Badge
                self.draw_address_pill(
                    painter,
                    pos2(center.x, y0 + 14.0 * self.zoom),
                    &elem.name,
                    badge_color,
                    is_closed,
                    palette,
                );

                // Bottom Symbol Tag
                let is_memory = elem.name.trim_start_matches('%').starts_with('M');
                let bottom_text = if !elem.symbol.is_empty() && elem.symbol != elem.name {
                    elem.symbol.as_str()
                } else if is_memory {
                    match elem.contact_mode {
                        ContactMode::NormallyOpen => "Memory (NO)",
                        ContactMode::NormallyClosed => "Memory (NC)",
                        ContactMode::PositiveTransition => "Memory Edge (P)",
                        ContactMode::NegativeTransition => "Memory Edge (N)",
                    }
                } else {
                    match elem.contact_mode {
                        ContactMode::NormallyOpen => "NO Contact",
                        ContactMode::NormallyClosed => "NC Contact",
                        ContactMode::PositiveTransition => "P Contact",
                        ContactMode::NegativeTransition => "N Contact",
                    }
                };
                painter.text(
                    pos2(center.x, y0 + h - 12.0 * self.zoom),
                    Align2::CENTER_CENTER,
                    bottom_text,
                    self.font_prop(10.5),
                    palette.symbol_subtext,
                );
            }

            TypeTag::Coil => {
                let active_style = elem.coil_style.unwrap_or(default_coil_style);
                let is_on = elem.is_done && sim_active;
                let coil_color = if is_on {
                    palette.wire_energized
                } else if is_selected {
                    palette.rung_text_sel
                } else {
                    palette.coil_idle
                };

                match active_style {
                    CoilStyle::IecParentheses => {
                        // --- IEC 61131-3 Standard Curved Parentheses: —( )— ---
                        let gap = 14.5 * self.zoom;
                        let arc_half_h = 15.0 * self.zoom;
                        let tip_dx = 5.5 * self.zoom;

                        self.draw_power_wire(
                            painter,
                            pos2(x0, mid_y),
                            pos2(center.x - gap, mid_y),
                            power_in && sim_active,
                            palette,
                        );
                        self.draw_power_wire(
                            painter,
                            pos2(center.x + gap, mid_y),
                            pos2(x0 + w, mid_y),
                            elem.is_energized && sim_active,
                            palette,
                        );

                        let lp0 = pos2(center.x - gap + tip_dx, mid_y - arc_half_h);
                        let lp1 = pos2(center.x - gap - tip_dx, mid_y);
                        let lp2 = pos2(center.x - gap + tip_dx, mid_y + arc_half_h);
                        let left_pts = Self::sample_quadratic_bezier(lp0, lp1, lp2, 12);

                        let rp0 = pos2(center.x + gap - tip_dx, mid_y - arc_half_h);
                        let rp1 = pos2(center.x + gap + tip_dx, mid_y);
                        let rp2 = pos2(center.x + gap - tip_dx, mid_y + arc_half_h);
                        let right_pts = Self::sample_quadratic_bezier(rp0, rp1, rp2, 12);

                        let mut fill_pts = left_pts.clone();
                        for p in right_pts.iter().rev() {
                            fill_pts.push(*p);
                        }
                        let fill_color = if is_on {
                            palette.wire_glow
                        } else {
                            palette.card_bg
                        };
                        painter.add(egui::Shape::convex_polygon(fill_pts, fill_color, Stroke::NONE));

                        if is_on {
                            for w in left_pts.windows(2) {
                                painter.line_segment([w[0], w[1]], Stroke::new(7.0 * self.zoom, palette.wire_glow));
                            }
                            for w in right_pts.windows(2) {
                                painter.line_segment([w[0], w[1]], Stroke::new(7.0 * self.zoom, palette.wire_glow));
                            }
                        }

                        let arc_stroke = Stroke::new(2.8 * self.zoom, coil_color);
                        for w in left_pts.windows(2) {
                            painter.line_segment([w[0], w[1]], arc_stroke);
                        }
                        for w in right_pts.windows(2) {
                            painter.line_segment([w[0], w[1]], arc_stroke);
                        }

                        match elem.coil_mode {
                            CoilMode::Normal => {}
                            CoilMode::Negated => {
                                painter.line_segment(
                                    [
                                        pos2(center.x - 5.5 * self.zoom, mid_y + 11.0 * self.zoom),
                                        pos2(center.x + 5.5 * self.zoom, mid_y - 11.0 * self.zoom),
                                    ],
                                    Stroke::new(2.4 * self.zoom, coil_color),
                                );
                            }
                            CoilMode::SetLatch => {
                                painter.text(
                                    center,
                                    Align2::CENTER_CENTER,
                                    "S",
                                    self.font_mono(14.0),
                                    coil_color,
                                );
                            }
                            CoilMode::ResetUnlatch => {
                                painter.text(
                                    center,
                                    Align2::CENTER_CENTER,
                                    "R",
                                    self.font_mono(14.0),
                                    coil_color,
                                );
                            }
                            CoilMode::PositiveTransition => {
                                painter.text(
                                    center,
                                    Align2::CENTER_CENTER,
                                    "P",
                                    self.font_mono(14.0),
                                    coil_color,
                                );
                            }
                            CoilMode::NegativeTransition => {
                                painter.text(
                                    center,
                                    Align2::CENTER_CENTER,
                                    "N",
                                    self.font_mono(14.0),
                                    coil_color,
                                );
                            }
                        }
                    }

                    CoilStyle::CircleRelay => {
                        // --- European DIN EN 60617 / Japanese JIS Circle: —○— ---
                        let coil_r = 16.0 * self.zoom;
                        self.draw_power_wire(
                            painter,
                            pos2(x0, mid_y),
                            pos2(center.x - coil_r, mid_y),
                            power_in && sim_active,
                            palette,
                        );
                        self.draw_power_wire(
                            painter,
                            pos2(center.x + coil_r, mid_y),
                            pos2(x0 + w, mid_y),
                            elem.is_energized && sim_active,
                            palette,
                        );

                        if is_on {
                            painter.circle_filled(center, coil_r + 4.0 * self.zoom, palette.wire_glow);
                            painter.circle_filled(center, coil_r, Color32::from_rgba_unmultiplied(45, 215, 95, 80));
                        } else {
                            painter.circle_filled(center, coil_r, palette.card_bg);
                        }
                        painter.circle_stroke(center, coil_r, Stroke::new(2.8 * self.zoom, coil_color));

                        match elem.coil_mode {
                            CoilMode::Normal => {}
                            CoilMode::Negated => {
                                painter.line_segment(
                                    [
                                        pos2(center.x - coil_r * 0.707, mid_y + coil_r * 0.707),
                                        pos2(center.x + coil_r * 0.707, mid_y - coil_r * 0.707),
                                    ],
                                    Stroke::new(2.4 * self.zoom, coil_color),
                                );
                            }
                            CoilMode::SetLatch => {
                                painter.text(center, Align2::CENTER_CENTER, "S", self.font_mono(14.0), coil_color);
                            }
                            CoilMode::ResetUnlatch => {
                                painter.text(center, Align2::CENTER_CENTER, "R", self.font_mono(14.0), coil_color);
                            }
                            CoilMode::PositiveTransition => {
                                painter.text(center, Align2::CENTER_CENTER, "P", self.font_mono(14.0), coil_color);
                            }
                            CoilMode::NegativeTransition => {
                                painter.text(center, Align2::CENTER_CENTER, "N", self.font_mono(14.0), coil_color);
                            }
                        }
                    }

                    CoilStyle::NemaBox => {
                        // --- American NEMA / JIC Starter Box: —[ ]— ---
                        let box_w = 36.0 * self.zoom;
                        let box_h = 28.0 * self.zoom;
                        let box_rect = Rect::from_center_size(center, vec2(box_w, box_h));

                        self.draw_power_wire(painter, pos2(x0, mid_y), pos2(box_rect.min.x, mid_y), power_in && sim_active, palette);
                        self.draw_power_wire(painter, pos2(box_rect.max.x, mid_y), pos2(x0 + w, mid_y), elem.is_energized && sim_active, palette);

                        let bg_col = if is_on { Color32::from_rgb(22, 60, 38) } else { palette.card_bg };
                        painter.rect_filled(box_rect, CornerRadius::same((3.0 * self.zoom) as u8), bg_col);

                        if is_on {
                            painter.rect_stroke(box_rect.expand(2.5 * self.zoom), CornerRadius::same((4.0 * self.zoom) as u8), Stroke::new(4.0 * self.zoom, palette.wire_glow), StrokeKind::Inside);
                        }
                        painter.rect_stroke(box_rect, CornerRadius::same((3.0 * self.zoom) as u8), Stroke::new(2.4 * self.zoom, coil_color), StrokeKind::Inside);

                        // Screw terminal dots
                        painter.circle_filled(pos2(box_rect.min.x, mid_y), 2.2 * self.zoom, coil_color);
                        painter.circle_filled(pos2(box_rect.max.x, mid_y), 2.2 * self.zoom, coil_color);

                        let box_txt = match elem.coil_mode {
                            CoilMode::Normal => "M",
                            CoilMode::Negated => "/",
                            CoilMode::SetLatch => "S",
                            CoilMode::ResetUnlatch => "R",
                            CoilMode::PositiveTransition => "P",
                            CoilMode::NegativeTransition => "N",
                        };
                        painter.text(center, Align2::CENTER_CENTER, box_txt, self.font_mono(13.0), coil_color);
                    }

                    CoilStyle::SolenoidValve => {
                        // --- Industrial Electromechanical Solenoid Valve: —[ ⧖ ]— ---
                        let sol_w = 38.0 * self.zoom;
                        let sol_h = 26.0 * self.zoom;
                        let sol_rect = Rect::from_center_size(center, vec2(sol_w, sol_h));

                        self.draw_power_wire(painter, pos2(x0, mid_y), pos2(sol_rect.min.x, mid_y), power_in && sim_active, palette);
                        self.draw_power_wire(painter, pos2(sol_rect.max.x, mid_y), pos2(x0 + w, mid_y), elem.is_energized && sim_active, palette);

                        let bg_col = if is_on { Color32::from_rgb(18, 55, 42) } else { palette.card_bg };
                        painter.rect_filled(sol_rect, CornerRadius::same((2.0 * self.zoom) as u8), bg_col);

                        if is_on {
                            painter.rect_stroke(sol_rect.expand(2.0 * self.zoom), CornerRadius::same((3.0 * self.zoom) as u8), Stroke::new(4.0 * self.zoom, palette.wire_glow), StrokeKind::Inside);
                        }
                        painter.rect_stroke(sol_rect, CornerRadius::same((2.0 * self.zoom) as u8), Stroke::new(2.2 * self.zoom, coil_color), StrokeKind::Inside);

                        // Diagonal actuator slash
                        painter.line_segment(
                            [pos2(sol_rect.min.x, sol_rect.max.y), pos2(sol_rect.max.x, sol_rect.min.y)],
                            Stroke::new(1.8 * self.zoom, if is_on { palette.wire_energized } else { palette.rung_text_dim }),
                        );

                        let sol_txt = match elem.coil_mode {
                            CoilMode::Normal => "SOL",
                            CoilMode::Negated => "SOL/",
                            CoilMode::SetLatch => "S",
                            CoilMode::ResetUnlatch => "R",
                            CoilMode::PositiveTransition => "P",
                            CoilMode::NegativeTransition => "N",
                        };
                        painter.text(
                            pos2(center.x, sol_rect.min.y + 7.5 * self.zoom),
                            Align2::CENTER_CENTER,
                            sol_txt,
                            self.font_mono(8.0),
                            if is_on { Color32::WHITE } else { coil_color },
                        );
                    }

                    CoilStyle::PilotLight => {
                        // --- Modern Industrial Pilot Lamp / Indicator Light: —( ⨂ )— ---
                        let lamp_r = 16.0 * self.zoom;
                        self.draw_power_wire(painter, pos2(x0, mid_y), pos2(center.x - lamp_r, mid_y), power_in && sim_active, palette);
                        self.draw_power_wire(painter, pos2(center.x + lamp_r, mid_y), pos2(x0 + w, mid_y), elem.is_energized && sim_active, palette);

                        if is_on {
                            painter.circle_filled(center, lamp_r + 5.0 * self.zoom, palette.wire_glow);
                            painter.circle_filled(center, lamp_r, Color32::from_rgba_unmultiplied(50, 235, 110, 160));
                        } else {
                            painter.circle_filled(center, lamp_r, palette.card_bg);
                        }
                        painter.circle_stroke(center, lamp_r, Stroke::new(2.4 * self.zoom, coil_color));

                        let d = lamp_r * 0.7071;
                        let x_stroke = Stroke::new(2.0 * self.zoom, if is_on { Color32::WHITE } else { coil_color });
                        painter.line_segment([pos2(center.x - d, mid_y - d), pos2(center.x + d, mid_y + d)], x_stroke);
                        painter.line_segment([pos2(center.x - d, mid_y + d), pos2(center.x + d, mid_y - d)], x_stroke);

                        match elem.coil_mode {
                            CoilMode::Normal => {}
                            CoilMode::Negated => {
                                painter.circle_filled(pos2(center.x + lamp_r - 2.0 * self.zoom, mid_y - lamp_r + 2.0 * self.zoom), 4.0 * self.zoom, palette.accent_warn);
                            }
                            CoilMode::SetLatch => {
                                let badge_pos = pos2(center.x + lamp_r - 1.0 * self.zoom, mid_y - lamp_r + 1.0 * self.zoom);
                                painter.circle_filled(badge_pos, 5.0 * self.zoom, palette.accent_heading);
                                painter.text(badge_pos, Align2::CENTER_CENTER, "S", self.font_mono(8.0), Color32::WHITE);
                            }
                            CoilMode::ResetUnlatch => {
                                let badge_pos = pos2(center.x + lamp_r - 1.0 * self.zoom, mid_y - lamp_r + 1.0 * self.zoom);
                                painter.circle_filled(badge_pos, 5.0 * self.zoom, palette.accent_danger);
                                painter.text(badge_pos, Align2::CENTER_CENTER, "R", self.font_mono(8.0), Color32::WHITE);
                            }
                            CoilMode::PositiveTransition => {
                                let badge_pos = pos2(center.x + lamp_r - 1.0 * self.zoom, mid_y - lamp_r + 1.0 * self.zoom);
                                painter.circle_filled(badge_pos, 5.0 * self.zoom, palette.accent_heading);
                                painter.text(badge_pos, Align2::CENTER_CENTER, "P", self.font_mono(8.0), Color32::WHITE);
                            }
                            CoilMode::NegativeTransition => {
                                let badge_pos = pos2(center.x + lamp_r - 1.0 * self.zoom, mid_y - lamp_r + 1.0 * self.zoom);
                                painter.circle_filled(badge_pos, 5.0 * self.zoom, palette.accent_heading);
                                painter.text(badge_pos, Align2::CENTER_CENTER, "N", self.font_mono(8.0), Color32::WHITE);
                            }
                        }
                    }
                }

                // Top Address Pill
                self.draw_address_pill(
                    painter,
                    pos2(center.x, y0 + 14.0 * self.zoom),
                    &elem.name,
                    badge_color,
                    is_on,
                    palette,
                );

                // Bottom Symbol Tag
                let is_memory = elem.name.trim_start_matches('%').starts_with('M');
                let bottom_text = if !elem.symbol.is_empty() && elem.symbol != elem.name {
                    elem.symbol.as_str()
                } else if is_memory {
                    match elem.coil_mode {
                        CoilMode::Normal => "Memory Relay (M)",
                        CoilMode::Negated => "Negated Memory (M)",
                        CoilMode::SetLatch => "Set Memory (S)",
                        CoilMode::ResetUnlatch => "Reset Memory (R)",
                        CoilMode::PositiveTransition => "Pulse Memory (P)",
                        CoilMode::NegativeTransition => "Pulse Memory (N)",
                    }
                } else {
                    match active_style {
                        CoilStyle::IecParentheses => match elem.coil_mode {
                            CoilMode::Normal => "Output Coil (Q)",
                            CoilMode::Negated => "Negated Output (Q)",
                            CoilMode::SetLatch => "Set Output (S)",
                            CoilMode::ResetUnlatch => "Reset Output (R)",
                            CoilMode::PositiveTransition => "Pulse Output (P)",
                            CoilMode::NegativeTransition => "Pulse Output (N)",
                        },
                        CoilStyle::CircleRelay => "Relay Coil",
                        CoilStyle::NemaBox => "Motor Starter",
                        CoilStyle::SolenoidValve => "Solenoid Valve",
                        CoilStyle::PilotLight => "Pilot Lamp",
                    }
                };
                painter.text(
                    pos2(center.x, y0 + h - 12.0 * self.zoom),
                    Align2::CENTER_CENTER,
                    bottom_text,
                    self.font_prop(10.5),
                    palette.symbol_subtext,
                );
            }

            TypeTag::Ton
            | TypeTag::Tof
            | TypeTag::Tp
            | TypeTag::Ctu
            | TypeTag::Ctd
            | TypeTag::Ctud => {
                // --- Modern Function Block Card with Progress Bar ---
                let card_w = w - 18.0 * self.zoom;
                let card_h = h - 18.0 * self.zoom;
                let card_rect = Rect::from_center_size(center, vec2(card_w, card_h));

                self.draw_power_wire(
                    painter,
                    pos2(x0, mid_y),
                    pos2(card_rect.min.x, mid_y),
                    power_in && sim_active,
                    palette,
                );
                self.draw_power_wire(
                    painter,
                    pos2(card_rect.max.x, mid_y),
                    pos2(x0 + w, mid_y),
                    elem.is_energized && sim_active,
                    palette,
                );

                let is_done = elem.is_done && sim_active;
                let border_c = if is_done {
                    palette.accent_ok
                } else if power_in && sim_active {
                    palette.accent_warn
                } else {
                    palette.card_border
                };

                // Card Body
                painter.rect_filled(card_rect, 6.0 * self.zoom, palette.card_bg);

                // Header Band
                let hdr_rect = Rect::from_min_max(
                    card_rect.min,
                    pos2(card_rect.max.x, card_rect.min.y + 18.0 * self.zoom),
                );
                let hdr_bg = match elem.element_type {
                    TypeTag::Ton | TypeTag::Tof | TypeTag::Tp => Color32::from_rgb(24, 95, 65),
                    _ => Color32::from_rgb(55, 50, 115),
                };
                painter.rect_filled(
                    hdr_rect,
                    CornerRadius {
                        nw: (6.0 * self.zoom) as u8,
                        ne: (6.0 * self.zoom) as u8,
                        sw: 0,
                        se: 0,
                    },
                    hdr_bg,
                );

                let type_short = match elem.element_type {
                    TypeTag::Ton => "TON",
                    TypeTag::Tof => "TOF",
                    TypeTag::Tp => "TP",
                    TypeTag::Ctu => "CTU",
                    TypeTag::Ctd => "CTD",
                    _ => "CTUD",
                };
                painter.text(
                    hdr_rect.center(),
                    Align2::CENTER_CENTER,
                    format!("{} : {}", type_short, elem.name),
                    self.font_mono(10.5),
                    Color32::WHITE,
                );

                // Preset & Current Values
                let is_timer =
                    matches!(elem.element_type, TypeTag::Ton | TypeTag::Tof | TypeTag::Tp);
                let pv_text = if is_timer {
                    if elem.preset_value >= 1000 {
                        format!("PT: {:.1}s", elem.preset_value as f32 / 1000.0)
                    } else {
                        format!("PT: {}ms", elem.preset_value)
                    }
                } else {
                    format!("PV: {}", elem.preset_value)
                };
                let cv_text = if is_timer {
                    if elem.preset_value >= 1000 {
                        format!("ET: {:.1}s", elem.current_accum as f32 / 1000.0)
                    } else {
                        format!("ET: {}ms", elem.current_accum)
                    }
                } else {
                    format!("CV: {}", elem.current_accum)
                };

                painter.text(
                    pos2(center.x, card_rect.min.y + 30.0 * self.zoom),
                    Align2::CENTER_CENTER,
                    pv_text,
                    self.font_mono(9.5),
                    palette.symbol_subtext,
                );
                painter.text(
                    pos2(center.x, card_rect.min.y + 44.0 * self.zoom),
                    Align2::CENTER_CENTER,
                    cv_text,
                    self.font_mono(9.5),
                    if is_done {
                        palette.accent_ok
                    } else {
                        palette.accent_warn
                    },
                );

                // Progress Bar at bottom of card
                let prog_ratio = if elem.preset_value > 0 {
                    (elem.current_accum as f32 / elem.preset_value as f32).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let bar_bg = Rect::from_min_max(
                    pos2(
                        card_rect.min.x + 6.0 * self.zoom,
                        card_rect.max.y - 10.0 * self.zoom,
                    ),
                    pos2(
                        card_rect.max.x - 6.0 * self.zoom,
                        card_rect.max.y - 4.0 * self.zoom,
                    ),
                );
                painter.rect_filled(bar_bg, 2.0, palette.pill_bg);
                if prog_ratio > 0.0 {
                    let bar_fill = Rect::from_min_max(
                        bar_bg.min,
                        pos2(bar_bg.min.x + bar_bg.width() * prog_ratio, bar_bg.max.y),
                    );
                    painter.rect_filled(
                        bar_fill,
                        2.0,
                        if is_done {
                            palette.accent_ok
                        } else {
                            palette.accent_warn
                        },
                    );
                }

                // IEC 61131-3 Pin Labels: IN/CU on left, Q on right
                let in_pin_lbl = match elem.element_type {
                    TypeTag::Ctu => "CU",
                    TypeTag::Ctd => "CD",
                    TypeTag::Ctud => "CU/CD",
                    _ => "IN",
                };
                painter.text(
                    pos2(card_rect.min.x + 8.0 * self.zoom, mid_y),
                    Align2::LEFT_CENTER,
                    in_pin_lbl,
                    self.font_mono(8.0),
                    if power_in && sim_active { palette.wire_energized } else { palette.rung_text_dim },
                );
                painter.text(
                    pos2(card_rect.max.x - 8.0 * self.zoom, mid_y),
                    Align2::RIGHT_CENTER,
                    "Q",
                    self.font_mono(8.0),
                    if elem.is_energized && sim_active { palette.wire_energized } else { palette.rung_text_dim },
                );

                painter.rect_stroke(
                    card_rect,
                    6.0 * self.zoom,
                    Stroke::new(1.6 * self.zoom, border_c),
                    StrokeKind::Inside,
                );
            }

            TypeTag::RTrig | TypeTag::FTrig => {
                // --- IEC 61131-3 Edge Detection Contacts: —[ P ]— and —[ N ]— ---
                let gap_half = 11.0 * self.zoom;
                let bar_half_h = 15.0 * self.zoom;
                let left_bar_x = center.x - gap_half;
                let right_bar_x = center.x + gap_half;
                let is_energized = elem.is_energized && sim_active;
                let is_closed = elem.is_done && sim_active;

                let contact_color = if is_energized {
                    palette.wire_energized
                } else if is_closed {
                    palette.symbol_closed
                } else if is_selected {
                    palette.rung_text_sel
                } else {
                    palette.symbol_idle
                };

                // Lead-in & Lead-out Wires
                self.draw_power_wire(painter, pos2(x0, mid_y), pos2(left_bar_x, mid_y), power_in && sim_active, palette);
                self.draw_power_wire(painter, pos2(right_bar_x, mid_y), pos2(x0 + w, mid_y), is_energized, palette);

                // Conduction glow between plates
                if is_closed {
                    let glow_rect = Rect::from_min_max(
                        pos2(left_bar_x + 2.0 * self.zoom, mid_y - bar_half_h * 0.75),
                        pos2(right_bar_x - 2.0 * self.zoom, mid_y + bar_half_h * 0.75),
                    );
                    painter.rect_filled(
                        glow_rect,
                        3.0 * self.zoom,
                        if is_energized { palette.wire_glow } else { Color32::from_rgba_unmultiplied(45, 200, 95, 65) },
                    );
                }

                // Two Vertical Parallel Plates
                painter.line_segment(
                    [pos2(left_bar_x, mid_y - bar_half_h), pos2(left_bar_x, mid_y + bar_half_h)],
                    Stroke::new(3.0 * self.zoom, contact_color),
                );
                painter.line_segment(
                    [pos2(right_bar_x, mid_y - bar_half_h), pos2(right_bar_x, mid_y + bar_half_h)],
                    Stroke::new(3.0 * self.zoom, contact_color),
                );

                // Center Edge Marker letter P or N
                let edge_letter = if elem.element_type == TypeTag::RTrig { "P" } else { "N" };
                painter.text(
                    center,
                    Align2::CENTER_CENTER,
                    edge_letter,
                    self.font_mono(14.0),
                    contact_color,
                );

                // Top Address Pill
                self.draw_address_pill(
                    painter,
                    pos2(center.x, y0 + 14.0 * self.zoom),
                    &elem.name,
                    badge_color,
                    is_closed,
                    palette,
                );

                // Bottom Symbol Tag
                let bottom_text = if !elem.symbol.is_empty() && elem.symbol != elem.name {
                    elem.symbol.as_str()
                } else if elem.element_type == TypeTag::RTrig {
                    "Rising Edge R_TRIG (P)"
                } else {
                    "Falling Edge F_TRIG (N)"
                };
                painter.text(
                    pos2(center.x, y0 + h - 12.0 * self.zoom),
                    Align2::CENTER_CENTER,
                    bottom_text,
                    self.font_prop(10.5),
                    palette.symbol_subtext,
                );
            }

            TypeTag::Compare
            | TypeTag::Move
            | TypeTag::Math
            | TypeTag::Adc
            | TypeTag::MasterRelay
            | TypeTag::ShiftRegisters
            | TypeTag::ShiftVariable
            | TypeTag::ShiftLeft
            | TypeTag::ShiftRight
            | TypeTag::ShiftRegReset
            | TypeTag::ShiftRegInbit
            | TypeTag::UartSend
            | TypeTag::UartRecv
            | TypeTag::Pid
            | TypeTag::Scale
            | TypeTag::Limit
            | TypeTag::Bitwise
            | TypeTag::MbSlave
            | TypeTag::Pwm
            | TypeTag::Select
            | TypeTag::InRange
            | TypeTag::Toggle
            | TypeTag::IncDec
            | TypeTag::Return => {
                // Function Block / Instruction Card
                let card_w = w - 18.0 * self.zoom;
                let card_h = h - 20.0 * self.zoom;
                let card_rect = Rect::from_center_size(center, vec2(card_w, card_h));

                self.draw_power_wire(
                    painter,
                    pos2(x0, mid_y),
                    pos2(card_rect.min.x, mid_y),
                    power_in && sim_active,
                    palette,
                );
                self.draw_power_wire(
                    painter,
                    pos2(card_rect.max.x, mid_y),
                    pos2(x0 + w, mid_y),
                    elem.is_energized && sim_active,
                    palette,
                );

                painter.rect_filled(card_rect, 5.0 * self.zoom, palette.card_bg);

                let (hdr, line1, line2) = match elem.element_type {
                    TypeTag::Compare => (
                        "CMP".to_string(),
                        format!("{} {} {}", elem.name, elem.compare_op, elem.preset_value),
                        if sim_active {
                            format!("VAL: {}", elem.current_accum)
                        } else {
                            elem.symbol.clone()
                        },
                    ),
                    TypeTag::Move => {
                        let reg = elem.word_target();
                        (
                            "MOVE".to_string(),
                            format!("{} := {}", reg, elem.preset_value),
                            if sim_active {
                                format!("OUT: {}", elem.current_accum)
                            } else {
                                elem.symbol.clone()
                            },
                        )
                    }
                    TypeTag::Math => {
                        let reg = elem.word_target();
                        (
                            format!("MATH ({})", elem.math_op),
                            format!("{} {}= {}", reg, elem.math_op, elem.preset_value),
                            if sim_active {
                                format!("OUT: {}", elem.current_accum)
                            } else {
                                elem.symbol.clone()
                            },
                        )
                    }
                    TypeTag::Adc => {
                        let dst = if elem.target_reg.is_empty() {
                            "ACC"
                        } else {
                            elem.target_reg.as_str()
                        };
                        (
                            format!("ADC : {}", elem.name),
                            format!("{} -> {}", elem.name, dst),
                            if sim_active {
                                format!("RAW: {}", elem.current_accum)
                            } else {
                                elem.symbol.clone()
                            },
                        )
                    }
                    TypeTag::MasterRelay => (
                        "MCR".to_string(),
                        "Master Relay".to_string(),
                        elem.symbol.clone(),
                    ),
                    TypeTag::UartSend => {
                        let crlf = if elem.fb.mode != 0 { "+CRLF" } else { "RAW" };
                        (
                            format!("UART TX : {}", elem.name),
                            format!("IN:{} ({})", elem.fb.in1, crlf),
                            if sim_active {
                                format!("LAST: {}", elem.current_accum)
                            } else {
                                elem.symbol.clone()
                            },
                        )
                    }
                    TypeTag::UartRecv => (
                        format!("UART RX : {}", elem.name),
                        format!("OUT: {}", elem.target_reg),
                        if sim_active {
                            format!("RX: {}", elem.current_accum)
                        } else {
                            elem.symbol.clone()
                        },
                    ),
                    TypeTag::Pid => {
                        let [kp, ki, kd, _] = elem.fb.k;
                        (
                            format!("PID : {}", elem.name),
                            format!("PV:{} SP:{}", elem.fb.in1, elem.fb.in2),
                            if sim_active {
                                format!("OUT({}): {}", elem.target_reg, elem.current_accum)
                            } else {
                                format!(
                                    "OUT:{} P:{:.1} I:{:.1} D:{:.1}",
                                    elem.target_reg,
                                    kp as f32 / 100.0,
                                    ki as f32 / 100.0,
                                    kd as f32 / 100.0
                                )
                            },
                        )
                    }
                    TypeTag::Scale => {
                        let [imin, imax, omin, omax] = elem.fb.k;
                        (
                            format!("SCALE : {}", elem.name),
                            format!("{} -> {}", elem.fb.in1, elem.target_reg),
                            if sim_active {
                                format!("OUT: {}", elem.current_accum)
                            } else {
                                format!("[{}..{}]->[{}..{}]", imin, imax, omin, omax)
                            },
                        )
                    }
                    TypeTag::Limit => {
                        let [mn, mx, _, _] = elem.fb.k;
                        (
                            format!("LIMIT : {}", elem.name),
                            format!("{} -> {}", elem.fb.in1, elem.target_reg),
                            if sim_active {
                                format!("OUT: {}", elem.current_accum)
                            } else {
                                format!("MIN:{} MAX:{}", mn, mx)
                            },
                        )
                    }
                    TypeTag::Bitwise => {
                        let op = if elem.math_op.is_empty() {
                            "AND"
                        } else {
                            elem.math_op.as_str()
                        };
                        let expr = if op.eq_ignore_ascii_case("NOT") {
                            format!("~{} -> {}", elem.fb.in1, elem.target_reg)
                        } else {
                            format!("{} {} {}", elem.fb.in1, op, elem.fb.in2)
                        };
                        (
                            format!("BIT {} : {}", op, elem.target_reg),
                            expr,
                            if sim_active {
                                format!(
                                    "0x{:04X} ({})",
                                    (elem.current_accum as u16), elem.current_accum
                                )
                            } else {
                                elem.symbol.clone()
                            },
                        )
                    }
                    TypeTag::ShiftRegisters => (
                        format!("SFT : {}", elem.name),
                        format!("LEN:{} IN:{}", elem.preset_value.max(1), elem.fb.in1),
                        if sim_active {
                            format!("HEAD: {}", elem.current_accum)
                        } else {
                            elem.symbol.clone()
                        },
                    ),
                    TypeTag::ShiftRegInbit => (
                        format!("SBIT : {}", elem.name),
                        format!("BIT: {}", elem.fb.in1),
                        if sim_active {
                            format!("0x{:04X}", elem.current_accum as u16)
                        } else {
                            elem.symbol.clone()
                        },
                    ),
                    TypeTag::ShiftRegReset => (
                        format!("SRST : {}", elem.name),
                        "CLR := 0".to_string(),
                        elem.symbol.clone(),
                    ),
                    TypeTag::ShiftLeft => (
                        format!("SHL : {}", elem.name),
                        format!("<< {} bits", elem.preset_value),
                        if sim_active {
                            format!("0x{:04X}", elem.current_accum as u16)
                        } else {
                            elem.symbol.clone()
                        },
                    ),
                    TypeTag::ShiftRight => (
                        format!("SHR : {}", elem.name),
                        format!(">> {} bits", elem.preset_value),
                        if sim_active {
                            format!("0x{:04X}", elem.current_accum as u16)
                        } else {
                            elem.symbol.clone()
                        },
                    ),
                    TypeTag::ShiftVariable => (
                        format!("SHV : {}", elem.name),
                        format!("CNT: {}", elem.fb.in1),
                        if sim_active {
                            format!("0x{:04X}", elem.current_accum as u16)
                        } else {
                            elem.symbol.clone()
                        },
                    ),
                    TypeTag::MbSlave => (
                        format!("MB_SLAVE : {}", elem.name),
                        format!("ID:{} {}bps", elem.mb_slave_id, elem.preset_value),
                        elem.symbol.clone(),
                    ),
                    TypeTag::Pwm => (
                        format!("PWM : {}", elem.name),
                        format!("IN:{} / {}", elem.fb.in1, elem.preset_value.max(1)),
                        if sim_active {
                            format!("DUTY: {}", elem.current_accum)
                        } else {
                            elem.symbol.clone()
                        },
                    ),
                    TypeTag::Select => {
                        let dst = if elem.target_reg.is_empty() { &elem.name } else { &elem.target_reg };
                        (
                            format!("SEL : {}", dst),
                            format!("0:{} 1:{}", elem.fb.in1, elem.fb.in2),
                            if sim_active {
                                format!("OUT: {}", elem.current_accum)
                            } else {
                                elem.symbol.clone()
                            },
                        )
                    }
                    TypeTag::InRange => (
                        "IN_RANGE".to_string(),
                        format!("{} <= {} <= {}", elem.fb.k[0], elem.fb.in1, elem.fb.k[1]),
                        if sim_active {
                            format!("VAL: {}", elem.current_accum)
                        } else {
                            elem.symbol.clone()
                        },
                    ),
                    TypeTag::Toggle => (
                        "ALT (TOGGLE)".to_string(),
                        elem.name.clone(),
                        elem.symbol.clone(),
                    ),
                    TypeTag::IncDec => (
                        if elem.fb.mode != 0 { "DEC (-1)" } else { "INC (+1)" }.to_string(),
                        elem.name.clone(),
                        if sim_active {
                            format!("VAL: {}", elem.current_accum)
                        } else {
                            elem.symbol.clone()
                        },
                    ),
                    TypeTag::Return => (
                        "RET".to_string(),
                        "Early Return".to_string(),
                        elem.symbol.clone(),
                    ),
                    _ => ("BLOCK".to_string(), elem.name.clone(), elem.symbol.clone()),
                };

                painter.rect_stroke(
                    card_rect,
                    5.0 * self.zoom,
                    Stroke::new(
                        1.5 * self.zoom,
                        if elem.is_energized && sim_active {
                            palette.wire_energized
                        } else {
                            palette.card_border
                        },
                    ),
                    StrokeKind::Inside,
                );

                // IEC 61131-3 Pin Labels: EN on left, ENO on right
                painter.text(
                    pos2(card_rect.min.x + 5.0 * self.zoom, mid_y),
                    Align2::LEFT_CENTER,
                    "EN",
                    self.font_mono(7.0),
                    if power_in && sim_active { palette.wire_energized } else { palette.rung_text_dim },
                );
                painter.text(
                    pos2(card_rect.max.x - 5.0 * self.zoom, mid_y),
                    Align2::RIGHT_CENTER,
                    "ENO",
                    self.font_mono(7.0),
                    if elem.is_energized && sim_active { palette.wire_energized } else { palette.rung_text_dim },
                );

                let clip_rect = card_rect.shrink(3.0 * self.zoom);
                let full_max_w = (card_w - 8.0 * self.zoom).max(8.0);
                let mid_max_w = (card_w - 36.0 * self.zoom).max(8.0);

                self.draw_fitted_card_text(
                    painter,
                    clip_rect,
                    pos2(center.x, card_rect.min.y + 11.0 * self.zoom),
                    &hdr,
                    9.5,
                    full_max_w,
                    palette.accent_heading,
                );
                self.draw_fitted_card_text(
                    painter,
                    clip_rect,
                    pos2(center.x, card_rect.center().y + 1.0 * self.zoom),
                    &line1,
                    8.8,
                    mid_max_w,
                    palette.text_primary,
                );
                if !line2.is_empty() {
                    self.draw_fitted_card_text(
                        painter,
                        clip_rect,
                        pos2(center.x, card_rect.max.y - 10.0 * self.zoom),
                        &line2,
                        8.2,
                        full_max_w,
                        if sim_active {
                            palette.accent_ok
                        } else {
                            palette.symbol_subtext
                        },
                    );
                }
            }

            TypeTag::Wire => {
                self.draw_power_wire(
                    painter,
                    pos2(x0, mid_y),
                    pos2(x0 + w, mid_y),
                    power_in && sim_active,
                    palette,
                );
            }
            TypeTag::OpenCircuit => {
                self.draw_power_wire(
                    painter,
                    pos2(x0, mid_y),
                    pos2(x0 + w * 0.3, mid_y),
                    power_in && sim_active,
                    palette,
                );
                self.draw_power_wire(
                    painter,
                    pos2(x0 + w * 0.7, mid_y),
                    pos2(x0 + w, mid_y),
                    false,
                    palette,
                );
                painter.circle_stroke(
                    pos2(x0 + w * 0.3, mid_y),
                    3.5 * self.zoom,
                    Stroke::new(1.5 * self.zoom, palette.accent_danger),
                );
                painter.circle_stroke(
                    pos2(x0 + w * 0.7, mid_y),
                    3.5 * self.zoom,
                    Stroke::new(1.5 * self.zoom, palette.accent_danger),
                );
            }
            TypeTag::Comment => {
                painter.text(
                    center,
                    Align2::CENTER_CENTER,
                    format!("// {}", elem.comment),
                    self.font_prop(11.0),
                    palette.comment_fg,
                );
            }
            TypeTag::Sr | TypeTag::Rs => {
                let card_w = w - 16.0 * self.zoom;
                let card_h = h - 22.0 * self.zoom;
                let card_rect = Rect::from_center_size(center, vec2(card_w, card_h));
                let is_on = elem.is_done && sim_active;
                let is_energized = elem.is_energized && sim_active;

                self.draw_power_wire(
                    painter,
                    pos2(x0, mid_y),
                    pos2(card_rect.min.x, mid_y),
                    power_in && sim_active,
                    palette,
                );
                self.draw_power_wire(
                    painter,
                    pos2(card_rect.max.x, mid_y),
                    pos2(x0 + w, mid_y),
                    is_energized,
                    palette,
                );

                let border_c = if is_energized {
                    palette.wire_energized
                } else if is_selected {
                    palette.rung_text_sel
                } else {
                    palette.card_border
                };

                painter.rect_filled(card_rect, 5.0 * self.zoom, palette.card_bg);
                painter.rect_stroke(card_rect, 5.0 * self.zoom, Stroke::new(1.6 * self.zoom, border_c), StrokeKind::Inside);

                let title = if elem.element_type == TypeTag::Sr { "SR" } else { "RS" };
                painter.text(
                    pos2(center.x, card_rect.min.y + 12.0 * self.zoom),
                    Align2::CENTER_CENTER,
                    title,
                    self.font_mono(11.0),
                    palette.accent_warn,
                );

                painter.text(
                    pos2(center.x, center.y + 2.0 * self.zoom),
                    Align2::CENTER_CENTER,
                    &elem.name,
                    self.font_mono(10.0),
                    if is_on { palette.wire_energized } else { palette.text_primary },
                );

                // Pin labels: Left inputs (S1/R or S/R1), Right output (Q1)
                let s_lbl = if elem.element_type == TypeTag::Sr { "S1" } else { "S" };
                painter.text(
                    pos2(card_rect.min.x + 6.0 * self.zoom, card_rect.min.y + 12.0 * self.zoom),
                    Align2::LEFT_CENTER,
                    s_lbl,
                    self.font_mono(8.0),
                    if power_in && sim_active { palette.wire_energized } else { palette.rung_text_dim },
                );

                let r_tag = if !elem.target_reg.is_empty() { &elem.target_reg } else { "R" };
                let r_lbl = if elem.element_type == TypeTag::Sr { format!("R:{}", r_tag) } else { format!("R1:{}", r_tag) };
                painter.text(
                    pos2(card_rect.min.x + 6.0 * self.zoom, card_rect.max.y - 10.0 * self.zoom),
                    Align2::LEFT_CENTER,
                    r_lbl,
                    self.font_mono(7.5),
                    palette.rung_text_dim,
                );

                painter.text(
                    pos2(card_rect.max.x - 6.0 * self.zoom, mid_y),
                    Align2::RIGHT_CENTER,
                    "Q1",
                    self.font_mono(8.5),
                    if is_energized { palette.wire_energized } else { palette.rung_text_dim },
                );

                self.draw_address_pill(
                    painter,
                    pos2(center.x, y0 + 14.0 * self.zoom),
                    &elem.name,
                    badge_color,
                    is_on,
                    palette,
                );
            }
            TypeTag::Jump => {
                let is_active = elem.is_energized && sim_active;
                let jump_w = 68.0 * self.zoom;
                let jump_h = 24.0 * self.zoom;
                let jump_rect = Rect::from_center_size(center, vec2(jump_w, jump_h));

                self.draw_power_wire(
                    painter,
                    pos2(x0, mid_y),
                    pos2(jump_rect.min.x, mid_y),
                    power_in && sim_active,
                    palette,
                );
                self.draw_power_wire(
                    painter,
                    pos2(jump_rect.max.x, mid_y),
                    pos2(x0 + w, mid_y),
                    is_active,
                    palette,
                );

                let bg = if is_active {
                    palette.wire_glow
                } else {
                    palette.card_bg
                };
                let border_c = if is_active {
                    palette.wire_energized
                } else if is_selected {
                    palette.rung_text_sel
                } else {
                    palette.card_border
                };

                painter.rect_filled(jump_rect, 4.0 * self.zoom, bg);
                painter.rect_stroke(jump_rect, 4.0 * self.zoom, Stroke::new(1.5 * self.zoom, border_c), StrokeKind::Inside);

                let txt = format!(">> JMP {}", elem.name);
                painter.text(
                    center,
                    Align2::CENTER_CENTER,
                    txt,
                    self.font_mono(10.0),
                    if is_active { palette.wire_energized } else { palette.accent_warn },
                );
            }
            TypeTag::Label => {
                let is_on = sim_active && power_in;
                // Full pass-through power wire
                self.draw_power_wire(
                    painter,
                    pos2(x0, mid_y),
                    pos2(x0 + w, mid_y),
                    is_on,
                    palette,
                );

                // Left Rail Anchor Flag Badge
                let badge_w = (elem.name.len().max(3) as f32 * 7.5 + 28.0) * self.zoom;
                let badge_h = 24.0 * self.zoom;
                let badge_rect = Rect::from_center_size(center, vec2(badge_w, badge_h));

                let bg = if is_selected {
                    palette.pill_active_bg
                } else if is_hovered {
                    palette.pill_bg.gamma_multiply(1.2)
                } else {
                    palette.pill_bg
                };
                let border_col = if is_selected {
                    palette.rung_text_sel
                } else {
                    palette.accent_heading
                };

                painter.rect_filled(badge_rect, 4.0 * self.zoom, bg);
                painter.rect_stroke(
                    badge_rect,
                    4.0 * self.zoom,
                    Stroke::new(1.4 * self.zoom, border_col),
                    StrokeKind::Inside,
                );

                let txt = format!("LBL {}", elem.name);
                painter.text(
                    center,
                    Align2::CENTER_CENTER,
                    txt,
                    self.font_mono(10.5),
                    if is_selected { Color32::WHITE } else { palette.accent_heading },
                );

                // Top address pill
                self.draw_address_pill(
                    painter,
                    pos2(center.x, y0 + 14.0 * self.zoom),
                    "LABEL",
                    palette.accent_heading,
                    is_on,
                    palette,
                );

                // Bottom Subtext
                painter.text(
                    pos2(center.x, y0 + h - 12.0 * self.zoom),
                    Align2::CENTER_CENTER,
                    "Target Landing",
                    self.font_prop(10.0),
                    palette.symbol_subtext,
                );
            }
            TypeTag::MbMaster => {
                let card_w = w - 18.0 * self.zoom;
                let card_h = h - 18.0 * self.zoom;
                let card_rect = Rect::from_center_size(center, vec2(card_w, card_h));

                self.draw_power_wire(
                    painter,
                    pos2(x0, mid_y),
                    pos2(card_rect.min.x, mid_y),
                    power_in && sim_active,
                    palette,
                );
                self.draw_power_wire(
                    painter,
                    pos2(card_rect.max.x, mid_y),
                    pos2(x0 + w, mid_y),
                    elem.is_energized && sim_active,
                    palette,
                );

                let is_on = elem.is_energized && sim_active;
                let border_c = if is_on {
                    palette.accent_ok
                } else if is_selected {
                    palette.accent_heading
                } else {
                    palette.card_border
                };

                painter.rect_filled(card_rect, 6.0 * self.zoom, palette.card_bg);
                painter.rect_stroke(
                    card_rect,
                    6.0 * self.zoom,
                    Stroke::new(1.5 * self.zoom, border_c),
                    StrokeKind::Inside,
                );

                // Header Band
                let hdr_rect = Rect::from_min_max(
                    card_rect.min,
                    pos2(card_rect.max.x, card_rect.min.y + 18.0 * self.zoom),
                );
                let hdr_bg = Color32::from_rgb(180, 85, 20); // Amber/copper industrial color for Modbus
                painter.rect_filled(
                    hdr_rect,
                    CornerRadius {
                        nw: (6.0 * self.zoom) as u8,
                        ne: (6.0 * self.zoom) as u8,
                        sw: 0,
                        se: 0,
                    },
                    hdr_bg,
                );
                painter.text(
                    hdr_rect.center(),
                    Align2::CENTER_CENTER,
                    "MB_MASTER",
                    self.font_mono(10.0),
                    Color32::WHITE,
                );

                let clip_rect = card_rect.shrink(3.0 * self.zoom);
                let max_w = (card_w - 8.0 * self.zoom).max(8.0);

                // Center: Instance name
                self.draw_fitted_card_text(
                    painter,
                    clip_rect,
                    pos2(center.x, center.y - 4.0 * self.zoom),
                    &elem.name,
                    13.0,
                    max_w,
                    if is_selected {
                        if palette.is_light() {
                            palette.accent_heading
                        } else {
                            Color32::WHITE
                        }
                    } else {
                        palette.rung_text_num
                    },
                );

                // Bottom: Slave ID, Func, Buffer
                let subtext = format!("ID:{} FC:{} -> {}", elem.mb_slave_id, elem.mb_function, elem.target_reg);
                self.draw_fitted_card_text(
                    painter,
                    clip_rect,
                    pos2(center.x, y0 + h - 14.0 * self.zoom),
                    &subtext,
                    9.5,
                    max_w,
                    palette.symbol_subtext,
                );
            }
            TypeTag::Branch => {}
        }
    }

    fn draw_address_pill(
        &self,
        painter: &Painter,
        center_pos: Pos2,
        text: &str,
        accent: Color32,
        is_active: bool,
        palette: ThemePalette,
    ) {
        let pill_w = (text.len().max(3) as f32 * 7.5 + 16.0) * self.zoom;
        let pill_h = 16.5 * self.zoom;
        let pill_rect = Rect::from_center_size(center_pos, vec2(pill_w, pill_h));

        let bg = if is_active {
            palette.pill_active_bg
        } else {
            palette.pill_bg
        };
        painter.rect_filled(pill_rect, 4.0 * self.zoom, bg);
        painter.rect_stroke(
            pill_rect,
            4.0 * self.zoom,
            Stroke::new(1.0 * self.zoom, accent.gamma_multiply(0.75)),
            StrokeKind::Inside,
        );
        painter.text(
            center_pos,
            Align2::CENTER_CENTER,
            text,
            self.font_mono(10.5),
            if is_active { Color32::WHITE } else { accent },
        );
    }

    fn draw_grid(&self, painter: &Painter, viewport: Rect, origin: Pos2, dot_color: Color32) {
        let mut grid_size = 27.0 * self.zoom;
        if grid_size < 18.0 {
            grid_size *= 2.0;
        }
        if grid_size < 10.0 {
            return;
        }

        let start_x = (viewport.min.x - origin.x - self.pan_offset.x).rem_euclid(grid_size);
        let start_y = (viewport.min.y - origin.y - self.pan_offset.y).rem_euclid(grid_size);
        let dot_sz = vec2(2.0, 2.0);

        let mut x = viewport.min.x + (grid_size - start_x);
        while x < viewport.max.x {
            let mut y = viewport.min.y + (grid_size - start_y);
            while y < viewport.max.y {
                painter.rect_filled(Rect::from_center_size(pos2(x, y), dot_sz), 0.0, dot_color);
                y += grid_size;
            }
            x += grid_size;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use my_canvas_core::{CoilMode, ContactMode, LadderElement, TypeTag};

    #[test]
    fn test_validate_placement_rules() {
        let contact = LadderElement::new_contact(1, "X0", "X0", ContactMode::NormallyOpen);
        let coil = LadderElement::new_coil(2, "Y0", "Y0", CoilMode::Normal);
        let jmp = LadderElement {
            id: 3,
            element_type: TypeTag::Jump,
            name: "LBL1".to_string(),
            ..Default::default()
        };
        let ton = LadderElement::new_ton(4, "T0", "T0", 1000);

        let ctx_first = PlacementContext {
            target_is_first: true,
            target_is_last: false,
            in_branch: false,
            has_downstream_after_branch: false,
        };
        let ctx_mid = PlacementContext {
            target_is_first: false,
            target_is_last: false,
            in_branch: false,
            has_downstream_after_branch: false,
        };
        let ctx_last = PlacementContext {
            target_is_first: false,
            target_is_last: true,
            in_branch: false,
            has_downstream_after_branch: false,
        };

        // Rule 1 (R01): Label [LBL]
        assert!(validate_placement(TypeTag::Label, &contact, InsertPosition::Left, ctx_first).is_ok());
        assert_eq!(
            validate_placement(TypeTag::Label, &contact, InsertPosition::Left, ctx_mid).unwrap_err().rule_id,
            "R01"
        );
        assert_eq!(
            validate_placement(TypeTag::Label, &contact, InsertPosition::Right, ctx_first).unwrap_err().rule_id,
            "R01"
        );
        assert_eq!(
            validate_placement(TypeTag::Label, &contact, InsertPosition::TopParallel, ctx_first).unwrap_err().rule_id,
            "R01"
        );

        // Rule 2 (R01): Cannot place any components after Jump [JMP] or Return [RET]
        assert_eq!(
            validate_placement(TypeTag::Contact, &jmp, InsertPosition::Right, ctx_last).unwrap_err().rule_id,
            "R01"
        );
        assert!(validate_placement(TypeTag::Contact, &jmp, InsertPosition::Left, ctx_last).is_ok());

        // Rule 3 (STRICT_EDITOR R07/R08/R09/R10): Output Coils terminate the branch
        // Dropping coil to the right or left of coil -> STRICT R08 ERROR (no auto-convert)
        assert_eq!(
            validate_placement(TypeTag::Coil, &coil, InsertPosition::Right, ctx_last).unwrap_err().rule_id,
            "R08"
        );
        assert_eq!(
            validate_placement(TypeTag::Coil, &coil, InsertPosition::Left, ctx_last).unwrap_err().rule_id,
            "R08"
        );
        // Dropping coil on TopParallel / BottomParallel of coil -> ALLOWED (R07 parallel output branch)
        assert_eq!(
            validate_placement(TypeTag::Coil, &coil, InsertPosition::TopParallel, ctx_last).unwrap(),
            InsertPosition::TopParallel
        );
        assert_eq!(
            validate_placement(TypeTag::Coil, &coil, InsertPosition::BottomParallel, ctx_last).unwrap(),
            InsertPosition::BottomParallel
        );
        // Branch after coil -> R10 ERROR
        assert_eq!(
            validate_placement(TypeTag::Branch, &coil, InsertPosition::Right, ctx_last).unwrap_err().rule_id,
            "R10"
        );
        // Contact cannot be placed to the right of coil -> R09 ERROR
        assert_eq!(
            validate_placement(TypeTag::Contact, &coil, InsertPosition::Right, ctx_last).unwrap_err().rule_id,
            "R09"
        );
        // Contact cannot parallel across coil -> R07 ERROR
        assert_eq!(
            validate_placement(TypeTag::Contact, &coil, InsertPosition::BottomParallel, ctx_last).unwrap_err().rule_id,
            "R07"
        );

        // Rule 4 (R06/R07): Cannot parallel an output coil across an input contact
        assert_eq!(
            validate_placement(TypeTag::Coil, &contact, InsertPosition::BottomParallel, ctx_first).unwrap_err().rule_id,
            "R07"
        );
        assert_eq!(
            validate_placement(TypeTag::Coil, &contact, InsertPosition::TopParallel, ctx_first).unwrap_err().rule_id,
            "R07"
        );
        assert_eq!(
            validate_placement(TypeTag::Coil, &contact, InsertPosition::Left, ctx_first).unwrap_err().rule_id,
            "R06"
        );

        // Rule 5 (R06): Cannot place an output coil before a function block
        assert_eq!(
            validate_placement(TypeTag::Coil, &ton, InsertPosition::Left, ctx_mid).unwrap_err().rule_id,
            "R06"
        );

        // Non-Coil right-aligned outputs (e.g. UartSend, MbMaster) vs Contact & Coil chaining
        let mut uart_tx = LadderElement::new_wire(6);
        uart_tx.element_type = TypeTag::UartSend;
        let mut uart_rx = LadderElement::new_wire(7);
        uart_rx.element_type = TypeTag::UartRecv;
        assert_eq!(
            validate_placement(TypeTag::Contact, &uart_tx, InsertPosition::Right, ctx_last).unwrap_err().rule_id,
            "R09"
        );
        assert_eq!(
            validate_placement(TypeTag::UartSend, &contact, InsertPosition::Left, ctx_first).unwrap_err().rule_id,
            "R06"
        );
        // Chaining Coil after an output FB (e.g. UartSend / MbMaster -> Coil) remains allowed at terminal position
        assert!(validate_placement(TypeTag::Coil, &uart_tx, InsertPosition::Right, ctx_last).is_ok());
        // Cannot place Coil before UartRecv
        assert_eq!(
            validate_placement(TypeTag::Coil, &uart_rx, InsertPosition::Left, ctx_mid).unwrap_err().rule_id,
            "R06"
        );

        // Rule R21: Coil inside branch before merge with downstream elements is blocked
        let ctx_branch_before_merge = PlacementContext {
            target_is_first: true,
            target_is_last: true,
            in_branch: true,
            has_downstream_after_branch: true,
        };
        assert_eq!(
            validate_placement(TypeTag::Coil, &contact, InsertPosition::Right, ctx_branch_before_merge)
                .unwrap_err()
                .rule_id,
            "R21"
        );
        assert_eq!(
            validate_placement(TypeTag::Coil, &coil, InsertPosition::BottomParallel, ctx_branch_before_merge)
                .unwrap_err()
                .rule_id,
            "R21"
        );
        // Rule R21: Coil placed right of a non-last contact (before downstream elements) is blocked
        assert_eq!(
            validate_placement(TypeTag::Coil, &contact, InsertPosition::Right, ctx_first)
                .unwrap_err()
                .rule_id,
            "R21"
        );
        // Coil placed right of the last contact (no downstream elements) is allowed
        assert!(validate_placement(TypeTag::Coil, &contact, InsertPosition::Right, ctx_last).is_ok());
    }

    #[test]
    fn test_validate_rung_append_rules() {
        let mut rung = Rung::new(0);
        // Empty rung accepts Label or Coil
        assert!(validate_rung_append(TypeTag::Label, &rung).is_ok());
        assert!(validate_rung_append(TypeTag::Coil, &rung).is_ok());

        // Non-empty rung with Coil blocks appending another Coil (R08) and Label (R01)
        rung.elements.push(LadderElement::new_contact(1, "%IX0.0", "A", ContactMode::NormallyOpen));
        rung.elements.push(LadderElement::new_coil(2, "%QX0.0", "Q1", CoilMode::Normal));
        assert_eq!(validate_rung_append(TypeTag::Label, &rung).unwrap_err().rule_id, "R01");
        assert_eq!(validate_rung_append(TypeTag::Coil, &rung).unwrap_err().rule_id, "R08");

        // Rung ending in JMP blocks appending anything (R01)
        let mut jmp_rung = Rung::new(1);
        jmp_rung.elements.push(LadderElement {
            id: 3,
            element_type: TypeTag::Jump,
            name: "L1".to_string(),
            ..Default::default()
        });
        assert_eq!(validate_rung_append(TypeTag::Contact, &jmp_rung).unwrap_err().rule_id, "R01");

        // R21: Rung ending in a Branch containing a Coil blocks appending downstream elements
        let mut branch_coil_rung = Rung::new(2);
        branch_coil_rung.elements.push(LadderElement::new_branch(
            10,
            vec![
                vec![
                    LadderElement::new_contact(11, "%IX0.0", "A", ContactMode::NormallyOpen),
                    LadderElement::new_coil(12, "%QX0.1", "Q1", CoilMode::Normal),
                ],
                vec![LadderElement::new_contact(
                    13,
                    "%IX0.1",
                    "B",
                    ContactMode::NormallyOpen,
                )],
            ],
        ));
        assert_eq!(
            validate_rung_append(TypeTag::Coil, &branch_coil_rung).unwrap_err().rule_id,
            "R21"
        );
        assert_eq!(
            validate_rung_append(TypeTag::Contact, &branch_coil_rung).unwrap_err().rule_id,
            "R21"
        );
    }

    #[test]
    fn test_drop_target_create_new_rung() {
        let dt = DropTarget {
            rung_idx: 2,
            target_elem_id: None,
            insert_pos: InsertPosition::AppendRung,
            target_label: "New Network #03".to_string(),
            validation_rule_id: None,
            validation_error: None,
            create_new_rung: true,
        };
        assert!(dt.create_new_rung);
        assert_eq!(dt.rung_idx, 2);
        assert!(dt.validation_error.is_none());
    }

    #[test]
    fn test_fit_to_viewport_clamps_zoom() {
        let doc = Document::default();
        let mut vp = CanvasViewport::default();
        vp.pan_offset = vec2(-220.0, -140.0);
        vp.zoom = 1.9;
        vp.target_zoom = 1.9;
        vp.zoom_anchor = Some(pos2(100.0, 100.0));

        // Very small viewport should clamp to lower bound (0.45) and reset pan_offset
        vp.fit_to_viewport(&doc, vec2(260.0, 160.0));
        assert!(
            (0.45..=1.25).contains(&vp.zoom),
            "expected clamped zoom in 0.45..=1.25, got {}",
            vp.zoom
        );
        assert!((vp.zoom - 0.45).abs() < 1e-5, "expected lower clamp 0.45, got {}", vp.zoom);
        assert_eq!(vp.target_zoom, vp.zoom);
        assert!(vp.zoom_anchor.is_none());
        assert_eq!(vp.pan_offset, vec2(14.0, 16.0));

        // Large viewport should stay within 0.45..=1.25 and reset pan_offset
        vp.pan_offset = vec2(-90.0, -50.0);
        vp.fit_to_viewport(&doc, vec2(2400.0, 1600.0));
        assert!(
            (0.45..=1.25).contains(&vp.zoom),
            "expected clamped zoom in 0.45..=1.25, got {}",
            vp.zoom
        );
        assert_eq!(vp.target_zoom, vp.zoom);
        assert!(vp.zoom_anchor.is_none());
        assert_eq!(vp.pan_offset, vec2(14.0, 16.0));
    }
}
