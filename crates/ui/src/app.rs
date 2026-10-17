use std::path::PathBuf;
use std::time::Instant;
use egui::{CentralPanel, Panel, RichText, Ui, Window};
use my_canvas_core::{Document, IOPortMapping, InsertPosition, SchematicStyle, SimState};
use serde::{Deserialize, Serialize};
use crate::canvas::CanvasViewport;
use crate::panels::{element_dialog::ElementConfigModal, left_panel, menu_bar, right_panel, status_bar, toolbar};
use crate::theme::{self, AppTheme};
use crate::tools::ActiveTool;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub theme: AppTheme,
    pub schematic_style: SchematicStyle,
    #[serde(alias = "show_plclib_code")]
    pub show_il_code: bool,
    pub com_port: String,
    pub baud_rate: String,
    pub cycle_time_ms: u32,
    pub hardware: String,
    pub window_width: Option<f32>,
    pub window_height: Option<f32>,
    pub window_maximized: bool,
    pub window_pos_x: Option<f32>,
    pub window_pos_y: Option<f32>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: AppTheme::Modern,
            schematic_style: SchematicStyle::default(),
            show_il_code: true,
            com_port: "ttyUSB0 (Auto)".to_string(),
            baud_rate: "115200".to_string(),
            cycle_time_ms: 10,
            hardware: "IEC 61131-3 Standard PLC".to_string(),
            window_width: Some(1280.0),
            window_height: Some(720.0),
            window_maximized: false,
            window_pos_x: None,
            window_pos_y: None,
        }
    }
}

impl AppSettings {
    pub fn clamped_window_size(&self) -> (f32, f32) {
        let w = self.window_width.unwrap_or(1280.0).max(800.0);
        let h = self.window_height.unwrap_or(720.0).max(600.0);
        (w, h)
    }

    pub fn config_path() -> PathBuf {
        if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
            PathBuf::from(xdg).join("uplc_studio").join("settings.json")
        } else if let Some(home) = std::env::var_os("HOME") {
            PathBuf::from(home)
                .join(".config")
                .join("uplc_studio")
                .join("settings.json")
        } else {
            PathBuf::from("uplc_settings.json")
        }
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(settings) = serde_json::from_str::<Self>(&content) {
                return settings;
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, json);
        }
    }
}

pub struct CanvasApp {
    pub doc: Document,
    pub active_tool: ActiveTool,
    pub insert_pos: InsertPosition,
    pub copied_rung_idx: Option<usize>,
    pub canvas: CanvasViewport,
    pub elem_modal: ElementConfigModal,
    pub elem_filter_text: String,
    pub elem_filter_cat: u8,
    pub component_filter: String,
    pub right_panel_tab: u8,
    pub show_il_code: bool,
    pub show_about: bool,
    pub show_code_export: bool,
    pub show_io_config: bool,
    pub show_update_plc: bool,
    pub update_plc_tab: u8,
    pub update_plc_status: String,
    pub update_plc_is_error: bool,
    pub com_port: String,
    pub baud_rate: String,
    pub last_sim_tick: Instant,
    pub theme: AppTheme,
    pub applied_theme: Option<AppTheme>,
    pub fonts_initialized: bool,
    pub window_width: Option<f32>,
    pub window_height: Option<f32>,
    pub window_maximized: bool,
    pub window_pos_x: Option<f32>,
    pub window_pos_y: Option<f32>,
    pub window_state_dirty: bool,
    pub last_window_change: Option<Instant>,
    pub frame_counter: u64,
}

impl Default for CanvasApp {
    fn default() -> Self {
        let settings = AppSettings::load();
        let mut doc = Document::default();
        doc.set_schematic_style(settings.schematic_style);
        doc.is_dirty = false;
        doc.simulator.cycle_time_ms = settings.cycle_time_ms.clamp(1, 1000);
        if !settings.hardware.is_empty() {
            doc.program.hardware = settings.hardware;
        }

        Self {
            doc,
            active_tool: ActiveTool::Select,
            insert_pos: InsertPosition::AppendRung,
            copied_rung_idx: None,
            canvas: CanvasViewport::default(),
            elem_modal: ElementConfigModal::default(),
            elem_filter_text: String::new(),
            elem_filter_cat: 0,
            component_filter: String::new(),
            right_panel_tab: 0,
            show_il_code: settings.show_il_code,
            show_about: false,
            show_code_export: false,
            show_io_config: false,
            show_update_plc: false,
            update_plc_tab: 0,
            update_plc_status: String::new(),
            update_plc_is_error: false,
            com_port: settings.com_port,
            baud_rate: settings.baud_rate,
            last_sim_tick: Instant::now(),
            theme: settings.theme,
            applied_theme: None,
            fonts_initialized: false,
            window_width: settings.window_width.or(Some(1280.0)),
            window_height: settings.window_height.or(Some(720.0)),
            window_maximized: settings.window_maximized,
            window_pos_x: settings.window_pos_x,
            window_pos_y: settings.window_pos_y,
            window_state_dirty: false,
            last_window_change: None,
            frame_counter: 0,
        }
    }
}

