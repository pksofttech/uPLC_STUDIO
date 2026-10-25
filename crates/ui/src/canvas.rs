use egui::{
    pos2, vec2, Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Sense, Stroke,
    StrokeKind, Ui, Vec2,
};
use my_canvas_core::{
    CoilMode, CoilStyle, ContactMode, ContactStyle, Document, InsertPosition, LadderElement, TypeTag,
};
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
    pub drag_candidate: Option<(u64, String, Pos2)>,
    pub dragging_elem: Option<(u64, String)>,
    pub dragging_tool: Option<(ActiveTool, String)>,
    pub marquee_start: Option<Pos2>,
    pub dragging_scrollbar: Option<ScrollbarDrag>,
    pub is_edit_mode: bool,
    pub is_online_hw_mode: bool,
    pub pending_debug_event: Option<CanvasDebugEvent>,
}

impl Default for CanvasViewport {
    fn default() -> Self {
        Self {
            pan_offset: vec2(14.0, 16.0),
            zoom: 1.0,
            target_zoom: 1.0,
            zoom_anchor: None,
            context_menu_elem: None,
            drag_candidate: None,
            dragging_elem: None,
            dragging_tool: None,
            marquee_start: None,
            dragging_scrollbar: None,
            is_edit_mode: true,
            is_online_hw_mode: false,
            pending_debug_event: None,
        }
    }
}

#[derive(Clone, Debug)]
struct DropTarget {
    rung_idx: usize,
    target_elem_id: Option<u64>,
    insert_pos: InsertPosition,
    target_label: String,
    validation_error: Option<String>,
    create_new_rung: bool,
}

fn validate_placement(
    source_tag: TypeTag,
    target_elem: &LadderElement,
    insert_pos: InsertPosition,
    target_is_first: bool,
    _target_is_last: bool,
) -> Result<InsertPosition, &'static str> {
    // Rule 1: Label [LBL] must only be placed at the beginning of the network
    if source_tag == TypeTag::Label {
        if insert_pos == InsertPosition::TopParallel || insert_pos == InsertPosition::BottomParallel {
            return Err("LBL cannot be placed in a parallel branch");
        }
        if !target_is_first || insert_pos != InsertPosition::Left {
            return Err("LBL must be placed at the left rail (beginning of network)");
        }
    }

    // Rule 2: Cannot place any components after Jump [JMP] (Dead code)
    if target_elem.element_type == TypeTag::Jump && insert_pos == InsertPosition::Right {
        return Err("Cannot place elements after JMP instruction");
    }

    // Rule 3: Output Coils terminate the rung
    if target_elem.element_type == TypeTag::Coil {
        if insert_pos == InsertPosition::Right {
            if source_tag == TypeTag::Coil {
                // Auto-correct: user dropped coil to the right of coil -> connect parallel coil!
                return Ok(InsertPosition::BottomParallel);
            } else {
                return Err("Coil terminates the network. Cannot place inputs to the right");
            }
        }
        if insert_pos == InsertPosition::Left && source_tag == TypeTag::Coil {
            // Auto-correct: user dropped coil to the left of coil -> connect parallel coil!
            return Ok(InsertPosition::TopParallel);
        }
        // Cannot parallel a contact across an output coil
        if (insert_pos == InsertPosition::TopParallel || insert_pos == InsertPosition::BottomParallel)
            && source_tag == TypeTag::Contact
        {
            return Err("Cannot parallel an input contact across an output coil");
        }
    }

    // Rule 4: Cannot parallel an output coil across an input contact
    if target_elem.element_type == TypeTag::Contact && source_tag == TypeTag::Coil {
        if insert_pos == InsertPosition::TopParallel || insert_pos == InsertPosition::BottomParallel {
            return Err("Cannot parallel an output coil across an input contact");
        }
        if insert_pos == InsertPosition::Left {
            return Err("Cannot place an output coil before an input contact");
        }
    }

    // Rule 5: Cannot place a coil before a Function Block (TON/TOF/CTU)
    if (target_elem.element_type == TypeTag::Ton
        || target_elem.element_type == TypeTag::Tof
        || target_elem.element_type == TypeTag::Tp
        || target_elem.element_type == TypeTag::Ctu
        || target_elem.element_type == TypeTag::Ctd
        || target_elem.element_type == TypeTag::Ctud)
        && source_tag == TypeTag::Coil
        && insert_pos == InsertPosition::Left
    {
        return Err("Cannot place an output coil before a function block");
    }

    Ok(insert_pos)
}

enum CanvasAction {
    ClearSelection,
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

