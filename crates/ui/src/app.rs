use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver},
    Arc,
};
use std::thread;
use std::time::{Duration, Instant};
use egui::{CentralPanel, Color32, Panel, RichText, Ui, Window};
use my_canvas_core::{Document, IOPortMapping, InsertPosition, PlcStatusSnapshot, SchematicStyle, SimState};
use serde::{Deserialize, Serialize};
use crate::canvas::CanvasViewport;
use crate::panels::{element_dialog::ElementConfigModal, left_panel, menu_bar, right_panel, status_bar, toolbar};
use crate::theme::{self, AppTheme};
use crate::tools::ActiveTool;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WatchDisplayFormat {
    Auto,
    Dec,
    Hex,
    Bin,
    Bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchRow {
    pub address: String,
    pub symbol: String,
    pub format: WatchDisplayFormat,
    #[serde(skip)]
    pub edit_buffer: String,
}

pub fn default_watch_rows() -> Vec<WatchRow> {
    vec![
        WatchRow {
            address: "%IX0.0".to_string(),
            symbol: "START_PB".to_string(),
            format: WatchDisplayFormat::Bool,
            edit_buffer: String::new(),
        },
        WatchRow {
            address: "%QX0.0".to_string(),
            symbol: "RUN_LED".to_string(),
            format: WatchDisplayFormat::Bool,
            edit_buffer: String::new(),
        },
        WatchRow {
            address: "%IW0".to_string(),
            symbol: "ADC0_GP26".to_string(),
            format: WatchDisplayFormat::Dec,
            edit_buffer: String::new(),
        },
        WatchRow {
            address: "%IW3".to_string(),
            symbol: "ADC_TEMP".to_string(),
            format: WatchDisplayFormat::Dec,
            edit_buffer: String::new(),
        },
        WatchRow {
            address: "%MW0".to_string(),
            symbol: "VAR_0".to_string(),
            format: WatchDisplayFormat::Dec,
            edit_buffer: String::new(),
        },
        WatchRow {
            address: "%SD1".to_string(),
            symbol: "SCAN_TIME_US".to_string(),
            format: WatchDisplayFormat::Dec,
            edit_buffer: String::new(),
        },
        WatchRow {
            address: "%SD6".to_string(),
            symbol: "CPU_TEMP".to_string(),
            format: WatchDisplayFormat::Dec,
            edit_buffer: String::new(),
        },
    ]
}

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
    pub online_target_hw: bool,
    pub window_width: Option<f32>,
    pub window_height: Option<f32>,
    pub window_maximized: bool,
    pub window_pos_x: Option<f32>,
    pub window_pos_y: Option<f32>,
    pub last_folder: Option<PathBuf>,
    pub watch_rows: Vec<WatchRow>,
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
            online_target_hw: true,
            window_width: Some(1280.0),
            window_height: Some(720.0),
            window_maximized: false,
            window_pos_x: None,
            window_pos_y: None,
            last_folder: None,
            watch_rows: default_watch_rows(),
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingAction {
    NewProgram,
    OpenProgram,
    LoadExample(String),
    ExitApp,
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
    pub special_reg_filter_text: String,
    pub special_reg_filter_cat: u8,
    pub watch_rows: Vec<WatchRow>,
    pub watch_filter_text: String,
    pub watch_new_addr: String,
    pub watch_new_symbol: String,
    pub right_panel_tab: u8,
    pub show_il_code: bool,
    pub show_about: bool,
    pub show_code_export: bool,
    pub show_io_config: bool,
    pub show_update_plc: bool,
    pub update_plc_tab: u8,
    pub update_plc_status: String,
    pub update_plc_is_error: bool,
    pub plc_remote_running: bool,
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
    pub online_mode: bool,
    pub online_target_hw: bool,
    pub online_status_text: String,
    pub latest_snapshot: Option<PlcStatusSnapshot>,
    pub last_telemetry_rx: Option<Instant>,
    pub online_worker_stop: Arc<AtomicBool>,
    pub online_worker_rx: Option<Receiver<(PlcStatusSnapshot, String)>>,
    pub online_worker_cmd_tx: Option<mpsc::Sender<Vec<u8>>>,
    pub last_folder: Option<PathBuf>,
    pub pending_action: Option<PendingAction>,
    pub allow_exit: bool,
    pub current_window_title: String,
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
            special_reg_filter_text: String::new(),
            special_reg_filter_cat: 0,
            watch_rows: if settings.watch_rows.is_empty() {
                default_watch_rows()
            } else {
                settings.watch_rows
            },
            watch_filter_text: String::new(),
            watch_new_addr: String::new(),
            watch_new_symbol: String::new(),
            right_panel_tab: 0,
            show_il_code: settings.show_il_code,
            show_about: false,
            show_code_export: false,
            show_io_config: false,
            show_update_plc: false,
            update_plc_tab: 0,
            update_plc_status: String::new(),
            update_plc_is_error: false,
            plc_remote_running: true,
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
            online_mode: false,
            online_target_hw: settings.online_target_hw,
            online_status_text: "Offline".to_string(),
            latest_snapshot: None,
            last_telemetry_rx: None,
            online_worker_stop: Arc::new(AtomicBool::new(false)),
            online_worker_rx: None,
            online_worker_cmd_tx: None,
            last_folder: settings.last_folder,
            pending_action: None,
            allow_exit: false,
            current_window_title: String::new(),
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
                        let prod_path = format!("/sys/class/tty/{}/device/../product", name);
                        let label = if let Ok(prod) = std::fs::read_to_string(&prod_path) {
                            let clean = prod.trim();
                            if !clean.is_empty() {
                                format!("/dev/{} ({})", name, clean)
                            } else {
                                format!("/dev/{}", name)
                            }
                        } else {
                            format!("/dev/{}", name)
                        };
                        ports.push(label);
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

    /// Find a connected RP2040 Micro-VM USB-CDC port (`/dev/ttyACM*` or `/dev/ttyUSB*`)
    pub fn detect_uplc_usb_port() -> Option<String> {
        if let Ok(entries) = std::fs::read_dir("/dev") {
            let mut candidates: Vec<String> = entries
                .flatten()
                .filter_map(|e| e.file_name().into_string().ok())
                .filter(|n| n.starts_with("ttyACM") || n.starts_with("ttyUSB"))
                .collect();
            candidates.sort();

            // Priority 1: device whose product description contains "uPLC", "RP2040", or "Pico", and is not a Debugprobe
            for name in &candidates {
                let prod_path = format!("/sys/class/tty/{}/device/../product", name);
                if let Ok(prod) = std::fs::read_to_string(&prod_path) {
                    if !prod.contains("Debugprobe")
                        && (prod.contains("uPLC") || prod.contains("RP2040") || prod.contains("Pico"))
                    {
                        return Some(format!("/dev/{}", name));
                    }
                }
            }

            // Priority 2: any ttyACM or ttyUSB that is explicitly not a Debugprobe
            for name in &candidates {
                let prod_path = format!("/sys/class/tty/{}/device/../product", name);
                let is_debugprobe = std::fs::read_to_string(&prod_path)
                    .map(|p| p.contains("Debugprobe"))
                    .unwrap_or(false);
                if !is_debugprobe {
                    return Some(format!("/dev/{}", name));
                }
            }
        }
        None
    }

    /// Read the compiled bytecode directly from the connected PLC Micro-VM via CMD 0x04
    /// and decompile it back into an active `LadderProgram` (Rungs & Elements).
    pub fn upload_program_from_plc(&mut self) -> Result<my_canvas_core::LadderProgram, String> {
        let was_online = self.online_mode && self.online_target_hw;
        if was_online {
            self.stop_online_monitoring();
            thread::sleep(Duration::from_millis(60));
        }

        let raw_port = self.com_port.split_whitespace().next().unwrap_or("/dev/ttyACM1");
        let mut port_path = if raw_port.starts_with("tty") {
            format!("/dev/{}", raw_port)
        } else {
            raw_port.to_string()
        };

        let is_debugprobe = port_path
            .strip_prefix("/dev/")
            .and_then(|n| std::fs::read_to_string(format!("/sys/class/tty/{}/device/../product", n)).ok())
            .map(|p| p.contains("Debugprobe"))
            .unwrap_or(false);
        if (!std::path::Path::new(&port_path).exists() || is_debugprobe)
            && Self::detect_uplc_usb_port().is_some()
        {
            if let Some(detected) = Self::detect_uplc_usb_port() {
                port_path = detected.clone();
                self.com_port = detected;
            }
        }

        if !std::path::Path::new(&port_path).exists() {
            if was_online {
                self.start_online_monitoring();
            }
            return Err(format!("Serial port '{}' not connected", port_path));
        }

        // Configure raw mode via stty on Linux
        let _ = std::process::Command::new("stty")
            .args(["-F", &port_path, &self.baud_rate, "raw", "-echo", "-hupcl", "min", "0", "time", "2"])
            .output();

        let res = (|| -> Result<my_canvas_core::LadderProgram, String> {
            use std::io::{Read, Write};
            let mut file = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&port_path)
                .map_err(|e| format!("Failed to open {}: {}", port_path, e))?;

            // Drain any pending bytes
            let mut drain = [0u8; 512];
            let _ = file.read(&mut drain);

            // Construct CMD 0x04 request packet: [0xAA, 0x04, 0x00, 0x00, CRC8]
            let dummy_bc = my_canvas_core::BytecodeProgram {
                hardware: self.doc.program.hardware.clone(),
                instructions: Vec::new(),
                memory_map: Vec::new(),
            };
            let req = dummy_bc.to_serial_packet(my_canvas_core::PlcSerialCommand::UploadProgram);

            file.write_all(&req)
                .map_err(|e| format!("Failed to write to {}: {}", port_path, e))?;
            file.flush()
                .map_err(|e| format!("Failed to flush {}: {}", port_path, e))?;

            let mut rx_accum = Vec::with_capacity(2048);
            let mut chunk = [0u8; 512];
            let start = Instant::now();

            while start.elapsed().as_millis() < 800 {
                match file.read(&mut chunk) {
                    Ok(0) => thread::sleep(Duration::from_millis(10)),
                    Ok(n) => {
                        rx_accum.extend_from_slice(&chunk[..n]);
                        if let Some(pos) = rx_accum.windows(2).position(|w| w == [0xAA, 0x84]) {
                            if pos > 0 {
                                rx_accum.drain(..pos);
                            }
                            if rx_accum.len() >= 4 {
                                let plen = u16::from_le_bytes([rx_accum[2], rx_accum[3]]) as usize;
                                let frame_len = 5 + plen;
                                if rx_accum.len() >= frame_len {
                                    let frame: Vec<u8> = rx_accum.drain(..frame_len).collect();
                                    let uploaded_bc = my_canvas_core::BytecodeProgram::parse_upload_packet(
                                        &frame,
                                        &self.doc.program.hardware,
                                    )?;
                                    return uploaded_bc.decompile_to_ladder();
                                }
                            }
                        }
                    }
                    Err(e) => return Err(format!("Serial read error: {}", e)),
                }
            }

            Err("Timed out waiting for CMD 0x84 program response from PLC".to_string())
        })();

        if was_online {
            self.online_mode = false;
            self.start_online_monitoring();
        }

        res
    }

    pub fn transmit_bytecode_to_plc(&mut self, cmd: my_canvas_core::PlcSerialCommand) {
        self.transmit_packet_internal(cmd, None);
    }

    pub fn transmit_custom_packet_to_plc(&mut self, cmd: my_canvas_core::PlcSerialCommand, packet: Vec<u8>) {
        self.transmit_packet_internal(cmd, Some(packet));
    }

    fn transmit_packet_internal(&mut self, cmd: my_canvas_core::PlcSerialCommand, custom_packet: Option<Vec<u8>>) {
        use std::io::{Read, Write};
        let bc = self.doc.compile_to_bytecode();
        if cmd == my_canvas_core::PlcSerialCommand::UploadProgram {
            match self.upload_program_from_plc() {
                Ok(ladder) => {
                    let n_rungs = ladder.rungs.len();
                    self.doc.program = ladder;
                    self.doc.selected_rung_idx = Some(0);
                    self.update_plc_is_error = false;
                    self.update_plc_status = format!(
                        "✔ Successfully uploaded & decompiled program from PLC ({} Networks)",
                        n_rungs
                    );
                }
                Err(e) => {
                    self.update_plc_is_error = true;
                    self.update_plc_status = format!("✖ Upload from PLC failed: {}", e);
                }
            }
            return;
        }

        let packet = custom_packet.unwrap_or_else(|| bc.to_serial_packet(cmd));

        // Fast-path: If Hardware Online Monitoring is active and this is a Run/Stop/Step command, route through cmd channel
        if self.online_mode && self.online_target_hw {
            if let Some(ref tx) = self.online_worker_cmd_tx {
                if cmd != my_canvas_core::PlcSerialCommand::DownloadProgram
                    && cmd != my_canvas_core::PlcSerialCommand::UploadProgram
                {
                    if tx.send(packet.clone()).is_ok() {
                        match cmd {
                            my_canvas_core::PlcSerialCommand::RunStopControl => {
                                self.plc_remote_running = true;
                                self.update_plc_status = "▶ PLC RUN Command sent to hardware".to_string();
                            }
                            my_canvas_core::PlcSerialCommand::StopPlc => {
                                self.plc_remote_running = false;
                                self.update_plc_status = "⏹ PLC STOP Command sent to hardware".to_string();
                            }
                            my_canvas_core::PlcSerialCommand::StepPlc => {
                                self.plc_remote_running = false;
                                self.update_plc_status = "⏭ PLC STEP Command sent to hardware".to_string();
                            }
                            my_canvas_core::PlcSerialCommand::ForceIo => {
                                self.update_plc_status = "⚡ Force I/O Command sent to hardware".to_string();
                            }
                            my_canvas_core::PlcSerialCommand::WriteValue => {
                                self.update_plc_status = "✍ Write Value Command sent to hardware".to_string();
                            }
                            _ => {}
                        }
                        return;
                    }
                }
            }
        }

        // If Hardware Online Monitoring is currently polling the serial port, pause it during transmission
        let resume_online_hw = self.online_mode && self.online_target_hw;
        if resume_online_hw {
            self.online_worker_stop.store(true, Ordering::Relaxed);
            self.online_worker_rx = None;
            self.online_worker_cmd_tx = None;
            thread::sleep(Duration::from_millis(60));
        }

        let raw_port = self.com_port.split_whitespace().next().unwrap_or("/dev/ttyACM0");
        let mut port_path = if raw_port.starts_with("tty") {
            format!("/dev/{}", raw_port)
        } else {
            raw_port.to_string()
        };

        // Auto-select the RP2040 Micro-VM USB-CDC port if the current port doesn't exist or is a Debugprobe
        let is_debugprobe = port_path
            .strip_prefix("/dev/")
            .and_then(|n| std::fs::read_to_string(format!("/sys/class/tty/{}/device/../product", n)).ok())
            .map(|p| p.contains("Debugprobe"))
            .unwrap_or(false);
        if (!std::path::Path::new(&port_path).exists() || is_debugprobe)
            && Self::detect_uplc_usb_port().is_some()
        {
            if let Some(detected) = Self::detect_uplc_usb_port() {
                port_path = detected.clone();
                self.com_port = detected;
            }
        }

        // Configure raw mode via stty on Linux if device exists
        if std::path::Path::new(&port_path).exists() {
            let _ = std::process::Command::new("stty")
                .args(["-F", &port_path, &self.baud_rate, "raw", "-echo", "-hupcl", "min", "0", "time", "1"])
                .output();
        }

        match std::fs::OpenOptions::new().read(true).write(true).open(&port_path) {
            Ok(mut file) => match file.write_all(&packet) {
                Ok(_) => {
                    let _ = file.flush();
                    // Briefly drain any 3-byte ACK ([0xAA, 0x81/0x82/0x85, 0x00]) so RX buffer stays clean
                    thread::sleep(Duration::from_millis(15));
                    let mut ack_drain = [0u8; 32];
                    let _ = file.read(&mut ack_drain);

                    self.update_plc_is_error = false;
                    match cmd {
                        my_canvas_core::PlcSerialCommand::DownloadProgram => {
                            self.plc_remote_running = true;
                            let (_, prog_crc) = bc.program_signature();
                            self.update_plc_status = format!(
                                "✔ Uploaded {} Bytes ({} inst, CRC16=0x{:04X}) to {} @ {} baud — Burned to Flash (0x1FE000) & PLC RUNNING",
                                packet.len(),
                                bc.instructions.len(),
                                prog_crc,
                                port_path,
                                self.baud_rate
                            );
                        }
                        my_canvas_core::PlcSerialCommand::RunStopControl => {
                            self.plc_remote_running = true;
                            self.update_plc_status = format!(
                                "▶ PLC RUN Command (0x02, payload=0x01) sent to {} — Scan Cycle Active",
                                port_path
                            );
                        }
                        my_canvas_core::PlcSerialCommand::StopPlc => {
                            self.plc_remote_running = false;
                            self.update_plc_status = format!(
                                "⏹ PLC STOP Command (0x02, payload=0x00) sent to {} — All Outputs Safely Turned OFF",
                                port_path
                            );
                        }
                        my_canvas_core::PlcSerialCommand::StepPlc => {
                            self.plc_remote_running = false;
                            self.update_plc_status = format!(
                                "⏭ PLC STEP Command (0x05) sent to {} — Single Scan Executed",
                                port_path
                            );
                        }
                        my_canvas_core::PlcSerialCommand::ForceIo => {
                            self.update_plc_status = format!("⚡ Force I/O Command sent to {}", port_path);
                        }
                        my_canvas_core::PlcSerialCommand::WriteValue => {
                            self.update_plc_status = format!("✍ Write Value Command sent to {}", port_path);
                        }
                        my_canvas_core::PlcSerialCommand::MonitorStatus => {
                            self.update_plc_status = format!(
                                "✔ Status Monitor Request (0x03) sent to {}",
                                port_path
                            );
                        }
                        my_canvas_core::PlcSerialCommand::UploadProgram => {}
                    }
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

        if resume_online_hw {
            self.online_mode = false;
            self.start_online_monitoring();
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
            online_target_hw: self.online_target_hw,
            window_width: self.window_width,
            window_height: self.window_height,
            window_maximized: self.window_maximized,
            window_pos_x: self.window_pos_x,
            window_pos_y: self.window_pos_y,
            last_folder: self.last_folder.clone(),
            watch_rows: self.watch_rows.clone(),
        }
    }

    /// Evaluates live value and display styling for any address in the Watch Table
    pub fn evaluate_watch_value(
        &self,
        addr_str: &str,
        format: WatchDisplayFormat,
    ) -> (String, bool, egui::Color32) {
        let palette = self.theme.palette();
        let upper = addr_str.trim().to_ascii_uppercase();

        // 1. Bit Addresses (%IX, %QX, %MX, %SM, X, Y, M, etc.)
        let is_bit_addr = upper.starts_with("%IX")
            || upper.starts_with("%QX")
            || upper.starts_with("%MX")
            || upper.starts_with("%SM")
            || ((upper.starts_with('X') || upper.starts_with('Y') || upper.starts_with('M') || upper.starts_with('I') || upper.starts_with('Q') || upper.starts_with('R'))
                && upper.len() > 1
                && upper[1..].chars().all(|c| c.is_ascii_digit()));

        if is_bit_addr {
            let is_on = self.doc.simulator.get_var_value(&upper);
            let color = if is_on { palette.accent_ok } else { palette.rung_text_dim };
            let text = match format {
                WatchDisplayFormat::Bool | WatchDisplayFormat::Auto => {
                    if is_on { "● ON (1)".to_string() } else { "○ OFF (0)".to_string() }
                }
                WatchDisplayFormat::Dec => if is_on { "1".to_string() } else { "0".to_string() },
                WatchDisplayFormat::Hex => if is_on { "0x01".to_string() } else { "0x00".to_string() },
                WatchDisplayFormat::Bin => if is_on { "0b1".to_string() } else { "0b0".to_string() },
            };
            return (text, true, color);
        }

        // 2. Timers (T0..T31)
        if upper.starts_with('T') && upper.len() > 1 && upper[1..].chars().all(|c| c.is_ascii_digit()) {
            if let Some(&(cv, done)) = self.doc.simulator.timers.get(&upper) {
                let color = if done { palette.accent_ok } else { palette.accent_heading };
                let text = format!("{} ms (Q={})", cv, if done { 1 } else { 0 });
                return (text, false, color);
            }
        }

        // 3. Counters (C0..C31)
        if upper.starts_with('C') && upper.len() > 1 && upper[1..].chars().all(|c| c.is_ascii_digit()) {
            if let Some(&(cv, done, _)) = self.doc.simulator.counters.get(&upper) {
                let color = if done { palette.accent_ok } else { palette.accent_heading };
                let text = format!("{} (Q={})", cv, if done { 1 } else { 0 });
                return (text, false, color);
            }
        }

        // 4. Word Registers (%MW, D, %IW, AI, %QW, AO, %SD, SD)
        let reg_val = self.doc.simulator.registers.get(&upper).copied();
        if let Some(v) = reg_val {
            let color = palette.accent_heading;
            let text = match format {
                WatchDisplayFormat::Auto => {
                    if upper == "%SD6" || upper == "SD6" || upper == "D134" || upper == "D8006" {
                        format!("{:.1} °C ({})", (v as f32) / 10.0, v)
                    } else if upper == "%SD1" || upper == "%SD2" || upper == "%SD3" {
                        format!("{} µs", v)
                    } else if upper.starts_with("%IW") || upper.starts_with("AI") || upper.starts_with("ADC") {
                        let volt = (v.clamp(0, 4095) as f32 * 3.3) / 4095.0;
                        format!("{} ({:.2}V)", v, volt)
                    } else {
                        format!("{}", v)
                    }
                }
                WatchDisplayFormat::Dec => format!("{}", v),
                WatchDisplayFormat::Hex => format!("0x{:04X}", (v as u32) & 0xFFFF),
                WatchDisplayFormat::Bin => format!("0b{:016b}", (v as u32) & 0xFFFF),
                WatchDisplayFormat::Bool => {
                    if v != 0 { "● TRUE (1)".to_string() } else { "○ FALSE (0)".to_string() }
                }
            };
            return (text, false, color);
        }

        ("-".to_string(), false, palette.rung_text_dim)
    }

    /// Write or force a value to a watched row by index
    pub fn write_watch_row(&mut self, row_idx: usize, val_str: &str) -> Result<(), &'static str> {
        if row_idx >= self.watch_rows.len() {
            return Err("Index out of bounds");
        }
        let addr = self.watch_rows[row_idx].address.trim().to_ascii_uppercase();
        let val_clean = val_str.trim();
        if val_clean.is_empty() {
            return Err("Empty value");
        }

        let is_bool_val = match val_clean.to_ascii_lowercase().as_str() {
            "true" | "1" | "on" | "high" => Some(true),
            "false" | "0" | "off" | "low" => Some(false),
            _ => None,
        };

        if let Some((vm_addr, _name, region)) = my_canvas_core::BytecodeCompiler::parse_direct_address(&addr, false) {
            match region {
                my_canvas_core::MemoryRegion::DigitalInputX
                | my_canvas_core::MemoryRegion::DigitalOutputY
                | my_canvas_core::MemoryRegion::InternalRelayM => {
                    let b_val = is_bool_val.unwrap_or_else(|| {
                        val_clean.parse::<i32>().map(|n| n != 0).unwrap_or(false)
                    });
                    self.doc.simulator.set_var_value(&addr, b_val);
                    self.force_bit(vm_addr, b_val);
                    return Ok(());
                }
                _ => {}
            }
        }

        let num_val = if let Some(hex) = val_clean.strip_prefix("0x").or_else(|| val_clean.strip_prefix("0X")) {
            i32::from_str_radix(hex, 16).map_err(|_| "Invalid Hex number")?
        } else if let Some(bin) = val_clean.strip_prefix("0b").or_else(|| val_clean.strip_prefix("0B")) {
            i32::from_str_radix(bin, 2).map_err(|_| "Invalid Binary number")?
        } else {
            val_clean.parse::<i32>().map_err(|_| "Invalid Decimal number")?
        };

        if let Some((vm_addr, _name, _)) = my_canvas_core::BytecodeCompiler::parse_direct_address(&addr, true) {
            self.write_word_register(vm_addr, num_val);
            self.doc.simulator.registers.insert(addr, num_val);
            Ok(())
        } else {
            self.doc.simulator.registers.insert(addr, num_val);
            Ok(())
        }
    }

    /// Add a new row to the watch table
    pub fn add_watch_row(&mut self, addr: &str, symbol: &str, format: WatchDisplayFormat) {
        let clean_addr = addr.trim().to_ascii_uppercase();
        if clean_addr.is_empty() {
            return;
        }
        if !self.watch_rows.iter().any(|r| r.address.eq_ignore_ascii_case(&clean_addr)) {
            self.watch_rows.push(WatchRow {
                address: clean_addr,
                symbol: symbol.trim().to_string(),
                format,
                edit_buffer: String::new(),
            });
            self.save_settings();
        }
    }

    /// Return the directory where file dialogs should open:
    /// Priority: 1. `last_folder` (if exists), 2. `example` folder, 3. current working directory.
    pub fn dialog_directory(&self) -> PathBuf {
        if let Some(ref folder) = self.last_folder {
            if folder.exists() && folder.is_dir() {
                return folder.clone();
            }
        }
        let example_dir = std::path::Path::new("example");
        if example_dir.exists() {
            return example_dir.to_path_buf();
        }
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    }

    /// Record the parent directory of a selected file as `last_folder` and persist it.
    pub fn set_last_folder_from_path(&mut self, file_path: &std::path::Path) {
        if let Some(parent) = file_path.parent() {
            if parent.exists() {
                self.last_folder = Some(parent.to_path_buf());
                self.save_settings();
            }
        }
    }

    /// Open file dialog for `.il`, `.json`, or `.ld` programs starting from `last_folder`.
    pub fn open_program_dialog(&mut self) {
        let dialog = rfd::FileDialog::new()
            .set_directory(self.dialog_directory())
            .add_filter("IEC 61131-3 IL & Projects (*.il, *.json, *.ld)", &["il", "json", "ld"]);
        if let Some(path) = dialog.pick_file() {
            self.set_last_folder_from_path(&path);
            if let Ok(content) = std::fs::read_to_string(&path) {
                if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    if let Ok(doc) = serde_json::from_str(&content) {
                        self.doc = doc;
                    }
                } else {
                    let _ = self.doc.load_from_il_str(&content);
                }
                self.doc.file_path = Some(path.display().to_string());
                self.doc.is_dirty = false;
            }
        }
    }

    /// Save current ladder program to `.il` file, defaulting to `last_folder`. Returns true if saved.
    pub fn save_program_dialog(&mut self) -> bool {
        let mut dialog = rfd::FileDialog::new()
            .set_directory(self.dialog_directory())
            .add_filter("IEC 61131-3 Instruction List (*.il)", &["il"]);
        if let Some(ref current_path) = self.doc.file_path {
            let path = std::path::Path::new(current_path);
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.ends_with(".il") {
                    dialog = dialog.set_file_name(name);
                } else if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    dialog = dialog.set_file_name(format!("{}.il", stem));
                }
            }
        }
        if let Some(path) = dialog.save_file() {
            self.set_last_folder_from_path(&path);
            let il_text = self.doc.export_to_il_str();
            if std::fs::write(&path, il_text).is_ok() {
                self.doc.file_path = Some(path.display().to_string());
                self.doc.is_dirty = false;
                return true;
            }
        }
        false
    }

    /// Save current project to `.json` file, defaulting to `last_folder`. Returns true if saved.
    pub fn save_as_json_dialog(&mut self) -> bool {
        let mut dialog = rfd::FileDialog::new()
            .set_directory(self.dialog_directory())
            .add_filter("uPLC JSON Project (*.json)", &["json"]);
        if let Some(ref current_path) = self.doc.file_path {
            let path = std::path::Path::new(current_path);
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                dialog = dialog.set_file_name(format!("{}.json", stem));
            }
        }
        if let Some(path) = dialog.save_file() {
            self.set_last_folder_from_path(&path);
            if let Ok(json) = serde_json::to_string_pretty(&self.doc) {
                if std::fs::write(&path, json).is_ok() {
                    self.doc.file_path = Some(path.display().to_string());
                    self.doc.is_dirty = false;
                    return true;
                }
            }
        }
        false
    }

    /// Quick-save directly to existing file_path if known; otherwise open Save dialog. Returns true if saved.
    pub fn save_or_save_as(&mut self) -> bool {
        if let Some(ref current_path) = self.doc.file_path {
            let path = std::path::Path::new(current_path);
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Ok(json) = serde_json::to_string_pretty(&self.doc) {
                    if std::fs::write(path, json).is_ok() {
                        self.doc.is_dirty = false;
                        return true;
                    }
                }
            } else {
                let il_text = self.doc.export_to_il_str();
                if std::fs::write(path, il_text).is_ok() {
                    self.doc.is_dirty = false;
                    return true;
                }
            }
        }
        self.save_program_dialog()
    }

    /// Import IEC 61131-3 IL code into active document, defaulting to `last_folder`.
    pub fn import_il_dialog(&mut self) {
        let dialog = rfd::FileDialog::new()
            .set_directory(self.dialog_directory())
            .add_filter("IEC 61131-3 Instruction List (*.il, *.ld)", &["il", "ld"]);
        if let Some(path) = dialog.pick_file() {
            self.set_last_folder_from_path(&path);
            if let Ok(content) = std::fs::read_to_string(&path) {
                let _ = self.doc.load_from_il_str(&content);
                self.doc.file_path = Some(path.display().to_string());
                self.doc.is_dirty = false;
            }
        }
    }

    /// Export IEC 61131-3 IL code, defaulting to `last_folder`.
    pub fn export_il_dialog(&mut self) {
        let mut dialog = rfd::FileDialog::new()
            .set_directory(self.dialog_directory())
            .add_filter("IEC 61131-3 Instruction List (*.il)", &["il"]);
        if let Some(ref current_path) = self.doc.file_path {
            let path = std::path::Path::new(current_path);
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                dialog = dialog.set_file_name(name);
            }
        }
        if let Some(path) = dialog.save_file() {
            self.set_last_folder_from_path(&path);
            let il_text = self.doc.export_to_il_str();
            let _ = std::fs::write(&path, il_text);
        }
    }

    /// Request creating a new program. Prompts if unsaved changes exist.
    pub fn request_new_program(&mut self) {
        if self.doc.is_dirty {
            self.pending_action = Some(PendingAction::NewProgram);
        } else {
            self.new_program();
        }
    }

    /// Request opening another program. Prompts if unsaved changes exist.
    pub fn request_open_program(&mut self) {
        if self.doc.is_dirty {
            self.pending_action = Some(PendingAction::OpenProgram);
        } else {
            self.open_program_dialog();
        }
    }

    /// Request loading an example program. Prompts if unsaved changes exist.
    pub fn request_load_example(&mut self, path_str: String) {
        if self.doc.is_dirty {
            self.pending_action = Some(PendingAction::LoadExample(path_str));
        } else {
            self.load_example_file(&path_str);
        }
    }

    /// Request exiting the application. Prompts if unsaved changes exist.
    pub fn request_exit(&mut self, ctx: &egui::Context) {
        if self.doc.is_dirty && !self.allow_exit {
            self.pending_action = Some(PendingAction::ExitApp);
        } else {
            self.allow_exit = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    /// Load example file directly into document.
    pub fn load_example_file(&mut self, path_str: &str) {
        let content = std::fs::read_to_string(path_str)
            .or_else(|_| {
                if let Ok(exe) = std::env::current_exe() {
                    if let Some(parent) = exe.parent() {
                        if let Ok(c) = std::fs::read_to_string(parent.join(path_str)) {
                            return Ok(c);
                        }
                        if let Ok(c) = std::fs::read_to_string(parent.join("../..").join(path_str)) {
                            return Ok(c);
                        }
                    }
                }
                Err(std::io::Error::new(std::io::ErrorKind::NotFound, "not found"))
            });
        if let Ok(c) = content {
            let _ = self.doc.load_from_il_str(&c);
            self.doc.file_path = Some(path_str.to_string());
            self.doc.is_dirty = false;
            self.set_last_folder_from_path(std::path::Path::new(path_str));
        }
    }

    /// Execute the pending action after user chose to save or discard unsaved changes.
    pub fn execute_pending_action(&mut self, ctx: &egui::Context) {
        if let Some(action) = self.pending_action.take() {
            match action {
                PendingAction::NewProgram => {
                    self.new_program();
                }
                PendingAction::OpenProgram => {
                    self.open_program_dialog();
                }
                PendingAction::LoadExample(path_str) => {
                    self.load_example_file(&path_str);
                }
                PendingAction::ExitApp => {
                    self.allow_exit = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }

    /// Render Unsaved Changes modal dialog with Save, Don't Save, and Cancel options.
    pub fn render_unsaved_changes_modal(&mut self, ctx: &egui::Context) {
        let palette = self.theme.palette();
        let file_name = self.doc.file_path.as_deref().map(|p| {
            std::path::Path::new(p)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(p)
        }).unwrap_or("Untitled Program (โปรแกรมยังไม่ได้บันทึก)");

        let action_desc = match self.pending_action.as_ref() {
            Some(PendingAction::NewProgram) => "creating a new program / ก่อนสร้างโปรแกรมใหม่",
            Some(PendingAction::OpenProgram) => "opening another program / ก่อนเปิดโปรแกรมใหม่",
            Some(PendingAction::LoadExample(_)) => "loading example / ก่อนโหลดตัวอย่าง",
            Some(PendingAction::ExitApp) => "closing application / ก่อนออกจากโปรแกรม",
            None => "",
        };

        let mut do_save = false;
        let mut do_discard = false;
        let mut do_cancel = false;

        egui::Window::new("⚠ Unsaved Changes / บันทึกการเปลี่ยนแปลง")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size([460.0, 160.0])
            .show(ctx, |ui| {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("⚠").size(28.0).color(palette.accent_warn));
                    ui.add_space(8.0);
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(format!("Do you want to save changes to \"{}\"?", file_name))
                                .strong()
                                .size(14.0)
                                .color(palette.text_primary),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(format!("มีข้อมูลที่มีการเปลี่ยนแปลง ต้องการบันทึกก่อน {} หรือไม่?", action_desc))
                                .size(11.5)
                                .color(palette.symbol_subtext),
                        );
                    });
                });

                ui.add_space(16.0);
                ui.separator();
                ui.add_space(10.0);

                ui.horizontal(|ui| {
                    // Right aligned buttons: Cancel, Don't Save, Save
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Cancel (ยกเลิก)").clicked()
                            || ui.input(|i| i.key_pressed(egui::Key::Escape))
                        {
                            do_cancel = true;
                        }

                        let discard_btn = egui::Button::new(
                            RichText::new("Don't Save (ไม่บันทึก)").color(palette.accent_danger)
                        );
                        if ui.add(discard_btn).clicked() {
                            do_discard = true;
                        }

                        let save_btn = egui::Button::new(
                            RichText::new("💾 Save (บันทึก)").color(Color32::WHITE)
                        ).fill(palette.accent_ok);
                        if ui.add(save_btn).clicked()
                            || ui.input(|i| i.key_pressed(egui::Key::Enter))
                        {
                            do_save = true;
                        }
                    });
                });
            });

        if do_save {
            if self.save_or_save_as() {
                self.execute_pending_action(ctx);
            }
        } else if do_discard {
            self.doc.is_dirty = false;
            self.execute_pending_action(ctx);
        } else if do_cancel {
            self.pending_action = None;
        }
    }

    pub fn start_online_monitoring(&mut self) {
        if self.online_mode {
            return;
        }
        self.online_mode = true;
        if !self.online_target_hw {
            self.doc.set_online_mode(true);
        } else {
            // For HW MCU, wait until a matching program snapshot is verified before activating live conduction
            self.doc.set_online_mode(false);
            self.doc.clear_conduction();
        }

        if self.online_target_hw {
            self.online_status_text = "Connecting to PLC...".to_string();
            let stop_flag = Arc::new(AtomicBool::new(false));
            self.online_worker_stop = stop_flag.clone();

            let raw_port = self.com_port.split_whitespace().next().unwrap_or("/dev/ttyACM1");
            let mut port_path = if raw_port.starts_with("tty") {
                format!("/dev/{}", raw_port)
            } else {
                raw_port.to_string()
            };

            // Auto-detect uPLC port if current does not exist or is a debugprobe
            let is_debugprobe = port_path
                .strip_prefix("/dev/")
                .and_then(|n| std::fs::read_to_string(format!("/sys/class/tty/{}/device/../product", n)).ok())
                .map(|p| p.contains("Debugprobe"))
                .unwrap_or(false);
            if (!std::path::Path::new(&port_path).exists() || is_debugprobe)
                && Self::detect_uplc_usb_port().is_some()
            {
                if let Some(detected) = Self::detect_uplc_usb_port() {
                    port_path = detected.clone();
                    self.com_port = detected;
                }
            }

            if !std::path::Path::new(&port_path).exists() {
                self.online_status_text = format!("⚠ Waiting for port '{}'...", port_path);
            }

            let baud = self.baud_rate.clone();
            let (tx, rx) = mpsc::channel::<(PlcStatusSnapshot, String)>();
            self.online_worker_rx = Some(rx);
            let (cmd_tx, cmd_rx) = mpsc::channel::<Vec<u8>>();
            self.online_worker_cmd_tx = Some(cmd_tx);

            thread::spawn(move || {
                use std::io::{Read, Write};
                // Pre-constructed MonitorStatus request packet: [0xAA, 0x03, 0x00, 0x00, 0xBD]
                let req = [0xAAu8, 0x03, 0x00, 0x00, 0xBD];
                let mut chunk = [0u8; 512];
                let mut rx_accum: Vec<u8> = Vec::with_capacity(1024);

                while !stop_flag.load(Ordering::Relaxed) {
                    // Check if current port_path exists; if not, attempt auto-detection (hotplug)
                    if !std::path::Path::new(&port_path).exists() {
                        if let Some(detected) = Self::detect_uplc_usb_port() {
                            port_path = detected;
                        } else {
                            thread::sleep(Duration::from_millis(250));
                            continue;
                        }
                    }

                    // Configure stty raw mode with timeout
                    let _ = std::process::Command::new("stty")
                        .args(["-F", &port_path, &baud, "raw", "-echo", "-hupcl", "min", "0", "time", "1"])
                        .output();

                    match std::fs::OpenOptions::new().read(true).write(true).open(&port_path) {
                        Ok(mut file) => {
                            // Drain any stale bytes on initial open
                            let _ = file.read(&mut chunk);
                            rx_accum.clear();
                            let mut consecutive_failures = 0u32;

                            while !stop_flag.load(Ordering::Relaxed) {
                                // 1. Check and send any pending interactive commands from UI (RUN / STOP / STEP)
                                while let Ok(cmd_packet) = cmd_rx.try_recv() {
                                    if file.write_all(&cmd_packet).is_ok() && file.flush().is_ok() {
                                        thread::sleep(Duration::from_millis(5));
                                        let mut ack = [0u8; 16];
                                        let _ = file.read(&mut ack);
                                    }
                                }

                                // 2. Poll Status Snapshot
                                if file.write_all(&req).is_err() || file.flush().is_err() {
                                    // Write failed: port likely disconnected
                                    break;
                                }

                                let start = Instant::now();
                                let mut got_packet = false;
                                while start.elapsed().as_millis() < 220 {
                                    if stop_flag.load(Ordering::Relaxed) {
                                        break;
                                    }
                                    match file.read(&mut chunk) {
                                        Ok(0) => thread::sleep(Duration::from_millis(4)),
                                        Ok(n) => {
                                            rx_accum.extend_from_slice(&chunk[..n]);
                                            // Search for [0xAA, 0x83, len_lo, len_hi] where payload len is 462 (0x01CE) or 458 (0x01CA)
                                            if let Some(pos) = rx_accum.windows(4).position(|w| {
                                                if w[0] == 0xAA && w[1] == 0x83 {
                                                    let plen = u16::from_le_bytes([w[2], w[3]]) as usize;
                                                    plen == PlcStatusSnapshot::PAYLOAD_LEN
                                                        || plen == PlcStatusSnapshot::LEGACY_PAYLOAD_LEN
                                                } else {
                                                    false
                                                }
                                            }) {
                                                if pos > 0 {
                                                    rx_accum.drain(..pos);
                                                }
                                                if rx_accum.len() >= 4 {
                                                    let plen = u16::from_le_bytes([rx_accum[2], rx_accum[3]]) as usize;
                                                    let frame_len = plen + 5;
                                                    if rx_accum.len() >= frame_len {
                                                        let frame: Vec<u8> =
                                                            rx_accum.drain(..frame_len).collect();
                                                        if let Ok(snap) =
                                                            PlcStatusSnapshot::parse_from_packet(
                                                                &frame,
                                                            )
                                                        {
                                                            got_packet = true;
                                                            consecutive_failures = 0;
                                                            if tx.send((snap, port_path.clone())).is_err() {
                                                                return; // UI receiver dropped
                                                            }
                                                        }
                                                        break;
                                                    }
                                                }
                                            } else if rx_accum.len() > 1024 {
                                                // Prevent buffer bloat on noise or desynchronization
                                                rx_accum.clear();
                                            }
                                        }
                                        Err(_) => {
                                            // Read error: device disconnected or serial error
                                            break;
                                        }
                                    }
                                }

                                if !got_packet {
                                    consecutive_failures += 1;
                                    if consecutive_failures >= 4 {
                                        // Repeated timeouts: drain OS serial buffer in case of framing jam
                                        let mut drain = [0u8; 512];
                                        let _ = file.read(&mut drain);
                                        rx_accum.clear();
                                        // If still failing after 8 attempts (~2s), force re-opening port
                                        if consecutive_failures >= 8 {
                                            break;
                                        }
                                    }
                                }

                                thread::sleep(Duration::from_millis(50));
                            }
                        }
                        Err(_) => {
                            // Port cannot be opened: wait and check for hotplug re-detection
                            thread::sleep(Duration::from_millis(250));
                            if let Some(detected) = Self::detect_uplc_usb_port() {
                                port_path = detected;
                            }
                        }
                    }
                }
            });
        } else {
            // Local Simulator mode
            if self.doc.simulator.state == SimState::Stopped {
                self.doc.simulator.state = SimState::Running;
            }
            self.online_status_text = "Online (Sim)".to_string();
        }
    }

    pub fn stop_online_monitoring(&mut self) {
        self.online_mode = false;
        self.online_worker_stop.store(true, Ordering::Relaxed);
        self.online_worker_rx = None;
        self.online_worker_cmd_tx = None;
        self.last_telemetry_rx = None;
        self.doc.set_online_mode(false);
        self.online_status_text = "Offline".to_string();
    }

    /// Returns true if Hardware Online Monitoring is currently receiving live packets within the heartbeat window (1000ms).
    pub fn is_hw_online_connected(&self) -> bool {
        self.online_mode
            && self.online_target_hw
            && self
                .last_telemetry_rx
                .map_or(false, |t| t.elapsed() < Duration::from_millis(1000))
    }

    /// Toggle RUN/STOP mode for either local simulator or hardware PLC.
    pub fn toggle_run_mode(&mut self) {
        if self.online_target_hw {
            if !self.online_mode {
                self.start_online_monitoring();
            }
            if self.plc_remote_running {
                self.stop_run_mode();
            } else {
                self.transmit_bytecode_to_plc(my_canvas_core::PlcSerialCommand::RunStopControl);
            }
        } else {
            self.doc.toggle_simulation();
        }
    }

    /// Execute a single scan step on either local simulator or hardware PLC.
    pub fn single_step_mode(&mut self) {
        if self.online_target_hw {
            if !self.online_mode {
                self.start_online_monitoring();
            }
            self.transmit_bytecode_to_plc(my_canvas_core::PlcSerialCommand::StepPlc);
        } else {
            self.doc.single_step_simulation(self.doc.simulator.cycle_time_ms);
        }
    }

    /// Stop execution and safely turn off outputs for either local simulator or hardware PLC.
    pub fn stop_run_mode(&mut self) {
        if self.online_target_hw {
            self.plc_remote_running = false;
            self.doc.simulator.remote_running = false;
            self.doc.clear_conduction();
            self.transmit_bytecode_to_plc(my_canvas_core::PlcSerialCommand::StopPlc);
        } else {
            self.doc.stop_simulation();
        }
    }

    /// Reset simulation/PLC execution.
    pub fn reset_run_mode(&mut self) {
        if self.online_target_hw {
            self.plc_remote_running = false;
            self.doc.simulator.remote_running = false;
            self.doc.clear_conduction();
            self.transmit_bytecode_to_plc(my_canvas_core::PlcSerialCommand::StopPlc);
        } else {
            self.doc.reset_simulation();
        }
    }

    /// Force a digital bit (Input X, Output Y, or Marker M) to ON (true) or OFF (false)
    pub fn force_bit(&mut self, addr: u8, val: bool) {
        self.doc.force_bit(addr, val);
        if self.online_target_hw {
            let packet = my_canvas_core::BytecodeProgram::create_force_packet(addr, if val { 1 } else { 2 });
            self.transmit_custom_packet_to_plc(my_canvas_core::PlcSerialCommand::ForceIo, packet);
        }
    }

    /// Release force override on a digital bit, returning control to physical inputs or ladder logic
    pub fn unforce_bit(&mut self, addr: u8) {
        self.doc.unforce_bit(addr);
        if self.online_target_hw {
            let packet = my_canvas_core::BytecodeProgram::create_force_packet(addr, 0);
            self.transmit_custom_packet_to_plc(my_canvas_core::PlcSerialCommand::ForceIo, packet);
        }
    }

    /// Release all active force overrides across all bits
    pub fn clear_all_forces(&mut self) {
        self.doc.clear_all_forces();
        if self.online_target_hw {
            let packet = my_canvas_core::BytecodeProgram::create_force_packet(0, 0xFF);
            self.transmit_custom_packet_to_plc(my_canvas_core::PlcSerialCommand::ForceIo, packet);
        }
    }

    /// Check if a bit address is forced
    pub fn is_bit_forced(&self, addr: u8) -> Option<bool> {
        self.doc.is_bit_forced(addr)
    }

    /// Direct write a 32-bit integer value into a Word Memory register (%MW / Data Register)
    pub fn write_word_register(&mut self, addr: u8, val: i32) {
        self.doc.write_word_register(addr, val);
        if self.online_target_hw {
            let packet = my_canvas_core::BytecodeProgram::create_write_value_packet(true, addr, val);
            self.transmit_custom_packet_to_plc(my_canvas_core::PlcSerialCommand::WriteValue, packet);
        }
    }

    pub fn toggle_online_mode(&mut self) {
        if self.online_mode {
            self.stop_online_monitoring();
        } else {
            self.start_online_monitoring();
        }
    }

    /// Returns `(matched, ide_len, ide_crc16, plc_len, plc_crc16)` when a live `PlcStatusSnapshot` is available.
    pub fn online_program_match_info(&self) -> Option<(bool, u16, u16, u16, u16)> {
        let snap = self.latest_snapshot.as_ref()?;
        let bc = self.doc.compile_to_bytecode();
        let (ide_len, ide_crc) = bc.program_signature();
        let matched = snap.matches_program(&bc);
        Some((matched, ide_len, ide_crc, snap.prog_len, snap.prog_crc))
    }

    pub fn set_online_target_hw(&mut self, is_hw: bool) {
        if self.online_target_hw == is_hw {
            return;
        }
        self.online_target_hw = is_hw;
        self.save_settings();
        self.latest_snapshot = None;
        self.last_telemetry_rx = None;
        self.doc.clear_conduction();

        if is_hw {
            // Switching to hardware target: pause / halt local simulator
            self.doc.stop_simulation();
        }

        if self.online_mode {
            self.stop_online_monitoring();
            self.start_online_monitoring();
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

        // Intercept window close request when unsaved changes exist
        if ui.input(|i| i.viewport().close_requested()) {
            if self.doc.is_dirty && !self.allow_exit {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.pending_action = Some(PendingAction::ExitApp);
            } else {
                self.allow_exit = true;
            }
        }

        // Dynamic Window Title with Dirty Marker (*)
        let file_title = if let Some(ref p) = self.doc.file_path {
            std::path::Path::new(p)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(p)
        } else {
            "Untitled"
        };
        let dirty_suffix = if self.doc.is_dirty { "*" } else { "" };
        let full_title = format!("uPLC Studio - {}{}", file_title, dirty_suffix);
        if self.current_window_title != full_title {
            self.current_window_title = full_title.clone();
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Title(full_title));
        }

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

        // Periodic simulation scan when Running (paused when Hardware Online Monitoring is active)
        if self.doc.simulator.state == SimState::Running && !(self.online_mode && self.online_target_hw) {
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

        // Online Monitoring Telemetry Polling (RP2040 Hardware or Virtual Simulator)
        if self.online_mode {
            if self.online_target_hw {
                let mut latest = None;
                if let Some(ref rx) = self.online_worker_rx {
                    while let Ok((snap, port)) = rx.try_recv() {
                        latest = Some((snap, port));
                    }
                }
                if let Some((snap, port)) = latest {
                    self.plc_remote_running = snap.running;
                    self.latest_snapshot = Some(snap);
                    self.last_telemetry_rx = Some(Instant::now());
                    if !port.is_empty() && self.com_port != port {
                        self.com_port = port;
                    }
                }

                let is_connected = self.is_hw_online_connected();

                if is_connected {
                    // Verify code match against latest snapshot
                    let bc = self.doc.compile_to_bytecode();
                    let is_matched = self
                        .latest_snapshot
                        .as_ref()
                        .map_or(false, |snap| snap.matches_program(&bc));

                    if is_matched {
                        if let Some(ref snap) = self.latest_snapshot {
                            self.doc.set_online_mode(true);
                            self.doc.apply_online_snapshot(snap);
                        }
                    } else {
                        // Safety & Accuracy: Do NOT show simulation/live conduction on canvas if code is mismatched!
                        self.doc.set_online_mode(false);
                        self.doc.clear_conduction();
                    }

                    if let Some((matched, _ide_len, ide_crc, _plc_len, plc_crc)) =
                        self.online_program_match_info()
                    {
                        let snap = self.latest_snapshot.as_ref().unwrap();
                        let match_tag = if matched {
                            format!("✔ MATCH (0x{:04X})", plc_crc)
                        } else {
                            format!("⚠ MISMATCH (IDE:0x{:04X}≠PLC:0x{:04X})", ide_crc, plc_crc)
                        };
                        self.online_status_text = format!(
                            "Online (HW) | Scan #{} | {} | {}",
                            snap.scan_count,
                            if snap.running { "RUN" } else { "STOP" },
                            match_tag
                        );
                    }
                } else {
                    // Disconnected or waiting for hotplug reconnection: clear conduction for safety
                    self.doc.set_online_mode(false);
                    self.doc.clear_conduction();
                    self.online_status_text = format!(
                        "⚠ Waiting for RP2040 ({}) - Auto-reconnecting...",
                        self.com_port
                    );
                }
            } else {
                let snap = self.doc.capture_status_snapshot();
                self.online_status_text = format!(
                    "Online (Sim) | Scan #{} | {} | ✔ MATCH (0x{:04X})",
                    snap.scan_count,
                    if snap.running { "RUN" } else { "STOP" },
                    snap.prog_crc
                );
                self.latest_snapshot = Some(snap);
            }
            ui.ctx().request_repaint();
        }

        let any_modal_open = self.elem_modal.is_open
            || self.show_about
            || self.show_io_config
            || self.show_code_export
            || self.show_update_plc
            || self.pending_action.is_some();

        // Global Keyboard Shortcuts (Ctrl+N / Ctrl+Z / Ctrl+Y work anytime; Arrow/Enter/Del/F5 when not typing & modal closed)
        if !any_modal_open {
            let new_shortcut = ui.input(|i| {
                i.modifiers.command && !i.modifiers.shift && i.key_pressed(egui::Key::N)
            });
            let open_shortcut = ui.input(|i| {
                i.modifiers.command && !i.modifiers.shift && i.key_pressed(egui::Key::O)
            });
            let save_shortcut = ui.input(|i| {
                i.modifiers.command && !i.modifiers.shift && i.key_pressed(egui::Key::S)
            });
            let undo_shortcut = ui.input(|i| {
                i.modifiers.command && !i.modifiers.shift && i.key_pressed(egui::Key::Z)
            });
            let redo_shortcut = ui.input(|i| {
                (i.modifiers.command && i.key_pressed(egui::Key::Y))
                    || (i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::Z))
            });
            if new_shortcut {
                self.request_new_program();
            } else if open_shortcut {
                self.request_open_program();
            } else if save_shortcut {
                self.save_or_save_as();
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
                        self.reset_run_mode();
                    } else {
                        self.toggle_run_mode();
                    }
                }
                if ui.input(|i| i.key_pressed(egui::Key::F6)) {
                    self.single_step_mode();
                }
                if ui.input(|i| i.key_pressed(egui::Key::F7) || (i.modifiers.command && i.key_pressed(egui::Key::M))) {
                    self.toggle_online_mode();
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
            .default_size(330.0)
            .min_size(260.0)
            .max_size(650.0)
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
                        if self.online_mode || self.doc.simulator.is_active() {
                            let acc_b = self
                                .latest_snapshot
                                .as_ref()
                                .map_or(self.doc.simulator.acc_bool, |s| s.acc_bool);
                            let acc_w = self
                                .latest_snapshot
                                .as_ref()
                                .map_or(self.doc.simulator.acc_word, |s| s.acc_word);
                            ui.label(
                                RichText::new(format!(
                                    "⚡ LIVE ACC: Bit={} | Word={}",
                                    if acc_b { "1" } else { "0" },
                                    acc_w
                                ))
                                .monospace()
                                .size(11.5)
                                .color(if acc_b { palette.accent_ok } else { palette.accent_heading }),
                            );
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("[x] Hide").clicked() {
                                self.show_il_code = false;
                                self.save_settings();
                            }
                        });
                    });
                    let il_code = if self.online_mode || self.doc.simulator.is_active() {
                        self.doc.generate_il_code_online()
                    } else {
                        self.doc.generate_il_code()
                    };
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

        let mut banner_cmd = None;
        let mut open_upload_dialog = false;

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

                // Floating Live Online Monitoring Telemetry HUD
                if self.online_mode {
                    let max_rect = ui.max_rect();
                    let hud_w = 304.0;
                    let hud_h = 74.0;
                    let hud_pos = egui::pos2(max_rect.max.x - hud_w - 18.0, max_rect.min.y + 14.0);
                    let hud_rect = egui::Rect::from_min_size(hud_pos, egui::vec2(hud_w, hud_h));

                    let match_info = self.online_program_match_info();
                    let is_matched = match_info.map_or(true, |(m, _, _, _, _)| m);

                    let painter = ui.painter();
                    let is_hw = self.online_target_hw;
                    let is_hw_conn = !is_hw || self.is_hw_online_connected();
                    let glow_color = if !is_hw_conn {
                        palette.accent_warn
                    } else if !is_matched {
                        palette.accent_warn
                    } else if is_hw {
                        palette.accent_ok
                    } else {
                        palette.accent_heading
                    };
                    painter.rect_filled(hud_rect, egui::CornerRadius::same(8), palette.banner_bg.gamma_multiply(0.95));
                    painter.rect_stroke(
                        hud_rect,
                        egui::CornerRadius::same(8),
                        egui::Stroke::new(1.5, glow_color),
                        egui::StrokeKind::Inside,
                    );

                    let snap = self.latest_snapshot.as_ref();
                    let scan_txt = if is_hw && !is_hw_conn {
                        0
                    } else {
                        snap.map_or(0, |s| s.scan_count)
                    };
                    let is_run = is_hw_conn && snap.map_or(false, |s| s.running);
                    let acc_b = is_hw_conn && snap.map_or(false, |s| s.acc_bool);
                    let acc_w = if is_hw_conn { snap.map_or(0, |s| s.acc_word) } else { 0 };

                    let target_str = if is_hw { "RP2040 MCU" } else { "Simulator" };
                    let (title_str, run_str, run_color) = if is_hw && !is_hw_conn {
                        (
                            format!("⚠ RECONNECTING: {}", target_str),
                            "WAITING USB",
                            palette.accent_warn,
                        )
                    } else {
                        (
                            format!("🟢 ONLINE: {}", target_str),
                            if is_run { "● RUN" } else { "⏹ STOP" },
                            if is_run { palette.accent_ok } else { palette.accent_danger },
                        )
                    };

                    painter.text(
                        egui::pos2(hud_rect.min.x + 10.0, hud_rect.min.y + 15.0),
                        egui::Align2::LEFT_CENTER,
                        title_str,
                        egui::FontId::proportional(12.5),
                        palette.text_primary,
                    );
                    painter.text(
                        egui::pos2(hud_rect.max.x - 12.0, hud_rect.min.y + 15.0),
                        egui::Align2::RIGHT_CENTER,
                        run_str,
                        egui::FontId::monospace(11.5),
                        run_color,
                    );

                    painter.text(
                        egui::pos2(hud_rect.min.x + 10.0, hud_rect.min.y + 36.0),
                        egui::Align2::LEFT_CENTER,
                        format!("Scan: #{}", scan_txt),
                        egui::FontId::monospace(11.0),
                        palette.rung_text_dim,
                    );

                    painter.text(
                        egui::pos2(hud_rect.max.x - 12.0, hud_rect.min.y + 36.0),
                        egui::Align2::RIGHT_CENTER,
                        format!("ACC: Bit={} | Word={}", if acc_b { "1" } else { "0" }, acc_w),
                        egui::FontId::monospace(11.0),
                        if acc_b { palette.accent_ok } else { palette.accent_heading },
                    );

                    let (sig_text, sig_color) = match match_info {
                        Some((true, ide_len, ide_crc, _, _)) => (
                            format!("✔ CODE MATCH: {} Inst (CRC16 0x{:04X})", ide_len / 4, ide_crc),
                            palette.accent_ok,
                        ),
                        Some((false, _ide_len, ide_crc, _plc_len, plc_crc)) => (
                            format!("⚠ MISMATCH! IDE:0x{:04X} ≠ PLC:0x{:04X}", ide_crc, plc_crc),
                            palette.accent_warn,
                        ),
                        None => (
                            "⏳ Verifying PLC Bytecode CRC16...".to_string(),
                            palette.rung_text_dim,
                        ),
                    };
                    painter.text(
                        egui::pos2(hud_rect.min.x + 10.0, hud_rect.min.y + 57.0),
                        egui::Align2::LEFT_CENTER,
                        sig_text,
                        egui::FontId::monospace(10.5),
                        sig_color,
                    );

                    // If HW MCU code mismatch, display prominent warning banner explaining live monitoring is paused
                    if is_hw && !is_matched {
                        let banner_pos = egui::pos2(max_rect.center().x - 260.0, max_rect.min.y + 14.0);

                        egui::Area::new(egui::Id::new("online_mismatch_banner"))
                            .order(egui::Order::Foreground)
                            .fixed_pos(banner_pos)
                            .interactable(true)
                            .show(ui.ctx(), |ui| {
                                egui::Frame::NONE
                                    .fill(egui::Color32::from_rgba_unmultiplied(38, 28, 8, 245))
                                    .stroke(egui::Stroke::new(1.2, palette.accent_warn))
                                    .corner_radius(egui::CornerRadius::same(6))
                                    .inner_margin(egui::Margin::symmetric(12, 6))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                RichText::new("⚠ LIVE MONITORING PAUSED (CODE MISMATCH)")
                                                    .color(palette.accent_warn)
                                                    .strong()
                                                    .size(11.5),
                                            );
                                            ui.add_space(8.0);
                                            if ui
                                                .button(
                                                    RichText::new("📥 Read PLC")
                                                        .size(11.0)
                                                        .strong()
                                                        .color(egui::Color32::WHITE),
                                                )
                                                .on_hover_text("Read bytecode from connected PLC and decompile into Ladder Diagram")
                                                .clicked()
                                            {
                                                banner_cmd = Some(my_canvas_core::PlcSerialCommand::UploadProgram);
                                            }
                                            if ui
                                                .button(
                                                    RichText::new("⬆ Upload to PLC")
                                                        .size(11.0)
                                                        .strong()
                                                        .color(egui::Color32::WHITE),
                                                )
                                                .on_hover_text("Open Update to PLC dialog to download current program")
                                                .clicked()
                                            {
                                                open_upload_dialog = true;
                                            }
                                        });
                                    });
                            });
                    }
                }
            });

        if let Some(cmd) = banner_cmd {
            self.transmit_bytecode_to_plc(cmd);
        }
        if open_upload_dialog {
            self.show_update_plc = true;
        }

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
                            let mut dialog = rfd::FileDialog::new()
                                .set_directory(self.dialog_directory())
                                .add_filter("IEC 61131-3 Instruction List (*.il)", &["il"]);
                            if let Some(ref current_path) = self.doc.file_path {
                                let path = std::path::Path::new(current_path);
                                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                                    dialog = dialog.set_file_name(format!("{}.il", stem));
                                }
                            }
                            if let Some(path) = dialog.save_file() {
                                self.set_last_folder_from_path(&path);
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
            let port_exists = std::path::Path::new(self.com_port.trim()).exists();

            Window::new("⚡ Update to PLC — Micro-VM Serial Loader & Controller")
                .order(egui::Order::Foreground)
                .open(&mut open_update_plc)
                .min_size([740.0, 540.0])
                .default_size([780.0, 580.0])
                .collapsible(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(ui.ctx(), |ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);

                    // 1. Modern Connection & Live PLC Runtime Control Card
                    egui::Frame::group(ui.style())
                        .fill(palette.card_bg)
                        .stroke(egui::Stroke::new(1.0, palette.card_border))
                        .corner_radius(egui::CornerRadius::same(8))
                        .inner_margin(egui::Margin::symmetric(12, 10))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                // Left: USB Serial Port Configuration
                                let status_dot_color = if port_exists {
                                    egui::Color32::from_rgb(46, 204, 113)
                                } else {
                                    egui::Color32::from_rgb(231, 76, 60)
                                };
                                ui.label(
                                    RichText::new(if port_exists { "● Connected" } else { "○ Offline" })
                                        .color(status_dot_color)
                                        .strong()
                                        .size(11.5),
                                );
                                ui.separator();

                                ui.label(
                                    RichText::new("USB Serial:")
                                        .color(palette.accent_heading)
                                        .strong(),
                                );
                                if ui
                                    .add(
                                        egui::TextEdit::singleline(&mut self.com_port)
                                            .desired_width(125.0)
                                            .font(egui::TextStyle::Monospace),
                                    )
                                    .changed()
                                {
                                    settings_changed = true;
                                }
                                egui::ComboBox::from_id_salt("update_plc_port_combo")
                                    .width(105.0)
                                    .selected_text("🔍 Detect")
                                    .show_ui(ui, |ui| {
                                        if let Some(auto_port) = Self::detect_uplc_usb_port() {
                                            if ui
                                                .selectable_value(
                                                    &mut self.com_port,
                                                    auto_port.clone(),
                                                    format!("★ {} (uPLC VM)", auto_port),
                                                )
                                                .changed()
                                            {
                                                settings_changed = true;
                                            }
                                            ui.separator();
                                        }
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
                                    .width(76.0)
                                    .selected_text(&self.baud_rate)
                                    .show_ui(ui, |ui| {
                                        for b in ["9600", "19200", "38400", "57600", "115200"] {
                                            if ui
                                                .selectable_value(
                                                    &mut self.baud_rate,
                                                    b.to_string(),
                                                    b,
                                                )
                                                .changed()
                                            {
                                                settings_changed = true;
                                            }
                                        }
                                    });

                                // Right: Hardware Target Tag & Live PLC Runtime Status Badge
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        // PLC State Pill Indicator
                                        let (badge_text, badge_bg, badge_fg) =
                                            if self.plc_remote_running {
                                                (
                                                    "● PLC RUNNING",
                                                    egui::Color32::from_rgba_unmultiplied(
                                                        39, 174, 96, 45,
                                                    ),
                                                    egui::Color32::from_rgb(46, 204, 113),
                                                )
                                            } else {
                                                (
                                                    "■ PLC STOPPED",
                                                    egui::Color32::from_rgba_unmultiplied(
                                                        231, 76, 60, 45,
                                                    ),
                                                    egui::Color32::from_rgb(241, 96, 80),
                                                )
                                            };
                                        egui::Frame::NONE
                                            .fill(badge_bg)
                                            .corner_radius(egui::CornerRadius::same(5))
                                            .inner_margin(egui::Margin::symmetric(10, 5))
                                            .show(ui, |ui| {
                                                ui.label(
                                                    RichText::new(badge_text)
                                                        .color(badge_fg)
                                                        .monospace()
                                                        .strong()
                                                        .size(11.5),
                                                );
                                            });

                                        // Target Hardware Tag
                                        egui::Frame::NONE
                                            .fill(palette.canvas_bg)
                                            .stroke(egui::Stroke::new(1.0, palette.card_border))
                                            .corner_radius(egui::CornerRadius::same(5))
                                            .inner_margin(egui::Margin::symmetric(8, 4))
                                            .show(ui, |ui| {
                                                ui.label(
                                                    RichText::new("🎯 RP2040 Micro-VM")
                                                        .size(11.0)
                                                        .color(palette.symbol_subtext),
                                                );
                                            });
                                    },
                                );
                            });
                        });

                    // 2. Telemetry & Bytecode Metrics Strip (4 Modern Cards)
                    let (_, prog_crc16) = bc.program_signature();
                    ui.columns(4, |cols| {
                        let metrics = [
                            (
                                "PROGRAM SIZE",
                                format!(
                                    "{} Inst ({} B)",
                                    bc.instructions.len(),
                                    bc.payload_size_bytes()
                                ),
                                palette.accent_ok,
                            ),
                            (
                                "SERIAL FRAME",
                                format!("{} Bytes (0xAA)", packet.len()),
                                palette.accent_heading,
                            ),
                            (
                                "FINGERPRINT (CRC16 / CRC8)",
                                format!("0x{:04X} | Frame 0x{:02X}", prog_crc16, crc8),
                                egui::Color32::from_rgb(52, 152, 219),
                            ),
                            (
                                "MAPPED I/O & MEM",
                                format!("{} Symbols", bc.memory_map.len()),
                                palette.symbol_subtext,
                            ),
                        ];
                        for (i, (title, val, color)) in metrics.iter().enumerate() {
                            egui::Frame::group(cols[i].style())
                                .fill(palette.card_bg)
                                .stroke(egui::Stroke::new(1.0, palette.card_border))
                                .corner_radius(egui::CornerRadius::same(6))
                                .inner_margin(egui::Margin::symmetric(10, 6))
                                .show(&mut cols[i], |ui| {
                                    ui.label(
                                        RichText::new(*title)
                                            .size(9.5)
                                            .color(palette.symbol_subtext)
                                            .strong(),
                                    );
                                    ui.label(
                                        RichText::new(val)
                                            .monospace()
                                            .strong()
                                            .size(12.5)
                                            .color(*color),
                                    );
                                });
                        }
                    });

                    // 3. Modern Segmented View Mode Tabs
                    ui.horizontal(|ui| {
                        let tab0_btn = egui::Button::new(
                            RichText::new("📦 1. 32-Bit Bytecode & Packet Frame")
                                .strong()
                                .size(12.0),
                        )
                        .selected(self.update_plc_tab == 0)
                        .corner_radius(egui::CornerRadius::same(6));
                        if ui.add(tab0_btn).clicked() {
                            self.update_plc_tab = 0;
                        }

                        let tab1_btn = egui::Button::new(
                            RichText::new("🗺 2. Unified Memory Mapping (0x00..0xFF)")
                                .strong()
                                .size(12.0),
                        )
                        .selected(self.update_plc_tab == 1)
                        .corner_radius(egui::CornerRadius::same(6));
                        if ui.add(tab1_btn).clicked() {
                            self.update_plc_tab = 1;
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .small_button("📋 Copy Packet Hex")
                                .on_hover_text("Copy raw Serial frame hex string to clipboard")
                                .clicked()
                            {
                                ui.ctx().copy_text(bc.to_packet_hex_string(
                                    my_canvas_core::PlcSerialCommand::DownloadProgram,
                                ));
                                self.update_plc_is_error = false;
                                self.update_plc_status = format!(
                                    "✔ Copied {} Bytes Serial Packet Hex to clipboard",
                                    packet.len()
                                );
                            }
                            if ui
                                .small_button("💾 Save .bin...")
                                .on_hover_text("Export raw 32-bit bytecode binary file (.bin)")
                                .clicked()
                            {
                                let mut dialog = rfd::FileDialog::new()
                                    .set_directory(self.dialog_directory())
                                    .add_filter("Micro-VM Bytecode Binary (*.bin)", &["bin"]);
                                if let Some(ref current_path) = self.doc.file_path {
                                    let path = std::path::Path::new(current_path);
                                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                                        dialog = dialog.set_file_name(format!("{}.bin", stem));
                                    }
                                }
                                if let Some(path) = dialog.save_file() {
                                    self.set_last_folder_from_path(&path);
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
                        });
                    });

                    // 4. Tab Content Container Card
                    egui::Frame::group(ui.style())
                        .fill(palette.card_bg)
                        .stroke(egui::Stroke::new(1.0, palette.card_border))
                        .corner_radius(egui::CornerRadius::same(8))
                        .inner_margin(egui::Margin::symmetric(10, 8))
                        .show(ui, |ui| match self.update_plc_tab {
                            0 => {
                                let disasm = bc.to_disassembly_text();
                                egui::ScrollArea::both()
                                    .id_salt("bytecode_disasm_scroll")
                                    .max_height(275.0)
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
                                    .max_height(250.0)
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
                                                    ui.monospace(format!(
                                                        "0x{:02X} ({:3})",
                                                        entry.addr, entry.addr
                                                    ));
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
                        });

                    // 5. Styled Status & ACK Telemetry Banner
                    let status_text = if self.update_plc_status.is_empty() {
                        format!(
                            "Ready — Target Port: {} @ {} bps | Protocol: SOF 0xAA + CMD + LEN + PAYLOAD + CRC8",
                            self.com_port, self.baud_rate
                        )
                    } else {
                        self.update_plc_status.clone()
                    };
                    let (banner_bg, banner_stroke, banner_fg) = if self.update_plc_status.is_empty()
                    {
                        (
                            palette.card_bg,
                            palette.card_border,
                            palette.symbol_subtext,
                        )
                    } else if self.update_plc_is_error {
                        (
                            egui::Color32::from_rgba_unmultiplied(231, 76, 60, 28),
                            egui::Color32::from_rgb(231, 76, 60),
                            egui::Color32::from_rgb(255, 118, 105),
                        )
                    } else {
                        (
                            egui::Color32::from_rgba_unmultiplied(39, 174, 96, 28),
                            egui::Color32::from_rgb(39, 174, 96),
                            egui::Color32::from_rgb(46, 204, 113),
                        )
                    };

                    egui::Frame::NONE
                        .fill(banner_bg)
                        .stroke(egui::Stroke::new(1.0, banner_stroke))
                        .corner_radius(egui::CornerRadius::same(6))
                        .inner_margin(egui::Margin::symmetric(10, 7))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.label(
                                RichText::new(status_text)
                                    .monospace()
                                    .size(11.5)
                                    .color(banner_fg),
                            );
                        });

                    ui.add_space(3.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("💾 Flash Retention:")
                                .strong()
                                .size(11.0)
                                .color(palette.accent_heading),
                        );
                        ui.label(
                            RichText::new("Non-Volatile Flash Persistence Active (Sector 0x1FE000 with CRC16 Boot Integrity)")
                                .size(11.0)
                                .color(palette.accent_ok),
                        );
                    });
                    ui.add_space(2.0);

                    // 6. Primary Bottom Action Bar (Unified PLC Controls)
                    ui.horizontal(|ui| {
                        let upload_btn = egui::Button::new(
                            RichText::new("⚡ Upload to PLC")
                                .strong()
                                .size(12.5)
                                .color(egui::Color32::WHITE),
                        )
                        .fill(egui::Color32::from_rgb(24, 130, 90))
                        .corner_radius(egui::CornerRadius::same(6))
                        .min_size(egui::vec2(160.0, 32.0));

                        if ui
                            .add(upload_btn)
                            .on_hover_text(
                                "Transmit 32-Bit Bytecode Packet Frame (CMD 0x01) via USB Serial and auto-start PLC execution",
                            )
                            .clicked()
                        {
                            trigger_cmd = Some(my_canvas_core::PlcSerialCommand::DownloadProgram);
                        }

                        let read_plc_btn = egui::Button::new(
                            RichText::new("📥 Read from PLC (Decompile)")
                                .strong()
                                .size(12.5)
                                .color(egui::Color32::WHITE),
                        )
                        .fill(egui::Color32::from_rgb(41, 128, 185))
                        .corner_radius(egui::CornerRadius::same(6))
                        .min_size(egui::vec2(195.0, 32.0));

                        if ui
                            .add(read_plc_btn)
                            .on_hover_text(
                                "Request Bytecode from connected PLC (CMD 0x04) and decompile it back into Ladder Diagram networks on Canvas",
                            )
                            .clicked()
                        {
                            trigger_cmd = Some(my_canvas_core::PlcSerialCommand::UploadProgram);
                        }

                        ui.separator();

                        let run_action_btn = egui::Button::new(
                            RichText::new("▶ RUN PLC")
                                .strong()
                                .size(12.5)
                                .color(egui::Color32::WHITE),
                        )
                        .fill(if self.plc_remote_running {
                            egui::Color32::from_rgb(39, 174, 96)
                        } else {
                            egui::Color32::from_rgb(45, 95, 65)
                        })
                        .corner_radius(egui::CornerRadius::same(6))
                        .min_size(egui::vec2(96.0, 32.0));

                        if ui
                            .add(run_action_btn)
                            .on_hover_text("Start 1ms deterministic PLC scan execution on MCU (CMD 0x02, Payload 0x01)")
                            .clicked()
                        {
                            trigger_cmd = Some(my_canvas_core::PlcSerialCommand::RunStopControl);
                        }

                        let stop_action_btn = egui::Button::new(
                            RichText::new("⏹ STOP PLC")
                                .strong()
                                .size(12.5)
                                .color(egui::Color32::WHITE),
                        )
                        .fill(if !self.plc_remote_running {
                            egui::Color32::from_rgb(192, 57, 43)
                        } else {
                            egui::Color32::from_rgb(120, 50, 48)
                        })
                        .corner_radius(egui::CornerRadius::same(6))
                        .min_size(egui::vec2(96.0, 32.0));

                        if ui
                            .add(stop_action_btn)
                            .on_hover_text(
                                "Halt PLC scan cycle and turn off all physical outputs (CMD 0x02, Payload 0x00)",
                            )
                            .clicked()
                        {
                            self.plc_remote_running = false;
                            self.doc.simulator.remote_running = false;
                            self.doc.clear_conduction();
                            trigger_cmd = Some(my_canvas_core::PlcSerialCommand::StopPlc);
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let close_btn = egui::Button::new(
                                RichText::new("Close").size(12.5),
                            )
                            .corner_radius(egui::CornerRadius::same(6))
                            .min_size(egui::vec2(80.0, 32.0));
                            if ui.add(close_btn).clicked() {
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

        if self.pending_action.is_some() {
            self.render_unsaved_changes_modal(ui.ctx());
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
                    online_target_hw: true,
                    window_width: Some(1600.0),
                    window_height: Some(900.0),
                    window_maximized: true,
                    window_pos_x: Some(100.0),
                    window_pos_y: Some(50.0),
                    last_folder: Some(PathBuf::from("/tmp/projects")),
                    watch_rows: default_watch_rows(),
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
                assert_eq!(loaded.last_folder, Some(PathBuf::from("/tmp/projects")));
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
        assert_eq!(loaded.last_folder, None);
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

    #[test]
    fn test_unsaved_changes_prompt_and_pending_action() {
        let mut app = CanvasApp::default();
        let ctx = egui::Context::default();

        // 1. Not dirty: request_new_program executes immediately without pending action
        app.doc.is_dirty = false;
        app.request_new_program();
        assert!(app.pending_action.is_none());

        // 2. Dirty: request_new_program sets PendingAction::NewProgram
        app.doc.is_dirty = true;
        app.request_new_program();
        assert_eq!(app.pending_action, Some(PendingAction::NewProgram));

        // 3. User chooses Don't Save / Discard: execute_pending_action resets state
        app.execute_pending_action(&ctx);
        assert!(app.pending_action.is_none());
        assert!(!app.doc.is_dirty);

        // 4. Dirty: request_exit sets PendingAction::ExitApp
        app.doc.is_dirty = true;
        app.request_exit(&ctx);
        assert_eq!(app.pending_action, Some(PendingAction::ExitApp));
        assert!(!app.allow_exit);

        // Discard / proceed exit
        app.execute_pending_action(&ctx);
        assert!(app.pending_action.is_none());
        assert!(app.allow_exit);
    }

    #[test]
    fn test_online_target_switching_logic() {
        let mut app = CanvasApp::default();

        // 1. Initial default state
        app.set_online_target_hw(false);
        assert!(!app.online_target_hw);

        // 2. Start simulation in simulator mode
        app.doc.simulator.state = SimState::Running;
        assert_eq!(app.doc.simulator.state, SimState::Running);

        // 3. Switch to Hardware target: should stop simulator and clear snapshots
        app.set_online_target_hw(true);
        assert!(app.online_target_hw);
        assert_eq!(app.doc.simulator.state, SimState::Stopped);
        assert!(app.latest_snapshot.is_none());

        // 4. Redundant call should be a no-op
        app.set_online_target_hw(true);
        assert!(app.online_target_hw);

        // 5. Online mode transitions
        app.set_online_target_hw(false);
        app.start_online_monitoring();
        assert!(app.online_mode);
        assert_eq!(app.online_status_text, "Online (Sim)");
        assert_eq!(app.doc.simulator.state, SimState::Running);

        // Switch target to HW while Online
        app.set_online_target_hw(true);
        assert!(app.online_mode);
        assert!(app.online_target_hw);
        assert_eq!(app.doc.simulator.state, SimState::Stopped);

        // Switch back to Simulator while Online
        app.set_online_target_hw(false);
        assert!(app.online_mode);
        assert!(!app.online_target_hw);
        assert_eq!(app.online_status_text, "Online (Sim)");
        assert_eq!(app.doc.simulator.state, SimState::Running);

        // Clean shutdown
        app.stop_online_monitoring();
        assert!(!app.online_mode);
        assert_eq!(app.online_status_text, "Offline");
    }

    #[test]
    fn test_online_hw_connection_watchdog_and_heartbeat() {
        let mut app = CanvasApp::default();
        app.online_mode = true;
        app.online_target_hw = true;

        // 1. Initial state without received telemetry: not connected
        assert!(!app.is_hw_online_connected());

        // 2. Recent telemetry received: connected
        app.last_telemetry_rx = Some(Instant::now());
        assert!(app.is_hw_online_connected());

        // 3. Telemetry expired (> 1000ms): watchdog flags disconnected
        app.last_telemetry_rx = Some(Instant::now() - Duration::from_millis(1500));
        assert!(!app.is_hw_online_connected());

        // 4. Fresh packet arrives: automatically reconnects
        app.last_telemetry_rx = Some(Instant::now());
        assert!(app.is_hw_online_connected());

        // 5. Stopping online monitoring clears heartbeat
        app.stop_online_monitoring();
        assert!(app.last_telemetry_rx.is_none());
        assert!(!app.is_hw_online_connected());
    }

    #[test]
    fn test_special_registers_tab_state() {
        let mut app = CanvasApp::default();
        assert_eq!(app.right_panel_tab, 0);

        // Switch to Special Register tab (Tab 3)
        app.right_panel_tab = 3;
        assert_eq!(app.right_panel_tab, 3);
        assert_eq!(app.special_reg_filter_cat, 0);
        assert!(app.special_reg_filter_text.is_empty());

        // Test filtering
        app.special_reg_filter_cat = 2; // Clocks
        app.special_reg_filter_text = "1s".to_string();
        assert_eq!(app.special_reg_filter_cat, 2);
        assert_eq!(app.special_reg_filter_text, "1s");
    }

    #[test]
    fn test_watch_table_evaluation_and_write() {
        let mut app = CanvasApp::default();
        app.watch_rows = default_watch_rows();
        // Default rows exist
        assert!(!app.watch_rows.is_empty());
        assert!(app.watch_rows.iter().any(|r| r.address == "%IX0.0"));
        assert!(app.watch_rows.iter().any(|r| r.address == "%IW0"));

        // Add a custom watched register
        app.add_watch_row("%MW10", "TANK_LEVEL", WatchDisplayFormat::Dec);
        let idx = app.watch_rows.iter().position(|r| r.address == "%MW10").unwrap();

        // Write value via Watch Table
        app.write_watch_row(idx, "3500").unwrap();
        let (val_str, is_bit, _) = app.evaluate_watch_value("%MW10", WatchDisplayFormat::Dec);
        assert!(!is_bit);
        assert_eq!(val_str, "3500");

        // Test Hex and Bin formats
        let (hex_str, _, _) = app.evaluate_watch_value("%MW10", WatchDisplayFormat::Hex);
        assert_eq!(hex_str, "0x0DAC");

        // Write bit via Watch Table
        let ix_idx = app.watch_rows.iter().position(|r| r.address == "%IX0.0").unwrap();
        app.write_watch_row(ix_idx, "1").unwrap();
        let (ix_str, is_bit_ix, _) = app.evaluate_watch_value("%IX0.0", WatchDisplayFormat::Bool);
        assert!(is_bit_ix);
        assert_eq!(ix_str, "● ON (1)");

        // Test Analog AI0 (%IW0)
        let iw_idx = app.watch_rows.iter().position(|r| r.address == "%IW0").unwrap();
        app.write_watch_row(iw_idx, "2048").unwrap();
        let (iw_str, _, _) = app.evaluate_watch_value("%IW0", WatchDisplayFormat::Auto);
        assert!(iw_str.contains("2048"));
        assert!(iw_str.contains("1.65V"));
    }
}