impl CanvasApp {
    pub fn available_serial_ports() -> Vec<String> {
        let mut ports = Vec::new();
        if let Ok(entries) = std::fs::read_dir("/dev") {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    if name.starts_with("ttyACM") || name.starts_with("ttyUSB") {
                        ports.push(format!("/dev/{}", name));
                    }
                }
            }
        }
        ports.sort();
        if ports.is_empty() {
            ports.push("/dev/ttyACM0".to_string());
            ports.push("/dev/ttyUSB0".to_string());
            ports.push("COM3".to_string());
        }
        ports
    }

    pub fn transmit_bytecode_to_plc(&mut self, cmd: my_canvas_core::PlcSerialCommand) {
        use std::io::Write;
        let bc = self.doc.compile_to_bytecode();
        let packet = bc.to_serial_packet(cmd);
        let raw_port = self.com_port.split_whitespace().next().unwrap_or("/dev/ttyACM0");
        let port_path = if raw_port.starts_with("tty") {
            format!("/dev/{}", raw_port)
        } else {
            raw_port.to_string()
        };

        // Attempt to configure baud rate via stty on Linux if device exists
        if std::path::Path::new(&port_path).exists() {
            let _ = std::process::Command::new("stty")
                .args(["-F", &port_path, &self.baud_rate, "raw", "-echo"])
                .output();
        }

        match std::fs::OpenOptions::new().write(true).open(&port_path) {
            Ok(mut file) => match file.write_all(&packet) {
                Ok(_) => {
                    let _ = file.flush();
                    self.update_plc_is_error = false;
                    self.update_plc_status = format!(
                        "✔ Sent {} Bytes ({} instructions, CRC8=0x{:02X}) to {} @ {} baud",
                        packet.len(),
                        bc.instructions.len(),
                        packet.last().copied().unwrap_or(0),
                        port_path,
                        self.baud_rate
                    );
                }
                Err(e) => {
                    self.update_plc_is_error = true;
                    self.update_plc_status = format!(
                        "✖ Failed writing to {}: {} (Packet ready: {} Bytes)",
                        port_path,
                        e,
                        packet.len()
                    );
                }
            },
            Err(e) => {
                self.update_plc_is_error = true;
                self.update_plc_status = format!(
                    "⚠ Serial port '{}' not connected ({}) — Bytecode compiled ({} instructions, {} Bytes packet ready)",
                    port_path,
                    e,
                    bc.instructions.len(),
                    packet.len()
                );
            }
        }
    }

    pub fn current_settings(&self) -> AppSettings {
        AppSettings {
            theme: self.theme,
            schematic_style: self.doc.schematic_style,
            show_il_code: self.show_il_code,
            com_port: self.com_port.clone(),
            baud_rate: self.baud_rate.clone(),
            cycle_time_ms: self.doc.simulator.cycle_time_ms,
            hardware: self.doc.program.hardware.clone(),
            window_width: self.window_width,
            window_height: self.window_height,
            window_maximized: self.window_maximized,
            window_pos_x: self.window_pos_x,
            window_pos_y: self.window_pos_y,
        }
    }

    pub fn save_settings(&self) {
        self.current_settings().save();
    }

    pub fn set_theme(&mut self, theme: AppTheme) {
        if self.theme != theme {
            self.theme = theme;
            self.save_settings();
        }
    }

    fn ensure_fonts_and_theme(&mut self, ctx: &egui::Context) {
        if !self.fonts_initialized {
            theme::configure_fonts(ctx);
            self.fonts_initialized = true;
        }
        if self.applied_theme != Some(self.theme) {
            let was_initialized = self.applied_theme.is_some();
            self.theme.apply_visuals(ctx);
            self.applied_theme = Some(self.theme);
            if was_initialized {
                self.save_settings();
            }
        }
    }

    pub fn new_program(&mut self) {
        self.doc.clear();
        self.elem_modal.close();
        self.canvas.context_menu_elem = None;
        self.canvas.reset_view();
        self.active_tool = ActiveTool::Select;
        self.copied_rung_idx = None;
        self.elem_filter_text.clear();
        self.elem_filter_cat = 0;
        self.component_filter.clear();
    }

    pub fn open_config_for_selected(&mut self) {
        if let Some(el) = self.doc.selected_element().cloned() {
            let r_comment = self.doc.selected_rung_idx
                .and_then(|idx| self.doc.program.rungs.get(idx))
                .map(|r| r.comment.as_str())
                .unwrap_or("");
            self.elem_modal.open_for(&el, self.doc.selected_rung_idx, r_comment);
        } else if let Some(r_idx) = self.doc.selected_rung_idx {
            if let Some(rung) = self.doc.program.rungs.get(r_idx) {
                self.elem_modal.open_for_rung(r_idx, rung);
            }
        }
    }
}