    pub fn compute_content_size(&self, doc: &Document, viewport_width: f32) -> Vec2 {
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

        vec2(
            (total_world_w * self.zoom) + self.pan_offset.x.max(0.0),
            (total_world_h * self.zoom) + self.pan_offset.y.max(0.0),
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
        let bar_thickness = 11.0_f32;

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
            pos2(track_rect_v.min.x + 1.5, thumb_top_y),
            pos2(track_rect_v.max.x - 1.5, thumb_top_y + thumb_h),
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
            pos2(thumb_left_x, track_rect_h.min.y + 1.5),
            pos2(thumb_left_x + thumb_w, track_rect_h.max.y - 1.5),
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

        let is_dragging = self.dragging_elem.is_some() || self.dragging_tool.is_some();
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

                let first_line_y = self.to_screen(pos2(0.0, cur_y + 12.5), origin).y;
                let mut text_x = c_rect.min.x + 8.0 * self.zoom;

                // Prominent Flag Badge for Target Label
                if let Some(lbl) = label_elem {
                    let badge_text = format!("⚑ LBL {}", lbl.name);
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
            let badge_bg = if is_rung_selected {
                palette.rung_badge_sel_bg
            } else if badge_hovered {
                palette.rung_badge_hov_bg
            } else {
                palette.rung_badge_bg
            };
            let badge_border = if is_rung_selected {
                palette.rung_badge_sel_border
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
            let top_sub = if label_elem.is_some() { "⚑ TARGET" } else { "NETWORK" };
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
                    // Right-align terminal elements (COIL or trailing output element at the end of the rung)
                    let is_last = elem_idx == last_idx;
                    if elem.element_type.is_right_aligned() && is_last {
                        let target_x = (margin_right - cell_w - 16.0).max(cur_x + 36.0);
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
                    );

                    cur_x = next_x;
                    if elem_max_y > max_y {
                        max_y = elem_max_y;
                    }
                    power_in = elem.is_energized && sim_active;
                }

                // If no right-aligned coil yet, show [+] button at end of elements (only in Edit mode)
                let has_coil = rung
                    .elements
                    .iter()
                    .any(|e| e.element_type.is_right_aligned());
                if self.is_edit_mode && !sim_active && !has_coil && cur_x < margin_right - 40.0 {
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

                // Connect tail wire to Right Power Rail
                if cur_x < margin_right {
                    self.draw_power_wire(
                        &painter,
                        self.to_screen(pos2(cur_x, wire_y), origin),
                        self.to_screen(pos2(margin_right, wire_y), origin),
                        rung.is_energized && sim_active,
                        palette,
                    );
                }
                // T-junction dot at L2 rail intersection
                let pt_l2 = self.to_screen(pos2(margin_right, wire_y), origin);
                if rung.is_energized && sim_active {
                    painter.circle_filled(pt_l2, 5.5 * self.zoom, palette.wire_glow);
                    painter.circle_filled(pt_l2, 3.4 * self.zoom, palette.wire_energized);
                } else {
                    painter.circle_filled(pt_l2, 3.0 * self.zoom, palette.l2_rail);
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
                    painter.rect_filled(
                        rung_band_screen.shrink(3.0 * self.zoom),
                        6.0 * self.zoom,
                        Color32::from_rgba_unmultiplied(40, 180, 255, 32),
                    );
                    painter.rect_stroke(
                        rung_band_screen.shrink(3.0 * self.zoom),
                        6.0 * self.zoom,
                        Stroke::new(2.0 * self.zoom, palette.accent_heading),
                        StrokeKind::Inside,
                    );
                    let label = if let Some((_, ref lbl)) = self.dragging_tool {
                        format!("[Drop New] Add {} to Network #{:02}", lbl, r_idx + 1)
                    } else {
                        format!("[Drop Here] Move onto Network #{:02}", r_idx + 1)
                    };
                    painter.text(
                        pos2(
                            rung_band_screen.center().x,
                            rung_band_screen.min.y + 12.0 * self.zoom,
                        ),
                        Align2::CENTER_CENTER,
                        label,
                        self.font_prop(11.5),
                        palette.accent_heading,
                    );
                    let mut append_err = None;
                    if let Some(src_type) = dragging_source_type {
                        if src_type == TypeTag::Label && !doc.program.rungs[r_idx].elements.is_empty() {
                            append_err = Some("LBL must be placed at the start of an empty network".to_string());
                        }
                    }
                    drop_target = Some(DropTarget {
                        rung_idx: r_idx,
                        target_elem_id: None,
                        insert_pos: InsertPosition::AppendRung,
                        target_label: format!("Network #{:02} (Append)", r_idx + 1),
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
            let ghost_fill = Color32::from_rgba_unmultiplied(40, 180, 255, 26);
            let ghost_border = palette.accent_heading;
            painter.rect_filled(new_rung_band_screen.shrink(2.0 * self.zoom), 6.0 * self.zoom, ghost_fill);
            painter.rect_stroke(
                new_rung_band_screen.shrink(2.0 * self.zoom),
                6.0 * self.zoom,
                Stroke::new(1.8 * self.zoom, ghost_border),
                StrokeKind::Inside,
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
            let banner_msg = if let Some((_, ref lbl)) = self.dragging_tool {
                format!("✨ Drop to create New Network #{:02} with {}", new_rung_idx + 1, lbl)
            } else if let Some((_, ref lbl)) = self.dragging_elem {
                format!("✨ Drop to move {} into New Network #{:02}", lbl, new_rung_idx + 1)
            } else {
                format!("✨ Drop here to create New Network #{:02}", new_rung_idx + 1)
            };
            painter.text(
                pos2(new_rung_band_screen.center().x, new_rung_band_screen.center().y - 12.0 * self.zoom),
                Align2::CENTER_CENTER,
                banner_msg,
                self.font_prop(12.0),
                palette.accent_heading,
            );

            drop_target = Some(DropTarget {
                rung_idx: new_rung_idx,
                target_elem_id: None,
                insert_pos: InsertPosition::AppendRung,
                target_label: format!("New Network #{:02}", new_rung_idx + 1),
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
                            drop_target = Some(DropTarget {
                                rung_idx: r_idx,
                                target_elem_id: None,
                                insert_pos: InsertPosition::AppendRung,
                                target_label: format!("Network #{:02} (Append)", r_idx + 1),
                                validation_error: None,
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

        // --- Floating Quick-Action HUD Bar above the Selected Element(s) ---
        if self.is_edit_mode && !sim_active && !is_dragging && active_marquee_rect.is_none() {
            if doc.selected_element_ids.len() > 1 {
                // Multi-Selection HUD Bar
                let mut group_rect: Option<Rect> = None;
                for &(_, eid, erect) in &element_screen_bounds {
                    if doc.selected_element_ids.contains(&eid) {
                        group_rect = Some(match group_rect {
                            Some(gr) => gr.union(erect),
                            None => erect,
                        });
                    }
                }
                if let Some(sel_rect) = group_rect {
                    let count = doc.selected_element_ids.len();
                    let hud_w = 345.0;
                    let hud_h = 26.0;
                    let hud_y = if sel_rect.min.y - hud_h - 6.0 > viewport_rect.min.y + 40.0 {
                        sel_rect.min.y - hud_h - 6.0
                    } else {
                        sel_rect.max.y + 6.0
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

                    painter.rect_filled(hud_rect, CornerRadius::same(6), palette.banner_bg);
                    painter.rect_stroke(
                        hud_rect,
                        CornerRadius::same(6),
                        Stroke::new(1.5, palette.accent_heading),
                        StrokeKind::Inside,
                    );

                    // Count badge on left
                    let badge_w = 78.0;
                    let badge_rect = Rect::from_min_size(
                        pos2(hud_rect.min.x + 4.0, hud_rect.min.y + 3.0),
                        vec2(badge_w, hud_h - 6.0),
                    );
                    painter.rect_filled(
                        badge_rect,
                        CornerRadius::same(4),
                        palette.accent_heading.gamma_multiply(0.22),
                    );
                    painter.text(
                        badge_rect.center(),
                        Align2::CENTER_CENTER,
                        format!("{} Selected", count),
                        FontId::proportional(10.5),
                        palette.accent_heading,
                    );

                    let multi_btns: [(&str, Color32, CanvasAction); 5] = [
                        (
                            "+∥ Par",
                            palette.accent_ok,
                            CanvasAction::AddParallelToSelection,
                        ),
                        (
                            "+─ Ser",
                            palette.accent_warn,
                            CanvasAction::AddSeriesToSelection(true),
                        ),
                        (
                            "Mode",
                            palette.accent_heading,
                            CanvasAction::ToggleSelectedModes,
                        ),
                        (
                            "Del All",
                            palette.accent_danger,
                            CanvasAction::DeleteSelectedElements,
                        ),
                        (
                            "Clear",
                            palette.text_primary,
                            CanvasAction::ClearSelection,
                        ),
                    ];
                    let rem_w = hud_w - badge_w - 10.0;
                    let btn_w = rem_w / 5.0;
                    for (i, (lbl, col, btn_act)) in multi_btns.into_iter().enumerate() {
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
                            FontId::proportional(10.5),
                            if b_hov {
                                if palette.is_light() { palette.text_primary } else { Color32::WHITE }
                            } else {
                                col
                            },
                        );
                        if is_clicked && b_hov {
                            action = Some(btn_act);
                        }
                    }
                }
            } else if let (Some(_), Some(sel_id)) =
                (doc.selected_rung_idx, doc.selected_element_id)
            {
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
                    let hud_w = 265.0;
                    let hud_h = 24.0;
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

                    painter.rect_filled(hud_rect, CornerRadius::same(6), palette.banner_bg);
                    painter.rect_stroke(
                        hud_rect,
                        CornerRadius::same(6),
                        Stroke::new(1.3, palette.rung_badge_sel_border),
                        StrokeKind::Inside,
                    );

                    let hud_buttons: [(&str, Color32, CanvasAction); 5] = [
                        (
                            "Config",
                            palette.accent_heading,
                            CanvasAction::OpenConfigModal(sel_id),
                        ),
                        (
                            "+∥ Par",
                            palette.accent_ok,
                            CanvasAction::AddParallelToSelection,
                        ),
                        (
                            "+─ Ser",
                            palette.accent_warn,
                            CanvasAction::AddSeriesToSelection(true),
                        ),
                        (
                            "Mode",
                            palette.accent_heading,
                            CanvasAction::ToggleElementMode(sel_id),
                        ),
                        (
                            "Del",
                            palette.accent_danger,
                            CanvasAction::DeleteElement(sel_id),
                        ),
                    ];

                    let btn_w = (hud_w - 10.0) / 5.0;
                    for (i, (lbl, col, btn_act)) in hud_buttons.into_iter().enumerate() {
                        let bx = hud_rect.min.x + 5.0 + i as f32 * btn_w;
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
                            FontId::proportional(10.5),
                            if b_hov {
                                if palette.is_light() { palette.text_primary } else { Color32::WHITE }
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
        } else if (!self.is_edit_mode || sim_active) && !is_dragging {
            // --- Live Debug / Force Floating HUD Bar (Simulator & Online Hardware Modes) ---
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
                        let hud_w = 342.0;
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
                            Stroke::new(1.4, mode_border_col),
                            StrokeKind::Inside,
                        );

                        // Address & Live State Badge on left
                        let is_on = doc.simulator.get_var_value(&elem_addr);
                        let badge_w = 92.0;
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
                                "{}={}",
                                elem_addr,
                                if is_on { "ON(1)" } else { "OFF(0)" }
                            ),
                            FontId::monospace(9.5),
                            state_col,
                        );

                        let debug_btns: [(&str, Color32, CanvasAction); 4] = if self.is_online_hw_mode {
                            [
                                (
                                    "⚡ Force 1",
                                    palette.accent_ok,
                                    CanvasAction::DebugForceBit(elem_addr.clone(), true),
                                ),
                                (
                                    "⚡ Force 0",
                                    palette.accent_danger,
                                    CanvasAction::DebugForceBit(elem_addr.clone(), false),
                                ),
                                (
                                    "🔓 Unforce",
                                    palette.accent_warn,
                                    CanvasAction::DebugUnforceBit(elem_addr.clone()),
                                ),
                                (
                                    "✏ Edit",
                                    palette.accent_heading,
                                    CanvasAction::SwitchToEditMode(Some(sel_id)),
                                ),
                            ]
                        } else {
                            [
                                (
                                    "🔁 Toggle",
                                    palette.accent_ok,
                                    CanvasAction::DebugToggleBit(elem_addr.clone()),
                                ),
                                (
                                    "⚡ Force 1",
                                    palette.accent_warn,
                                    CanvasAction::DebugForceBit(elem_addr.clone(), true),
                                ),
                                (
                                    "🔓 Unforce",
                                    palette.text_primary,
                                    CanvasAction::DebugUnforceBit(elem_addr.clone()),
                                ),
                                (
                                    "✏ Edit",
                                    palette.accent_heading,
                                    CanvasAction::SwitchToEditMode(Some(sel_id)),
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
            Some(("Moving", drag_label.clone(), palette.accent_heading))
        } else if let Some((_, ref tool_label)) = self.dragging_tool {
            Some(("Placing", tool_label.clone(), palette.accent_ok))
        } else {
            None
        };

        if let Some((action_verb, label, accent_col)) = drag_info {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            if let Some(hp) = hover_pos {
                let ghost_w = 225.0;
                let ghost_x = (hp.x + 14.0).min((viewport_rect.max.x - ghost_w - 6.0).max(viewport_rect.min.x + 6.0));
                let ghost_rect = Rect::from_min_size(pos2(ghost_x, hp.y + 14.0), vec2(ghost_w, 44.0));
                painter.rect_filled(ghost_rect, CornerRadius::same(6), palette.card_bg);
                let val_err = drop_target.as_ref().and_then(|dt| dt.validation_error.as_ref());
                let ghost_border_col = if val_err.is_some() {
                    palette.accent_danger
                } else {
                    accent_col
                };
                painter.rect_stroke(
                    ghost_rect,
                    CornerRadius::same(6),
                    Stroke::new(1.5, ghost_border_col),
                    StrokeKind::Inside,
                );
                painter.text(
                    pos2(ghost_rect.min.x + 10.0, ghost_rect.min.y + 14.0),
                    Align2::LEFT_CENTER,
                    format!("[Drag] {}: {}", action_verb, label),
                    FontId::proportional(12.0),
                    palette.text_primary,
                );
                let (sub_msg, sub_color) = if let Some(ref dt) = drop_target {
                    if let Some(ref err) = dt.validation_error {
                        (format!("-> ❌ {}", err), palette.accent_danger)
                    } else if dt.create_new_rung {
                        (format!("-> ✨ Create & Drop: {}", dt.target_label), palette.accent_ok)
                    } else {
                        (format!("-> Drop at: {}", dt.target_label), palette.accent_ok)
                    }
                } else {
                    ("Hover target Network or Empty Space to place".to_string(), palette.accent_warn)
                };
                painter.text(
                    pos2(ghost_rect.min.x + 10.0, ghost_rect.min.y + 31.0),
                    Align2::LEFT_CENTER,
                    sub_msg,
                    FontId::proportional(10.5),
                    sub_color,
                );
            }
        }

        // --- Draw Vertical & Horizontal Scrollbars ---
        if show_v_bar {
            let is_dragging_v = matches!(self.dragging_scrollbar, Some(ScrollbarDrag::Vertical { .. }));
            let is_thumb_hov = raw_hover_pos.map_or(false, |hp| thumb_rect_v.contains(hp));
            let track_bg = palette.canvas_bg.gamma_multiply(0.85);
            painter.rect_filled(track_rect_v, CornerRadius::same(5), track_bg);
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
            painter.rect_filled(track_rect_h, CornerRadius::same(5), track_bg);
            let thumb_bg = if is_dragging_h {
                palette.accent_heading
            } else if is_thumb_hov {
                palette.banner_border.gamma_multiply(1.5)
            } else {
                palette.banner_border.gamma_multiply(0.9)
            };
            painter.rect_filled(thumb_rect_h, CornerRadius::same(4), thumb_bg);
        }

        // --- Bottom-Right Floating Zoom Control HUD ([-], 100%, [+], [Reset]) ---
        {
            let zbar_w = 158.0;
            let zbar_h = 26.0;
            let offset_x = if show_v_bar { bar_thickness + 12.0 } else { 12.0 };
            let offset_y = if show_h_bar { bar_thickness + 10.0 } else { 10.0 };
            let zbar_rect = Rect::from_min_size(
                pos2(
                    viewport_rect.max.x - zbar_w - offset_x,
                    viewport_rect.max.y - zbar_h - offset_y,
                ),
                vec2(zbar_w, zbar_h),
            );
            if raw_hover_pos.map_or(false, |hp| zbar_rect.contains(hp)) {
                any_interactive_hovered = true;
            }
            painter.rect_filled(zbar_rect, CornerRadius::same(6), palette.banner_bg);
            painter.rect_stroke(
                zbar_rect,
                CornerRadius::same(6),
                Stroke::new(1.0, palette.banner_border),
                StrokeKind::Inside,
            );

            let minus_rect = Rect::from_min_size(
                pos2(zbar_rect.min.x + 3.0, zbar_rect.min.y + 3.0),
                vec2(24.0, zbar_h - 6.0),
            );
            let pct_rect = Rect::from_min_size(
                pos2(minus_rect.max.x + 2.0, zbar_rect.min.y + 3.0),
                vec2(52.0, zbar_h - 6.0),
            );
            let plus_rect = Rect::from_min_size(
                pos2(pct_rect.max.x + 2.0, zbar_rect.min.y + 3.0),
                vec2(24.0, zbar_h - 6.0),
            );
            let fit_rect = Rect::from_min_size(
                pos2(plus_rect.max.x + 2.0, zbar_rect.min.y + 3.0),
                vec2(44.0, zbar_h - 6.0),
            );

            for (btn_r, lbl, kind) in [
                (minus_rect, "−", -1i8),
                (
                    pct_rect,
                    &format!("{:.0}%", self.zoom * 100.0),
                    0i8,
                ),
                (plus_rect, "+", 1i8),
                (fit_rect, "Reset", 2i8),
            ] {
                let is_hov = raw_hover_pos.map_or(false, |hp| btn_r.contains(hp));
                if is_hov {
                    painter.rect_filled(
                        btn_r,
                        CornerRadius::same(4),
                        palette.accent_heading.gamma_multiply(0.22),
                    );
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
                if raw_is_clicked && is_hov {
                    match kind {
                        -1 => self.set_target_zoom(
                            self.target_zoom / 1.18,
                            Some(viewport_rect.center()),
                        ),
                        1 => self.set_target_zoom(
                            self.target_zoom * 1.18,
                            Some(viewport_rect.center()),
                        ),
                        _ => self.reset_view(),
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
                            doc.move_element(drag_id, best_r, None, InsertPosition::AppendRung);
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
                        let next_id = doc.program.alloc_id();
                        if let Some(elem) = tool.create_default_element(next_id) {
                            if doc.program.rungs.is_empty() || world_p.y >= cur_y - 8.0 {
                                doc.insert_element_to_new_rung(elem, None);
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
                                doc.selected_rung_idx = Some(best_r);
                                doc.selected_element_id = None;
                                doc.insert_element(elem, default_insert_pos);
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

        // --- Context Menu Popup (Right-Click on Element or Network) ---
        if let Some((r_idx, el_id)) = self.context_menu_elem {
            let mut close_menu = false;
            let menu_title = if !self.is_edit_mode || sim_active {
                if self.is_online_hw_mode {
                    "🔌 Online PLC Debug & Force Menu"
                } else {
                    "💻 Simulator Debug & Force Menu"
                }
            } else {
                "Quick Element & Network Menu"
            };
            egui::Window::new(menu_title)
                .collapsible(false)
                .resizable(false)
                .default_pos(mouse_pos.unwrap_or(viewport_rect.center()))
                .show(ui.ctx(), |ui| {
                    if !self.is_edit_mode || sim_active {
                        if let Some(el) = doc.element_by_id(el_id).cloned() {
                            let addr = el.name.trim().to_string();
                            let is_on = doc.simulator.get_var_value(&addr);
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(format!("{} ({})", addr, el.element_type.display_name()))
                                        .color(palette.accent_heading)
                                        .strong(),
                                );
                                ui.label(
                                    egui::RichText::new(if is_on { "● ON (1)" } else { "○ OFF (0)" })
                                        .monospace()
                                        .strong()
                                        .color(if is_on { palette.accent_ok } else { palette.rung_text_dim }),
                                );
                            });
                            ui.separator();
                            if !self.is_online_hw_mode {
                                if ui
                                    .button(
                                        egui::RichText::new("🔁 Toggle Input State (ON ↔ OFF)")
                                            .color(palette.accent_ok)
                                            .strong(),
                                    )
                                    .clicked()
                                {
                                    action = Some(CanvasAction::DebugToggleBit(addr.clone()));
                                    close_menu = true;
                                }
                            }
                            if ui
                                .button(
                                    egui::RichText::new("⚡ Force ON (1)")
                                        .color(palette.accent_ok)
                                        .strong(),
                                )
                                .clicked()
                            {
                                action = Some(CanvasAction::DebugForceBit(addr.clone(), true));
                                close_menu = true;
                            }
                            if ui
                                .button(
                                    egui::RichText::new("⚡ Force OFF (0)")
                                        .color(palette.accent_danger)
                                        .strong(),
                                )
                                .clicked()
                            {
                                action = Some(CanvasAction::DebugForceBit(addr.clone(), false));
                                close_menu = true;
                            }
                            if ui
                                .button(
                                    egui::RichText::new("🔓 Release Force (Normal)")
                                        .color(palette.accent_warn),
                                )
                                .clicked()
                            {
                                action = Some(CanvasAction::DebugUnforceBit(addr));
                                close_menu = true;
                            }
                            ui.separator();
                        }
                        if !doc.simulator.forced_bits.is_empty() || self.is_online_hw_mode {
                            if ui.button("🔓 Clear All Forced Bits").clicked() {
                                action = Some(CanvasAction::DebugClearAllForces);
                                close_menu = true;
                            }
                            ui.separator();
                        }
                        if ui
                            .button(
                                egui::RichText::new("✏ Switch to Edit Mode to Modify Circuit")
                                    .color(palette.accent_heading)
                                    .strong(),
                            )
                            .clicked()
                        {
                            let open_id = if el_id != 0 { Some(el_id) } else { None };
                            action = Some(CanvasAction::SwitchToEditMode(open_id));
                            close_menu = true;
                        }
                        ui.separator();
                        if ui.button("Close").clicked() {
                            close_menu = true;
                        }
                    } else {
                        let is_multi = doc.selected_element_ids.len() > 1 && doc.selected_element_ids.contains(&el_id);
                        if is_multi {
                            ui.label(egui::RichText::new(format!("Selected Group ({} Elements)", doc.selected_element_ids.len())).color(palette.accent_heading).strong());
                            ui.separator();
                            if ui.button("+ Add in Parallel (Below)").clicked() {
                                action = Some(CanvasAction::AddParallelToSelection);
                                close_menu = true;
                            }
                            if ui.button("+ Add in Series (Right / After)").clicked() {
                                action = Some(CanvasAction::AddSeriesToSelection(true));
                                close_menu = true;
                            }
                            if ui.button("+ Add in Series (Left / Before)").clicked() {
                                action = Some(CanvasAction::AddSeriesToSelection(false));
                                close_menu = true;
                            }
                            if ui.button("Toggle Contact / Coil Modes").clicked() {
                                action = Some(CanvasAction::ToggleSelectedModes);
                                close_menu = true;
                            }
                            if ui
                                .button(egui::RichText::new("Delete Selected Elements").color(palette.accent_danger))
                                .clicked()
                            {
                                action = Some(CanvasAction::DeleteSelectedElements);
                                close_menu = true;
                            }
                            ui.separator();
                        } else if el_id != 0 {
                            ui.label(egui::RichText::new(format!("Element ID #{}", el_id)).strong());
                            ui.separator();
                            if ui
                                .button(
                                    egui::RichText::new("Configure Element... (Enter)")
                                        .color(palette.accent_heading)
                                        .strong(),
                                )
                                .clicked()
                            {
                                action = Some(CanvasAction::OpenConfigModal(el_id));
                                close_menu = true;
                            }
                            if ui.button("+ Add in Parallel (Below)").clicked() {
                                doc.set_single_selection(Some(r_idx), el_id);
                                action = Some(CanvasAction::AddParallelToSelection);
                                close_menu = true;
                            }
                            if ui.button("+ Add in Series (Right / After)").clicked() {
                                doc.set_single_selection(Some(r_idx), el_id);
                                action = Some(CanvasAction::AddSeriesToSelection(true));
                                close_menu = true;
                            }
                            if ui.button("+ Add in Series (Left / Before)").clicked() {
                                doc.set_single_selection(Some(r_idx), el_id);
                                action = Some(CanvasAction::AddSeriesToSelection(false));
                                close_menu = true;
                            }
                            if ui.button("Toggle Contact / Coil Mode").clicked() {
                                action = Some(CanvasAction::ToggleElementMode(el_id));
                                close_menu = true;
                            }
                            if ui
                                .button(
                                    egui::RichText::new("Delete Element").color(palette.accent_danger),
                                )
                                .clicked()
                            {
                                action = Some(CanvasAction::DeleteElement(el_id));
                                close_menu = true;
                            }
                            ui.separator();
                        }
                        ui.label(egui::RichText::new(format!("Network #{:02}", r_idx + 1)).strong());
                        if ui.button("Duplicate Network Below").clicked() {
                            action = Some(CanvasAction::DuplicateRung(r_idx));
                            close_menu = true;
                        }
                        if ui
                            .button(egui::RichText::new("Delete Network").color(palette.accent_danger))
                            .clicked()
                        {
                            action = Some(CanvasAction::DeleteRung(r_idx));
                            close_menu = true;
                        }
                        ui.separator();
                        if ui.button("Close").clicked() {
                            close_menu = true;
                        }
                    }
                });
            if close_menu {
                self.context_menu_elem = None;
            }
        }

        let mut open_modal_for_id: Option<u64> = None;

        // --- Process Canvas Actions ---
        if let Some(act) = action {
            match act {
                CanvasAction::ClearSelection => {
                    doc.clear_element_selection();
                }
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

            for elem in &rung.elements {
                if elem.element_type.is_right_aligned() {
                    let target_x = (margin_right - cell_w - 16.0).max(cur_x + 36.0);
                    if target_x > cur_x {
                        cur_x = target_x;
                    }
                }
                if elem.element_type == TypeTag::Branch {
                    let pad_x = 14.0;
                    let inner_start_x = cur_x + pad_x;
                    let mut max_inner_x = inner_start_x + cell_w;
                    let mut cur_branch_y = rung_start_y;
                    for path in &elem.branch_paths {
                        let mut path_x = inner_start_x;
                        for sub in path {
                            if sub.id == target_id {
                                let rw = Rect::from_min_size(pos2(path_x, cur_branch_y), vec2(cell_w, cell_h));
                                return Some(Rect::from_min_max(
                                    self.to_screen(rw.min, origin),
                                    self.to_screen(rw.max, origin),
                                ));
                            }
                            path_x += cell_w;
                        }
                        if path_x > max_inner_x {
                            max_inner_x = path_x;
                        }
                        cur_branch_y += cell_h;
                    }
                    cur_x = max_inner_x + pad_x;
                    if cur_branch_y > max_y {
                        max_y = cur_branch_y;
                    }
                } else {
                    if elem.id == target_id {
                        let rw = Rect::from_min_size(pos2(cur_x, rung_start_y), vec2(cell_w, cell_h));
                        return Some(Rect::from_min_max(
                            self.to_screen(rw.min, origin),
                            self.to_screen(rw.max, origin),
                        ));
                    }
                    cur_x += cell_w;
                }
            }
            cur_y = max_y + 18.0;
        }
        None
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
            let mut validation_result: Result<InsertPosition, &'static str> = Ok(InsertPosition::Right);

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
                        validation_result = validate_placement(src_tag, elem, raw_zone, target_is_first, target_is_last);
                    }
                    hovered_zone = Some(raw_zone);
                }
            }

            if is_dragging_other && is_hovered {
                if let Some(zone) = hovered_zone {
                    let (effective_zone, val_err) = match validation_result {
                        Ok(adjusted_zone) => (adjusted_zone, None),
                        Err(err_msg) => (zone, Some(err_msg.to_string())),
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
                        validation_error: val_err,
                        create_new_rung: false,
                    });
                }
            }

            if is_being_dragged {
                painter.rect_stroke(
                    rect_screen.shrink(2.0 * self.zoom),
                    6.0 * self.zoom,
                    Stroke::new(1.8 * self.zoom, palette.accent_heading),
                    StrokeKind::Inside,
                );
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
            }

            let is_valid = validation_result.is_ok();
            let hovered_zone_with_validity = hovered_zone.map(|z| (z, is_valid));

            self.draw_modern_element(
                painter,
                rect_screen,
                elem,
                is_selected,
                is_hovered,
                sim_active,
                power_in,
                hovered_zone_with_validity,
                default_contact_style,
                default_coil_style,
                palette,
            );

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
        is_hovered: bool,
        sim_active: bool,
        power_in: bool,
        hovered_zone: Option<(InsertPosition, bool)>,
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

        // Directional Insertion Zone Highlight
        if let Some((zone, is_valid)) = hovered_zone {
            let (z_rect, z_label) = match zone {
                InsertPosition::Left => (
                    Rect::from_min_max(rect_screen.min, pos2(x0 + w * 0.28, rect_screen.max.y)),
                    if is_valid { "< Left" } else { "❌ Block" },
                ),
                InsertPosition::Right => (
                    Rect::from_min_max(pos2(x0 + w * 0.72, y0), rect_screen.max),
                    if is_valid { "Right >" } else { "❌ Block" },
                ),
                InsertPosition::TopParallel => (
                    Rect::from_min_max(rect_screen.min, pos2(rect_screen.max.x, y0 + h * 0.30)),
                    if is_valid { "^ Parallel" } else { "❌ Block" },
                ),
                InsertPosition::BottomParallel => (
                    Rect::from_min_max(pos2(x0, y0 + h * 0.70), rect_screen.max),
                    if is_valid { "v Parallel" } else { "❌ Block" },
                ),
                InsertPosition::AppendRung => (rect_screen, if is_valid { "+ Add" } else { "❌ Block" }),
            };
            let fill_col = if is_valid {
                Color32::from_rgba_unmultiplied(35, 165, 240, 90)
            } else {
                Color32::from_rgba_unmultiplied(225, 45, 40, 110)
            };
            painter.rect_filled(
                z_rect,
                4.0 * self.zoom,
                fill_col,
            );
            painter.text(
                z_rect.center(),
                Align2::CENTER_CENTER,
                z_label,
                self.font_prop(9.5),
                Color32::WHITE,
            );
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
            | TypeTag::Bitwise => {
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

                painter.text(
                    pos2(center.x, card_rect.min.y + 11.0 * self.zoom),
                    Align2::CENTER_CENTER,
                    hdr,
                    self.font_mono(9.5),
                    palette.accent_heading,
                );
                painter.text(
                    pos2(center.x, card_rect.center().y + 1.0 * self.zoom),
                    Align2::CENTER_CENTER,
                    line1,
                    self.font_mono(8.8),
                    palette.text_primary,
                );
                if !line2.is_empty() {
                    painter.text(
                        pos2(center.x, card_rect.max.y - 10.0 * self.zoom),
                        Align2::CENTER_CENTER,
                        line2,
                        self.font_mono(8.2),
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

                let txt = format!("⚑ LBL {}", elem.name);
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

                // Center: Instance name
                painter.text(
                    pos2(center.x, center.y - 4.0 * self.zoom),
                    Align2::CENTER_CENTER,
                    &elem.name,
                    self.font_mono(13.0),
                    if is_selected { Color32::WHITE } else { palette.rung_text_num },
                );

                // Bottom: Slave ID, Func, Buffer
                let subtext = format!("ID:{} FC:{} -> {}", elem.mb_slave_id, elem.mb_function, elem.target_reg);
                painter.text(
                    pos2(center.x, y0 + h - 14.0 * self.zoom),
                    Align2::CENTER_CENTER,
                    &subtext,
                    self.font_mono(9.5),
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

        // Rule 1: Label [LBL]
        assert!(validate_placement(TypeTag::Label, &contact, InsertPosition::Left, true, false).is_ok());
        assert!(validate_placement(TypeTag::Label, &contact, InsertPosition::Left, false, false).is_err());
        assert!(validate_placement(TypeTag::Label, &contact, InsertPosition::Right, true, false).is_err());
        assert!(validate_placement(TypeTag::Label, &contact, InsertPosition::TopParallel, true, false).is_err());

        // Rule 2: Cannot place any components after Jump [JMP]
        assert!(validate_placement(TypeTag::Contact, &jmp, InsertPosition::Right, false, true).is_err());
        assert!(validate_placement(TypeTag::Contact, &jmp, InsertPosition::Left, false, true).is_ok());

        // Rule 3: Output Coils terminate the rung & auto-correction
        // Dropping coil to the right of coil -> auto-correct to BottomParallel
        assert_eq!(
            validate_placement(TypeTag::Coil, &coil, InsertPosition::Right, false, true).unwrap(),
            InsertPosition::BottomParallel
        );
        // Dropping coil to the left of coil -> auto-correct to TopParallel
        assert_eq!(
            validate_placement(TypeTag::Coil, &coil, InsertPosition::Left, false, true).unwrap(),
            InsertPosition::TopParallel
        );
        // Contact cannot be placed to the right of coil
        assert!(validate_placement(TypeTag::Contact, &coil, InsertPosition::Right, false, true).is_err());
        // Contact cannot parallel across coil
        assert!(validate_placement(TypeTag::Contact, &coil, InsertPosition::BottomParallel, false, true).is_err());

        // Rule 4: Cannot parallel an output coil across an input contact
        assert!(validate_placement(TypeTag::Coil, &contact, InsertPosition::BottomParallel, true, false).is_err());
        assert!(validate_placement(TypeTag::Coil, &contact, InsertPosition::TopParallel, true, false).is_err());
        assert!(validate_placement(TypeTag::Coil, &contact, InsertPosition::Left, true, false).is_err());

        // Rule 5: Cannot place an output coil before a function block
        assert!(validate_placement(TypeTag::Coil, &ton, InsertPosition::Left, false, false).is_err());
    }

    #[test]
    fn test_drop_target_create_new_rung() {
        let dt = DropTarget {
            rung_idx: 2,
            target_elem_id: None,
            insert_pos: InsertPosition::AppendRung,
            target_label: "New Network #03".to_string(),
            validation_error: None,
            create_new_rung: true,
        };
        assert!(dt.create_new_rung);
        assert_eq!(dt.rung_idx, 2);
        assert!(dt.validation_error.is_none());
    }
}