impl eframe::App for CanvasApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.ensure_fonts_and_theme(ui.ctx());
        let palette = self.theme.palette();

        // Window size, position, and maximized state tracking
        self.frame_counter = self.frame_counter.saturating_add(1);
        if self.frame_counter > 5 {
            let (is_max, is_min, size, pos) = ui.input(|i| {
                let vp = i.viewport();
                (
                    vp.maximized,
                    vp.minimized.unwrap_or(false),
                    vp.inner_rect.map(|r| r.size()).or_else(|| {
                        let s = i.viewport_rect().size();
                        if s.x > 0.0 && s.y > 0.0 {
                            Some(s)
                        } else {
                            None
                        }
                    }),
                    vp.outer_rect.map(|r| [r.min.x, r.min.y]),
                )
            });

            if !is_min {
                if let Some(max) = is_max {
                    if max != self.window_maximized {
                        self.window_maximized = max;
                        self.window_state_dirty = true;
                        self.last_window_change = Some(Instant::now());
                    }
                }

                // Only record normal window size/position when NOT maximized
                if !self.window_maximized {
                    if let Some(s) = size {
                        if s.x >= 400.0 && s.y >= 300.0 {
                            let changed = match (self.window_width, self.window_height) {
                                (Some(w), Some(h)) => {
                                    (w - s.x).abs() >= 2.0 || (h - s.y).abs() >= 2.0
                                }
                                _ => true,
                            };
                            if changed {
                                self.window_width = Some(s.x);
                                self.window_height = Some(s.y);
                                self.window_state_dirty = true;
                                self.last_window_change = Some(Instant::now());
                            }
                        }
                    }

                    if let Some([px, py]) = pos {
                        if px >= 0.0 && py >= 0.0 && px < 10000.0 && py < 10000.0 {
                            let changed = match (self.window_pos_x, self.window_pos_y) {
                                (Some(x), Some(y)) => {
                                    (x - px).abs() >= 4.0 || (y - py).abs() >= 4.0
                                }
                                _ => true,
                            };
                            if changed {
                                self.window_pos_x = Some(px);
                                self.window_pos_y = Some(py);
                                self.window_state_dirty = true;
                                self.last_window_change = Some(Instant::now());
                            }
                        }
                    }
                }
            }
        }

        if self.window_state_dirty {
            if let Some(last) = self.last_window_change {
                if last.elapsed().as_millis() >= 1000 {
                    self.window_state_dirty = false;
                    self.save_settings();
                }
            }
        }

        // Periodic simulation scan when Running
        if self.doc.simulator.state == SimState::Running {
            let now = Instant::now();
            let elapsed = now.duration_since(self.last_sim_tick);
            let cycle_ms = self.doc.simulator.cycle_time_ms.max(5) as u128;
            if elapsed.as_millis() >= cycle_ms {
                let dt = elapsed.as_millis().min(100) as u32;
                self.doc.step_simulation(dt);
                self.last_sim_tick = now;
            }
            ui.ctx().request_repaint();
        }

        let any_modal_open = self.elem_modal.is_open
            || self.show_about
            || self.show_io_config
            || self.show_code_export
            || self.show_update_plc;

        // Global Keyboard Shortcuts (Ctrl+N / Ctrl+Z / Ctrl+Y work anytime; Arrow/Enter/Del/F5 when not typing & modal closed)
        if !any_modal_open {
            let new_shortcut = ui.input(|i| {
                i.modifiers.command && !i.modifiers.shift && i.key_pressed(egui::Key::N)
            });
            let undo_shortcut = ui.input(|i| {
                i.modifiers.command && !i.modifiers.shift && i.key_pressed(egui::Key::Z)
            });
            let redo_shortcut = ui.input(|i| {
                (i.modifiers.command && i.key_pressed(egui::Key::Y))
                    || (i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::Z))
            });
            if new_shortcut {
                self.new_program();
            } else if undo_shortcut {
                self.doc.undo();
            } else if redo_shortcut {
                self.doc.redo();
            }

            if !ui.ctx().egui_wants_keyboard_input() {
                if ui.input(|i| {
                    i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)
                }) {
                    self.doc.delete_selected_element();
                }
                if ui.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
                    self.doc.select_neighbor_element(-1);
                }
                if ui.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
                    self.doc.select_neighbor_element(1);
                }
                if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                    self.doc.select_neighbor_rung(-1);
                }
                if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                    self.doc.select_neighbor_rung(1);
                }
                if ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::A)) {
                    self.doc.select_all_elements();
                }
                if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    self.open_config_for_selected();
                }
                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.doc.clear_element_selection();
                    self.active_tool = ActiveTool::Select;
                }
                let has_sel = !self.doc.selected_element_ids.is_empty() || self.doc.selected_element_id.is_some();
                if has_sel {
                    if ui.input(|i| !i.modifiers.command && !i.modifiers.ctrl && i.key_pressed(egui::Key::P)) {
                        let next_id = self.doc.program.alloc_id();
                        let new_elem = self.active_tool.create_default_element(next_id)
                            .unwrap_or_else(|| my_canvas_core::LadderElement::new_contact(next_id, "%IX0.0", "PAR", my_canvas_core::ContactMode::NormallyOpen));
                        self.doc.insert_parallel_to_selection(new_elem, false);
                    }
                    if ui.input(|i| !i.modifiers.command && !i.modifiers.ctrl && i.key_pressed(egui::Key::S)) {
                        let next_id = self.doc.program.alloc_id();
                        let new_elem = self.active_tool.create_default_element(next_id)
                            .unwrap_or_else(|| my_canvas_core::LadderElement::new_contact(next_id, "%IX0.0", "SER", my_canvas_core::ContactMode::NormallyOpen));
                        self.doc.insert_series_to_selection(new_elem, true);
                    }
                }
                if ui.input(|i| i.key_pressed(egui::Key::F5)) {
                    if ui.input(|i| i.modifiers.shift) {
                        self.doc.reset_simulation();
                    } else {
                        self.doc.toggle_simulation();
                    }
                }
                if ui.input(|i| i.key_pressed(egui::Key::F6)) {
                    self.doc.single_step_simulation(self.doc.simulator.cycle_time_ms);
                }
            }
        }

        let modal_open = any_modal_open;

        // 1. Top Menu Bar
        Panel::top("top_menu_bar").show(ui, |ui| {
            if modal_open {
                ui.disable();
            }
            menu_bar::render(ui, self);
        });

        // 2. Top Toolbar
        Panel::top("top_toolbar").show(ui, |ui| {
            if modal_open {
                ui.disable();
            }
            toolbar::render(ui, self);
        });

        // 3. Bottom Status Bar
        Panel::bottom("bottom_status_bar").show(ui, |ui| {
            if modal_open {
                ui.disable();
            }
            status_bar::render(ui, self);
        });

        // 4. Left Panel: Ladder Tools & Components
        Panel::left("left_tools_panel")
            .resizable(!modal_open)
            .default_size(200.0)
            .min_size(175.0)
            .max_size(270.0)
            .show(ui, |ui| {
                if modal_open {
                    ui.disable();
                }
                left_panel::render(ui, self);
            });

        // 5. Right Panel: Elements List, Inspector, and Input/Output Ports
        Panel::right("right_properties_panel")
            .resizable(!modal_open)
            .default_size(285.0)
            .min_size(245.0)
            .max_size(450.0)
            .show(ui, |ui| {
                if modal_open {
                    ui.disable();
                }
                right_panel::render(ui, self);
            });

        // 6. Bottom-Center Panel: IEC 61131-3 IL CODE Preview
        if self.show_il_code {
            Panel::bottom("bottom_il_code_panel")
                .resizable(!modal_open)
                .default_size(110.0)
                .min_size(68.0)
                .max_size(250.0)
                .show(ui, |ui| {
                    if modal_open {
                        ui.disable();
                    }
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("IEC 61131-3 IL CODE:")
                                .color(palette.accent_heading)
                                .strong(),
                        );
                        ui.weak("(Live generated Instruction List for active rung / network)");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("[x] Hide").clicked() {
                                self.show_il_code = false;
                                self.save_settings();
                            }
                        });
                    });
                    let il_code = self.doc.generate_il_code();
                    let dynamic_rows = ((ui.available_height() / 16.5).floor() as usize).max(3);
                    egui::ScrollArea::vertical()
                        .id_salt("il_code_scroll")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            let mut code_view = il_code;
                            ui.add(
                                egui::TextEdit::multiline(&mut code_view)
                                    .font(egui::TextStyle::Monospace)
                                    .desired_width(f32::INFINITY)
                                    .desired_rows(dynamic_rows),
                            );
                        });
                });
        }

        // 7. Central Panel: Interactive Ladder Diagram Canvas
        CentralPanel::default()
            .frame(
                egui::Frame::central_panel(ui.style())
                    .inner_margin(0.0)
                    .fill(palette.canvas_bg),
            )
            .show(ui, |ui| {
                if modal_open {
                    ui.disable();
                }
                ui.set_clip_rect(ui.max_rect());
                if let Some(elem_id) = self.canvas.show(
                    ui,
                    &mut self.doc,
                    &mut self.active_tool,
                    self.insert_pos,
                    palette,
                ) {
                    if let Some(el) = self.doc.element_by_id(elem_id).cloned() {
                        let r_comment = self.doc.selected_rung_idx
                            .and_then(|idx| self.doc.program.rungs.get(idx))
                            .map(|r| r.comment.as_str())
                            .unwrap_or("");
                        self.elem_modal.open_for(&el, self.doc.selected_rung_idx, r_comment);
                    }
                }
            });

        // --- Full-Screen Frosted / Dimmed Modal Backdrop Shield for all In-App Dialogs ---
        if modal_open {
            crate::panels::element_dialog::draw_modal_backdrop(ui.ctx(), palette);
        }

        // --- Professional Element Configuration Modal Dialog ---
        self.elem_modal.show(ui.ctx(), &mut self.doc, palette);

        // --- About Dialog ---
        if self.show_about {
            if ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
                self.show_about = false;
            }
            let mut open_about = self.show_about;
            let mut close_about = false;
            Window::new("About uPLC Studio")
                .order(egui::Order::Foreground)
                .open(&mut open_about)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(ui.ctx(), |ui| {
                    ui.horizontal(|ui| {
                        // Microchip Ladder Logic badge (Concept 1)
                        let (rect, _response) = ui.allocate_exact_size(egui::vec2(48.0, 48.0), egui::Sense::hover());
                        let painter = ui.painter_at(rect);
                        // Background squircle chip
                        painter.rect_filled(rect.shrink(4.0), 6.0, egui::Color32::from_rgb(15, 23, 42));
                        painter.rect_stroke(
                            rect.shrink(4.0),
                            6.0,
                            egui::Stroke::new(1.5, egui::Color32::from_rgb(14, 165, 233)),
                            egui::StrokeKind::Inside,
                        );
                        // IC Pins
                        let p = egui::Stroke::new(1.5, egui::Color32::from_rgb(148, 163, 184));
                        painter.line_segment([egui::pos2(rect.left() + 14.0, rect.top()), egui::pos2(rect.left() + 14.0, rect.top() + 4.0)], p);
                        painter.line_segment([egui::pos2(rect.left() + 24.0, rect.top()), egui::pos2(rect.left() + 24.0, rect.top() + 4.0)], p);
                        painter.line_segment([egui::pos2(rect.left() + 34.0, rect.top()), egui::pos2(rect.left() + 34.0, rect.top() + 4.0)], p);
                        painter.line_segment([egui::pos2(rect.left() + 14.0, rect.bottom() - 4.0), egui::pos2(rect.left() + 14.0, rect.bottom())], p);
                        painter.line_segment([egui::pos2(rect.left() + 24.0, rect.bottom() - 4.0), egui::pos2(rect.left() + 24.0, rect.bottom())], p);
                        painter.line_segment([egui::pos2(rect.left() + 34.0, rect.bottom() - 4.0), egui::pos2(rect.left() + 34.0, rect.bottom())], p);
                        // Rails
                        let cy = rect.center().y;
                        painter.line_segment([egui::pos2(rect.left() + 10.0, cy - 12.0), egui::pos2(rect.left() + 10.0, cy + 12.0)], egui::Stroke::new(2.0, egui::Color32::from_rgb(245, 158, 11)));
                        painter.line_segment([egui::pos2(rect.right() - 10.0, cy - 12.0), egui::pos2(rect.right() - 10.0, cy + 12.0)], egui::Stroke::new(2.0, egui::Color32::from_rgb(148, 163, 184)));
                        // Contact & Coil in center
                        painter.line_segment([egui::pos2(rect.left() + 10.0, cy), egui::pos2(rect.left() + 17.0, cy)], egui::Stroke::new(1.5, egui::Color32::from_rgb(0, 240, 255)));
                        painter.line_segment([egui::pos2(rect.left() + 17.0, cy - 6.0), egui::pos2(rect.left() + 17.0, cy + 6.0)], egui::Stroke::new(2.0, egui::Color32::from_rgb(0, 240, 255)));
                        painter.line_segment([egui::pos2(rect.left() + 22.0, cy - 6.0), egui::pos2(rect.left() + 22.0, cy + 6.0)], egui::Stroke::new(2.0, egui::Color32::from_rgb(0, 240, 255)));
                        painter.line_segment([egui::pos2(rect.left() + 22.0, cy), egui::pos2(rect.left() + 28.0, cy)], egui::Stroke::new(1.5, egui::Color32::from_rgb(0, 240, 255)));
                        // Coil
                        let coil_color = egui::Color32::from_rgb(16, 225, 130);
                        painter.circle_stroke(egui::pos2(rect.left() + 32.5, cy), 4.5, egui::Stroke::new(1.5, coil_color));
                        painter.line_segment([egui::pos2(rect.left() + 37.0, cy), egui::pos2(rect.right() - 10.0, cy)], egui::Stroke::new(1.5, egui::Color32::from_rgb(0, 240, 255)));

                        ui.vertical(|ui| {
                            ui.heading("uPLC Studio (MICROPLC Edition)");
                            ui.label("Ladder Program By uPLC Studio [PksofttecH]");
                        });
                    });
                    ui.separator();
                    ui.label("- CAD Corner-Bracket Element Selection & Floating Quick-Action HUD");
                    ui.label("- Professional LD Element Configuration Window (Double-Click / Enter)");
                    ui.label("- Undo / Redo History (Ctrl+Z / Ctrl+Y) & Cross-Rung Drag & Drop");
                    ui.label("- Modern / Dark / Light Theme Support with Thai & Symbol Fonts");
                    ui.add_space(8.0);
                    if ui.button("Close").clicked() {
                        close_about = true;
                    }
                });
            if !open_about || close_about {
                self.show_about = false;
            }
        }

        // --- Settings, Theme & Hardware Configuration Dialog ---
        if self.show_io_config {
            if ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
                self.show_io_config = false;
            }
            let mut open_io_config = self.show_io_config;
            let mut close_io_config = false;
            let mut settings_changed = false;
            Window::new("Settings & Hardware Configuration")
                .order(egui::Order::Foreground)
                .open(&mut open_io_config)
                .min_size([460.0, 360.0])
                .collapsible(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(ui.ctx(), |ui| {
                    ui.label(
                        RichText::new("Appearance & Environment Settings")
                            .color(palette.accent_heading)
                            .strong(),
                    );
                    ui.add_space(4.0);

                    egui::Grid::new("ui_theme_settings_grid")
                        .num_columns(2)
                        .spacing([12.0, 8.0])
                        .show(ui, |ui| {
                            ui.label("UI Theme:");
                            ui.horizontal(|ui| {
                                for t in AppTheme::ALL {
                                    if ui.selectable_label(self.theme == t, t.label()).clicked() {
                                        self.theme = t;
                                        settings_changed = true;
                                    }
                                }
                            });
                            ui.end_row();

                            ui.label("Schematic Style:");
                            egui::ComboBox::from_id_salt("settings_schematic_style_combo")
                                .selected_text(self.doc.schematic_style.label())
                                .show_ui(ui, |ui| {
                                    for st in SchematicStyle::ALL {
                                        let is_cur = self.doc.schematic_style == st;
                                        if ui
                                            .selectable_label(is_cur, st.label())
                                            .on_hover_text(st.description())
                                            .clicked()
                                        {
                                            self.doc.set_schematic_style(st);
                                            settings_changed = true;
                                        }
                                    }
                                });
                            ui.end_row();

                            ui.label("IEC 61131-3 IL Panel:");
                            if ui
                                .checkbox(&mut self.show_il_code, "Show bottom IL code preview")
                                .changed()
                            {
                                settings_changed = true;
                            }
                            ui.end_row();
                        });

                    ui.separator();
                    ui.label(
                        RichText::new("Hardware & Communication")
                            .color(palette.accent_heading)
                            .strong(),
                    );
                    ui.add_space(4.0);

                    egui::Grid::new("opt_cfg_grid")
                        .num_columns(2)
                        .spacing([12.0, 8.0])
                        .show(ui, |ui| {
                            ui.label("Target Hardware:");
                            if ui
                                .text_edit_singleline(&mut self.doc.program.hardware)
                                .changed()
                            {
                                settings_changed = true;
                            }
                            ui.end_row();

                            ui.label("Serial Port:");
                            if ui.text_edit_singleline(&mut self.com_port).changed() {
                                settings_changed = true;
                            }
                            ui.end_row();

                            ui.label("Baud Rate:");
                            egui::ComboBox::from_id_salt("baud_combo")
                                .selected_text(&self.baud_rate)
                                .show_ui(ui, |ui| {
                                    for b in ["9600", "19200", "38400", "57600", "115200"] {
                                        if ui
                                            .selectable_value(&mut self.baud_rate, b.to_string(), b)
                                            .changed()
                                        {
                                            settings_changed = true;
                                        }
                                    }
                                });
                            ui.end_row();

                            ui.label("Scan Cycle Loop (ms):");
                            if ui
                                .add(
                                    egui::DragValue::new(&mut self.doc.simulator.cycle_time_ms)
                                        .range(1..=1000),
                                )
                                .changed()
                            {
                                settings_changed = true;
                            }
                            ui.end_row();
                        });

                    ui.separator();
                    ui.weak(format!(
                        "Settings file: {}",
                        AppSettings::config_path().display()
                    ));
                    ui.add_space(4.0);

                    ui.horizontal(|ui| {
                        if ui.button("+ Add Input Port").clicked() {
                            let n = self
                                .doc
                                .program
                                .io_mappings
                                .iter()
                                .filter(|m| m.is_input)
                                .count();
                            self.doc.program.io_mappings.push(IOPortMapping {
                                plc_address: format!("X{}", n),
                                iec_address: format!("%IX{}.{}", n / 8, n % 8),
                                symbol: format!("IN_{}", n),
                                hw_pin: format!("D{}", n + 2),
                                description: "Digital Input".to_string(),
                                is_input: true,
                            });
                        }
                        if ui.button("+ Add Output Port").clicked() {
                            let n = self
                                .doc
                                .program
                                .io_mappings
                                .iter()
                                .filter(|m| !m.is_input)
                                .count();
                            self.doc.program.io_mappings.push(IOPortMapping {
                                plc_address: format!("Y{}", n),
                                iec_address: format!("%QX{}.{}", n / 8, n % 8),
                                symbol: format!("OUT_{}", n),
                                hw_pin: format!("D{}", n + 8),
                                description: "Digital Output".to_string(),
                                is_input: false,
                            });
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Save & Close").clicked() {
                                self.save_settings();
                                close_io_config = true;
                            }
                            if ui.button("Save Settings").clicked() {
                                self.save_settings();
                            }
                        });
                    });
                });

            if settings_changed {
                self.save_settings();
            }
            if !open_io_config || close_io_config {
                self.show_io_config = false;
            }
        }

        // --- IEC 61131-3 IL Code Preview & Export Dialog ---
        if self.show_code_export {
            if ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
                self.show_code_export = false;
            }
            let mut open_code_export = self.show_code_export;
            let mut close_code_export = false;
            let code = self.doc.export_to_il_str();
            Window::new("IEC 61131-3 IL Code Preview / Export")
                .order(egui::Order::Foreground)
                .open(&mut open_code_export)
                .min_size([580.0, 420.0])
                .collapsible(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(ui.ctx(), |ui| {
                    ui.label("Generated IEC 61131-3 Instruction List (IL) Code (OpenPLC / CODESYS compatible):");
                    ui.separator();
                    egui::ScrollArea::both().max_height(330.0).show(ui, |ui| {
                        let mut text = code.clone();
                        ui.add(
                            egui::TextEdit::multiline(&mut text)
                                .code_editor()
                                .desired_width(f32::INFINITY),
                        );
                    });
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Copy to Clipboard").clicked() {
                            ui.ctx().copy_text(code.clone());
                        }
                        if ui.button("Save .il File...").clicked() {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("IEC 61131-3 Instruction List (*.il)", &["il"])
                                .save_file()
                            {
                                let _ = std::fs::write(path, &code);
                            }
                        }
                        if ui.button("⚡ Compile Bytecode / Update to PLC...").clicked() {
                            close_code_export = true;
                            self.show_update_plc = true;
                        }
                        if ui.button("Close").clicked() {
                            close_code_export = true;
                        }
                    });
                });
            if !open_code_export || close_code_export {
                self.show_code_export = false;
            }
        }

        // --- Update to PLC (IL -> 32-bit Micro-VM Bytecode & Serial Upload) Dialog ---
        if self.show_update_plc {
            if ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
                self.show_update_plc = false;
            }
            let mut open_update_plc = self.show_update_plc;
            let mut close_update_plc = false;
            let mut settings_changed = false;
            let mut trigger_cmd: Option<my_canvas_core::PlcSerialCommand> = None;

            let bc = self.doc.compile_to_bytecode();
            let packet = bc.to_serial_packet(my_canvas_core::PlcSerialCommand::DownloadProgram);
            let crc8 = packet.last().copied().unwrap_or(0);

            Window::new("⚡ Update to PLC (IL → Micro-VM Bytecode)")
                .order(egui::Order::Foreground)
                .open(&mut open_update_plc)
                .min_size([680.0, 480.0])
                .default_size([720.0, 520.0])
                .collapsible(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(ui.ctx(), |ui| {
                    // 1. Connection & Hardware Header Bar
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Serial Port:")
                                .color(palette.accent_heading)
                                .strong(),
                        );
                        if ui
                            .add(egui::TextEdit::singleline(&mut self.com_port).desired_width(135.0))
                            .changed()
                        {
                            settings_changed = true;
                        }
                        egui::ComboBox::from_id_salt("update_plc_port_combo")
                            .width(120.0)
                            .selected_text("Detect Port")
                            .show_ui(ui, |ui| {
                                for p in Self::available_serial_ports() {
                                    if ui
                                        .selectable_value(&mut self.com_port, p.clone(), &p)
                                        .changed()
                                    {
                                        settings_changed = true;
                                    }
                                }
                            });

                        ui.label(
                            RichText::new("Baud:")
                                .color(palette.accent_heading)
                                .strong(),
                        );
                        egui::ComboBox::from_id_salt("update_plc_baud_combo")
                            .width(82.0)
                            .selected_text(&self.baud_rate)
                            .show_ui(ui, |ui| {
                                for b in ["9600", "19200", "38400", "57600", "115200"] {
                                    if ui
                                        .selectable_value(&mut self.baud_rate, b.to_string(), b)
                                        .changed()
                                    {
                                        settings_changed = true;
                                    }
                                }
                            });

                        ui.separator();
                        ui.label(
                            RichText::new(format!(
                                "{} Inst ({} B) | Frame: {} B | CRC8: 0x{:02X}",
                                bc.instructions.len(),
                                bc.payload_size_bytes(),
                                packet.len(),
                                crc8
                            ))
                            .monospace()
                            .size(11.5)
                            .color(palette.accent_ok),
                        );
                    });

                    ui.separator();

                    // 2. View Mode Tabs
                    ui.horizontal(|ui| {
                        if ui
                            .selectable_label(
                                self.update_plc_tab == 0,
                                "1. 32-Bit Bytecode & Packet Frame",
                            )
                            .clicked()
                        {
                            self.update_plc_tab = 0;
                        }
                        if ui
                            .selectable_label(
                                self.update_plc_tab == 1,
                                "2. Unified Memory Mapping (0x00..0xFF)",
                            )
                            .clicked()
                        {
                            self.update_plc_tab = 1;
                        }
                    });
                    ui.separator();

                    // 3. Tab Content
                    match self.update_plc_tab {
                        0 => {
                            let disasm = bc.to_disassembly_text();
                            egui::ScrollArea::both()
                                .id_salt("bytecode_disasm_scroll")
                                .max_height(310.0)
                                .show(ui, |ui| {
                                    let mut text = disasm;
                                    ui.add(
                                        egui::TextEdit::multiline(&mut text)
                                            .code_editor()
                                            .desired_width(f32::INFINITY),
                                    );
                                });
                        }
                        _ => {
                            ui.label(
                                RichText::new(
                                    "Unified Memory Space: X[0x00..0x1F] | Y[0x20..0x3F] | M[0x40..0xBF] | T[0xC0..0xDF] | C[0xE0..0xFF]",
                                )
                                .monospace()
                                .size(11.5)
                                .color(palette.accent_heading),
                            );
                            ui.add_space(4.0);
                            egui::ScrollArea::vertical()
                                .id_salt("unified_mem_map_scroll")
                                .max_height(285.0)
                                .show(ui, |ui| {
                                    egui::Grid::new("unified_mem_map_grid")
                                        .striped(true)
                                        .num_columns(6)
                                        .spacing([14.0, 6.0])
                                        .show(ui, |ui| {
                                            ui.strong("Unified Addr");
                                            ui.strong("Canonical");
                                            ui.strong("IEC 61131-3");
                                            ui.strong("Symbol");
                                            ui.strong("MCU Pin");
                                            ui.strong("VM Memory Region");
                                            ui.end_row();

                                            for entry in &bc.memory_map {
                                                ui.monospace(format!("0x{:02X} ({:3})", entry.addr, entry.addr));
                                                ui.monospace(&entry.canonical_name);
                                                ui.monospace(&entry.iec_address);
                                                ui.label(&entry.symbol);
                                                ui.monospace(if entry.hw_pin.is_empty() {
                                                    "-"
                                                } else {
                                                    &entry.hw_pin
                                                });
                                                ui.weak(entry.region.space_label());
                                                ui.end_row();
                                            }
                                        });
                                });
                        }
                    }

                    // 4. Status Banner
                    if !self.update_plc_status.is_empty() {
                        ui.separator();
                        let status_color = if self.update_plc_is_error {
                            palette.accent_warn
                        } else {
                            palette.accent_ok
                        };
                        ui.label(
                            RichText::new(&self.update_plc_status)
                                .monospace()
                                .size(11.5)
                                .color(status_color),
                        );
                    }

                    ui.separator();

                    // 5. Action Buttons
                    ui.horizontal(|ui| {
                        let upload_btn = egui::Button::new(
                            RichText::new("⚡ Update to PLC (Send via Serial)")
                                .strong()
                                .color(palette.accent_ok),
                        );
                        if ui
                            .add(upload_btn)
                            .on_hover_text("Transmit Packet Frame (SOF 0xAA + CMD 0x01 + LEN + Bytecode + CRC8) via USB Serial Port")
                            .clicked()
                        {
                            trigger_cmd = Some(my_canvas_core::PlcSerialCommand::DownloadProgram);
                        }

                        if ui
                            .button("▶ Send RUN (0x02)")
                            .on_hover_text("Send CMD 0x02 to start PLC scan execution on MCU")
                            .clicked()
                        {
                            trigger_cmd = Some(my_canvas_core::PlcSerialCommand::RunStopControl);
                        }

                        if ui.button("Copy Packet Hex").clicked() {
                            ui.ctx().copy_text(
                                bc.to_packet_hex_string(my_canvas_core::PlcSerialCommand::DownloadProgram),
                            );
                            self.update_plc_is_error = false;
                            self.update_plc_status = format!(
                                "✔ Copied {} Bytes Serial Packet Hex to clipboard",
                                packet.len()
                            );
                        }

                        if ui.button("Save Bytecode (.bin)...").clicked() {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("Micro-VM Bytecode Binary (*.bin)", &["bin"])
                                .save_file()
                            {
                                let raw = bc.to_raw_bytes();
                                if std::fs::write(&path, &raw).is_ok() {
                                    self.update_plc_is_error = false;
                                    self.update_plc_status = format!(
                                        "✔ Saved {} Bytes raw bytecode to {}",
                                        raw.len(),
                                        path.display()
                                    );
                                }
                            }
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Close").clicked() {
                                close_update_plc = true;
                            }
                        });
                    });
                });

            if let Some(cmd) = trigger_cmd {
                self.transmit_bytecode_to_plc(cmd);
            }
            if settings_changed {
                self.save_settings();
            }
            if !open_update_plc || close_update_plc {
                self.show_update_plc = false;
            }
        }
    }

    fn on_exit(&mut self) {
        if self.window_state_dirty {
            self.window_state_dirty = false;
            self.save_settings();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_settings_theme_serialization() {
        for theme in AppTheme::ALL {
            for schematic_style in SchematicStyle::ALL {
                let settings = AppSettings {
                    theme,
                    schematic_style,
                    show_il_code: false,
                    com_port: "ttyACM0".to_string(),
                    baud_rate: "57600".to_string(),
                    cycle_time_ms: 20,
                    hardware: "Arduino NANO".to_string(),
                    window_width: Some(1600.0),
                    window_height: Some(900.0),
                    window_maximized: true,
                    window_pos_x: Some(100.0),
                    window_pos_y: Some(50.0),
                };
                let json = serde_json::to_string_pretty(&settings).unwrap();
                let loaded: AppSettings = serde_json::from_str(&json).unwrap();
                assert_eq!(loaded.theme, theme);
                assert_eq!(loaded.schematic_style, schematic_style);
                assert!(!loaded.show_il_code);
                assert_eq!(loaded.com_port, "ttyACM0");
                assert_eq!(loaded.baud_rate, "57600");
                assert_eq!(loaded.cycle_time_ms, 20);
                assert_eq!(loaded.hardware, "Arduino NANO");
                assert_eq!(loaded.window_width, Some(1600.0));
                assert_eq!(loaded.window_height, Some(900.0));
                assert!(loaded.window_maximized);
                assert_eq!(loaded.window_pos_x, Some(100.0));
                assert_eq!(loaded.window_pos_y, Some(50.0));
            }
        }
    }

    #[test]
    fn test_app_settings_backward_compatibility() {
        let legacy_json = r#"{
            "theme": "Dark",
            "schematic_style": "DinEuropean",
            "show_plclib_code": true,
            "com_port": "ttyUSB0",
            "baud_rate": "115200",
            "cycle_time_ms": 10,
            "hardware": "Arduino UNO"
        }"#;
        let loaded: AppSettings = serde_json::from_str(legacy_json).unwrap();
        assert_eq!(loaded.clamped_window_size(), (1280.0, 720.0));
        assert!(loaded.show_il_code);
        assert!(!loaded.window_maximized);
        assert_eq!(loaded.window_pos_x, None);
        assert_eq!(loaded.window_pos_y, None);
    }

    #[test]
    fn test_canvas_with_scrollbars() {
        let ctx = egui::Context::default();
        let mut raw_input = egui::RawInput::default();
        raw_input.screen_rect = Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1000.0, 700.0)));
        let mut output = ctx.run_ui(raw_input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let mut canvas = CanvasViewport::default();
                let mut doc = Document::default();
                for _ in 0..20 {
                    doc.add_rung();
                }
                let mut tool = ActiveTool::Select;
                let palette = AppTheme::Dark.palette();
                let _ = canvas.show(ui, &mut doc, &mut tool, InsertPosition::AppendRung, palette);
                let content_sz = canvas.compute_content_size(&doc, 1000.0);
                assert!(content_sz.y > 2000.0, "20 rungs should provide large scrollable content height");
            });
        });
        output.textures_delta.clear();
    }

    #[test]
    fn test_scrollbar_pointer_isolation() {
        let ctx = egui::Context::default();
        let mut raw_input = egui::RawInput::default();
        raw_input.screen_rect = Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1000.0, 700.0)));
        raw_input.events.push(egui::Event::PointerMoved(egui::pos2(995.0, 300.0)));
        let mut output = ctx.run_ui(raw_input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let mut canvas = CanvasViewport::default();
                let mut doc = Document::default();
                for _ in 0..20 {
                    doc.add_rung();
                }
                let mut tool = ActiveTool::Select;
                let palette = AppTheme::Dark.palette();
                let clicked = canvas.show(ui, &mut doc, &mut tool, InsertPosition::AppendRung, palette);
                assert_eq!(clicked, None);
                assert_eq!(canvas.marquee_start, None);
            });
        });
        output.textures_delta.clear();
    }
}



