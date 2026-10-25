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
use crate::panels::{element_dialog::ElementConfigModal, find_dialog, left_panel, menu_bar, right_panel, status_bar, toolbar};
use crate::panels::find_dialog::FindState;
use crate::serial;
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
    /// Most-recently opened/saved program files (newest first, max [`MAX_RECENT_FILES`]).
    #[serde(default)]
    pub recent_files: Vec<String>,
    #[serde(default)]
    pub update_use_custom_server: bool,
    #[serde(default = "default_update_repo_owner")]
    pub update_repo_owner: String,
    #[serde(default = "default_update_repo_name")]
    pub update_repo_name: String,
    #[serde(default)]
    pub update_custom_manifest_url: String,
    #[serde(default = "default_rp2040_input_pins")]
    pub rp2040_input_pins: [u8; 8],
    #[serde(default = "default_rp2040_output_pins")]
    pub rp2040_output_pins: [u8; 8],
    #[serde(default = "default_rp2040_status_led_pin")]
    pub rp2040_status_led_pin: u8,
    #[serde(default = "default_rp2040_io_flags")]
    pub rp2040_io_flags: u8,
    #[serde(default = "default_rp2040_rs485_de_re_pin")]
    pub rp2040_rs485_de_re_pin: u8,
    #[serde(default = "default_rp2040_rs485_config")]
    pub rp2040_rs485_config: u8,
}

fn default_rp2040_input_pins() -> [u8; 8] {
    my_canvas_core::HardwareIoConfig::default_rp2040().input_pins
}

fn default_rp2040_output_pins() -> [u8; 8] {
    my_canvas_core::HardwareIoConfig::default_rp2040().output_pins
}

fn default_rp2040_status_led_pin() -> u8 {
    my_canvas_core::HardwareIoConfig::default_rp2040().status_led_pin
}

fn default_rp2040_io_flags() -> u8 {
    my_canvas_core::HardwareIoConfig::default_rp2040().flags
}

fn default_rp2040_rs485_de_re_pin() -> u8 {
    my_canvas_core::HardwareIoConfig::default_rp2040().rs485_de_re_pin
}

fn default_rp2040_rs485_config() -> u8 {
    my_canvas_core::HardwareIoConfig::default_rp2040().rs485_config
}

fn default_update_repo_owner() -> String {
    crate::updater::DEFAULT_REPO_OWNER.to_string()
}

fn default_update_repo_name() -> String {
    crate::updater::DEFAULT_REPO_NAME.to_string()
}

/// Maximum number of entries kept in File → Open Recent.
pub const MAX_RECENT_FILES: usize = 10;

/// Prefix used for ladder elements serialized to the system clipboard.
pub const CLIPBOARD_ELEM_PREFIX: &str = "LADUINO-ELEM:";

/// Move `path` to the front of a recent-files list (deduplicated, capped at [`MAX_RECENT_FILES`]).
pub fn push_recent_file(list: &mut Vec<String>, path: &str) {
    if path.is_empty() {
        return;
    }
    list.retain(|p| p != path);
    list.insert(0, path.to_string());
    list.truncate(MAX_RECENT_FILES);
}

/// Parse `LADUINO-ELEM:<json>` clipboard text into ladder elements.
pub fn parse_clipboard_elements(text: &str) -> Option<Vec<my_canvas_core::LadderElement>> {
    let json = text.trim().strip_prefix(CLIPBOARD_ELEM_PREFIX)?;
    serde_json::from_str(json).ok()
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: AppTheme::Modern,
            schematic_style: SchematicStyle::default(),
            show_il_code: true,
            com_port: String::new(), // empty = auto-detect
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
            recent_files: Vec::new(),
            update_use_custom_server: false,
            update_repo_owner: default_update_repo_owner(),
            update_repo_name: default_update_repo_name(),
            update_custom_manifest_url: String::new(),
            rp2040_input_pins: default_rp2040_input_pins(),
            rp2040_output_pins: default_rp2040_output_pins(),
            rp2040_status_led_pin: default_rp2040_status_led_pin(),
            rp2040_io_flags: default_rp2040_io_flags(),
            rp2040_rs485_de_re_pin: default_rp2040_rs485_de_re_pin(),
            rp2040_rs485_config: default_rp2040_rs485_config(),
        }
    }
}

impl AppSettings {
    pub fn clamped_window_size(&self) -> (f32, f32) {
        let w = self.window_width.unwrap_or(1280.0).max(800.0);
        let h = self.window_height.unwrap_or(720.0).max(600.0);
        (w, h)
    }

    /// Location of `settings.json`:
    /// * Windows: `%APPDATA%\LaduinoPLC\settings.json`
    /// * macOS: `~/Library/Application Support/LaduinoPLC/settings.json`
    /// * Linux / other Unix: `$XDG_CONFIG_HOME/laduino_plc/settings.json` or
    ///   `~/.config/laduino_plc/settings.json` (legacy `uplc_studio` dir is honoured if present)
    pub fn config_path() -> PathBuf {
        let file_name = "settings.json";
        #[cfg(windows)]
        {
            if let Some(appdata) = std::env::var_os("APPDATA") {
                return PathBuf::from(appdata).join("LaduinoPLC").join(file_name);
            }
        }
        #[cfg(target_os = "macos")]
        {
            if let Some(home) = std::env::var_os("HOME") {
                return PathBuf::from(home)
                    .join("Library")
                    .join("Application Support")
                    .join("LaduinoPLC")
                    .join(file_name);
            }
        }
        if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
            let primary = PathBuf::from(&xdg).join("laduino_plc").join(file_name);
            if primary.exists() {
                return primary;
            }
            let legacy = PathBuf::from(xdg).join("uplc_studio").join(file_name);
            if legacy.exists() {
                return legacy;
            }
            primary
        } else if let Some(home) = std::env::var_os("HOME") {
            let base = PathBuf::from(home).join(".config");
            let primary = base.join("laduino_plc").join(file_name);
            if primary.exists() {
                return primary;
            }
            let legacy = base.join("uplc_studio").join(file_name);
            if legacy.exists() {
                return legacy;
            }
            primary
        } else {
            let primary = PathBuf::from("laduino_settings.json");
            if primary.exists() {
                return primary;
            }
            let legacy = PathBuf::from("uplc_settings.json");
            if legacy.exists() {
                return legacy;
            }
            primary
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
    ImportIl,
    UploadFromPlc,
    LoadExample(String),
    OpenRecent(String),
    ExitApp,
}

pub struct CanvasApp {
    pub doc: Document,
    pub active_tool: ActiveTool,
    pub insert_pos: InsertPosition,
    /// "Copy Network" clipboard: a snapshot of the copied rung (not an index, so later edits don't change it)
    pub copied_rung: Option<my_canvas_core::Rung>,
    /// Element-level clipboard (Ctrl+C / Ctrl+X / Ctrl+V)
    pub clipboard: Vec<my_canvas_core::LadderElement>,
    /// File → Open Recent (newest first)
    pub recent_files: Vec<String>,
    /// Find / Replace window state (Ctrl+F)
    pub find: FindState,
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
    pub is_example_template: bool,
    pub pending_action: Option<PendingAction>,
    pub allow_exit: bool,
    pub current_window_title: String,
    pub about_logo_texture: Option<egui::TextureHandle>,
    pub confirm_retain_action: Option<my_canvas_core::PlcSerialCommand>,
    pub uart_tx_history: Vec<u8>,
    pub uart_rx_input: String,
    pub uart_rx_mode: u8,
    pub uart_hex_view: bool,
    pub updater: crate::updater::AppUpdater,
    pub firmware_flasher: crate::firmware_flasher::FirmwareFlasher,
    pub hw_io_config: my_canvas_core::HardwareIoConfig,
}

impl Default for CanvasApp {
    fn default() -> Self {
        let settings = AppSettings::load();
        let mut doc = Document::new_empty();
        doc.set_schematic_style(settings.schematic_style);
        doc.is_dirty = false;
        doc.simulator.cycle_time_ms = settings.cycle_time_ms.clamp(1, 1000);
        if !settings.hardware.is_empty() {
            doc.program.hardware = settings.hardware;
        }
        let mut hw_io_config = my_canvas_core::HardwareIoConfig {
            input_pins: settings.rp2040_input_pins,
            output_pins: settings.rp2040_output_pins,
            status_led_pin: settings.rp2040_status_led_pin,
            flags: settings.rp2040_io_flags,
            rs485_de_re_pin: settings.rp2040_rs485_de_re_pin,
            rs485_config: settings.rp2040_rs485_config,
        };
        hw_io_config.sanitize();

        let mut app = Self {
            doc,
            active_tool: ActiveTool::Select,
            insert_pos: InsertPosition::AppendRung,
            copied_rung: None,
            clipboard: Vec::new(),
            recent_files: settings.recent_files.clone(),
            find: FindState::default(),
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
            is_example_template: false,
            pending_action: None,
            allow_exit: false,
            current_window_title: String::new(),
            about_logo_texture: None,
            confirm_retain_action: None,
            uart_tx_history: Vec::new(),
            uart_rx_input: String::new(),
            uart_rx_mode: 0,
            uart_hex_view: false,
            updater: crate::updater::AppUpdater::with_settings(
                settings.update_use_custom_server,
                if settings.update_repo_owner.trim().is_empty() {
                    default_update_repo_owner()
                } else {
                    settings.update_repo_owner
                },
                if settings.update_repo_name.trim().is_empty() {
                    default_update_repo_name()
                } else {
                    settings.update_repo_name
                },
                settings.update_custom_manifest_url,
            ),
            firmware_flasher: crate::firmware_flasher::FirmwareFlasher::default(),
            hw_io_config,
        };
        app.sync_doc_io_mappings_from_hw_config();
        app
    }
}

impl CanvasApp {
    /// List serial ports for the port picker as `name (label)` display strings.
    pub fn available_serial_ports() -> Vec<String> {
        serial::list_ports().iter().map(|p| p.display()).collect()
    }

    /// Find a connected RP2040 Micro-VM USB-CDC port (e.g. `/dev/ttyACM*`, `COM*`, `/dev/cu.usbmodem*`)
    pub fn detect_uplc_usb_port() -> Option<String> {
        serial::detect_plc_port()
    }

    /// Resolve the configured `com_port` (empty = auto-detect) into an openable port name.
    ///
    /// If an explicitly configured port is missing or is a Debugprobe and another Micro-VM
    /// port was detected instead, `com_port` is updated to the detected port.
    /// An empty (auto-detect) setting is left untouched.
    fn resolve_serial_port(&mut self) -> Option<String> {
        let resolved = serial::resolve_port(&self.com_port)?;
        if let Some(configured) = serial::normalize_port_name(&self.com_port) {
            if !serial::same_port(&configured, &resolved) {
                self.com_port = resolved.clone();
            }
        }
        Some(resolved)
    }

    /// Human-readable configured port for status messages (`Auto` when empty).
    pub fn com_port_display(&self) -> &str {
        if self.com_port.trim().is_empty() {
            "Auto"
        } else {
            self.com_port.as_str()
        }
    }

    /// Read the compiled bytecode directly from the connected PLC Micro-VM via CMD 0x04
    /// and decompile it back into an active `LadderProgram` (Rungs & Elements).
    pub fn upload_program_from_plc(&mut self) -> Result<my_canvas_core::LadderProgram, String> {
        let was_online = self.online_mode && self.online_target_hw;
        if was_online {
            self.stop_online_monitoring();
            thread::sleep(Duration::from_millis(60));
        }

        let Some(port_path) = self.resolve_serial_port() else {
            if was_online {
                self.start_online_monitoring();
            }
            return Err(format!("Serial port '{}' not connected", self.com_port_display()));
        };
        let baud = serial::parse_baud(&self.baud_rate);

        let res = (|| -> Result<my_canvas_core::LadderProgram, String> {
            use std::io::Write;
            // Raw 8N1, 200 ms read timeout (equivalent to legacy `stty ... min 0 time 2`)
            let mut file = serial::open(&port_path, baud, 200)
                .map_err(|e| format!("Failed to open {}: {}", port_path, e))?;

            // Drain any pending bytes
            let mut drain = [0u8; 512];
            let _ = serial::read_some(&mut *file, &mut drain);

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
                match serial::read_some(&mut *file, &mut chunk) {
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
        use std::io::Write;
        let bc = self.doc.compile_to_bytecode();
        if cmd == my_canvas_core::PlcSerialCommand::UploadProgram {
            match self.upload_program_from_plc() {
                Ok(ladder) => {
                    let n_rungs = ladder.rungs.len();
                    self.doc.push_undo_snapshot();
                    self.doc.program = ladder;
                    self.doc.selected_rung_idx = Some(0);
                    self.doc.selected_element_id = None;
                    self.doc.selected_element_ids.clear();
                    self.doc.file_path = None;
                    self.doc.is_dirty = true;
                    self.is_example_template = false;
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
                            my_canvas_core::PlcSerialCommand::PersistSave => {
                                self.update_plc_status = "💾 Save RETAIN to Flash (0x08) sent to hardware".to_string();
                            }
                            my_canvas_core::PlcSerialCommand::PersistClear => {
                                self.update_plc_status = "🗑 Clear RETAIN (0x09) sent to hardware".to_string();
                            }
                            my_canvas_core::PlcSerialCommand::EnterBootsel => {
                                self.plc_remote_running = false;
                                self.stop_online_monitoring();
                                self.update_plc_status =
                                    "⚡ Rebooting RP2040 into USB BOOTSEL (RPI-RP2) mode...".to_string();
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

        // Resolve the configured port (empty = auto); auto-selects the RP2040 Micro-VM USB-CDC
        // port if the configured one doesn't exist or is a Debugprobe
        let resolved_port = self.resolve_serial_port();
        let port_path = resolved_port
            .clone()
            .unwrap_or_else(|| self.com_port_display().to_string());

        // Raw 8N1, 100 ms read timeout (equivalent to legacy `stty ... min 0 time 1`)
        let open_res = match resolved_port {
            Some(ref p) => serial::open(p, serial::parse_baud(&self.baud_rate), 100),
            None => Err(std::io::Error::new(std::io::ErrorKind::NotFound, "no serial port found")),
        };

        match open_res {
            Ok(mut file) => match file.write_all(&packet) {
                Ok(_) => {
                    let _ = file.flush();
                    // Briefly drain any 3-byte ACK ([0xAA, 0x81/0x82/0x85, 0x00]) so RX buffer stays clean
                    thread::sleep(Duration::from_millis(15));
                    let mut ack_drain = [0u8; 32];
                    let _ = serial::read_some(&mut *file, &mut ack_drain);

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
                        my_canvas_core::PlcSerialCommand::PersistSave => {
                            self.update_plc_status = format!("💾 Save RETAIN to Flash (0x08) sent to {}", port_path);
                        }
                        my_canvas_core::PlcSerialCommand::PersistClear => {
                            self.update_plc_status = format!("🗑 Clear RETAIN (0x09) sent to {}", port_path);
                        }
                        my_canvas_core::PlcSerialCommand::EnterBootsel => {
                            self.plc_remote_running = false;
                            self.update_plc_status = format!(
                                "⚡ Reboot to BOOTSEL (0x0A) sent to {} — RP2040 is mounting as RPI-RP2 drive",
                                port_path
                            );
                        }
                        my_canvas_core::PlcSerialCommand::GetIoConfig => {
                            self.update_plc_status = format!(
                                "📥 Read Hardware I/O Pin Mapping (0x0B) sent to {}",
                                port_path
                            );
                        }
                        my_canvas_core::PlcSerialCommand::SetIoConfig => {
                            self.update_plc_status = format!(
                                "✔ Applied & saved Hardware I/O Pin Mapping (0x0C) to {} Flash (Sector 509 / 0x1FD000) without rebuilding firmware!",
                                port_path
                            );
                        }
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
            recent_files: self.recent_files.clone(),
            update_use_custom_server: self.updater.use_custom_server,
            update_repo_owner: self.updater.repo_owner.clone(),
            update_repo_name: self.updater.repo_name.clone(),
            update_custom_manifest_url: self.updater.custom_manifest_url.clone(),
            rp2040_input_pins: self.hw_io_config.input_pins,
            rp2040_output_pins: self.hw_io_config.output_pins,
            rp2040_status_led_pin: self.hw_io_config.status_led_pin,
            rp2040_io_flags: self.hw_io_config.flags,
            rp2040_rs485_de_re_pin: self.hw_io_config.rs485_de_re_pin,
            rp2040_rs485_config: self.hw_io_config.rs485_config,
        }
    }

    /// Format an RP2040 GPIO pin index (`0..=29` or `0xFF`) for display
    pub fn format_rp2040_pin(pin: u8) -> String {
        if pin < 30 && !matches!(pin, 0 | 1 | 26 | 27 | 28) {
            format!("GP{}", pin)
        } else {
            "-".to_string()
        }
    }

    /// Sync `self.doc.program.io_mappings` (`X0..X7`, `Y0..Y7`) with `self.hw_io_config`
    pub fn sync_doc_io_mappings_from_hw_config(&mut self) {
        for i in 0..8 {
            let plc_addr = format!("X{}", i);
            let iec_addr = format!("%IX0.{}", i);
            let hw_pin = Self::format_rp2040_pin(self.hw_io_config.input_pins[i]);
            if let Some(existing) = self
                .doc
                .program
                .io_mappings
                .iter_mut()
                .find(|m| m.is_input && (m.plc_address == plc_addr || m.iec_address == iec_addr))
            {
                existing.hw_pin = hw_pin;
            } else if self.hw_io_config.input_pins[i] != my_canvas_core::PIN_UNASSIGNED {
                self.doc.program.io_mappings.push(IOPortMapping {
                    plc_address: plc_addr,
                    iec_address: iec_addr,
                    symbol: format!("IN_{}", i),
                    hw_pin,
                    description: "RP2040 Digital Input".to_string(),
                    is_input: true,
                });
            }
        }
        for i in 0..8 {
            let plc_addr = format!("Y{}", i);
            let iec_addr = format!("%QX0.{}", i);
            let hw_pin = Self::format_rp2040_pin(self.hw_io_config.output_pins[i]);
            if let Some(existing) = self
                .doc
                .program
                .io_mappings
                .iter_mut()
                .find(|m| !m.is_input && (m.plc_address == plc_addr || m.iec_address == iec_addr))
            {
                existing.hw_pin = hw_pin;
            } else if self.hw_io_config.output_pins[i] != my_canvas_core::PIN_UNASSIGNED {
                self.doc.program.io_mappings.push(IOPortMapping {
                    plc_address: plc_addr,
                    iec_address: iec_addr,
                    symbol: format!("OUT_{}", i),
                    hw_pin,
                    description: "RP2040 Digital Output".to_string(),
                    is_input: false,
                });
            }
        }
    }

    /// Read current `HardwareIoConfig` from the connected RP2040 PLC via `CMD_GET_IO_CONFIG (0x0B)`
    pub fn read_io_config_from_plc(&mut self) -> Result<my_canvas_core::HardwareIoConfig, String> {
        let was_online = self.online_mode && self.online_target_hw;
        if was_online {
            self.stop_online_monitoring();
            thread::sleep(Duration::from_millis(60));
        }

        let Some(port_path) = self.resolve_serial_port() else {
            if was_online {
                self.start_online_monitoring();
            }
            return Err(format!("Serial port '{}' not connected", self.com_port_display()));
        };
        let baud = serial::parse_baud(&self.baud_rate);

        let res = (|| -> Result<my_canvas_core::HardwareIoConfig, String> {
            use std::io::Write;
            let mut file = serial::open(&port_path, baud, 200)
                .map_err(|e| format!("Failed to open {}: {}", port_path, e))?;

            let mut drain = [0u8; 512];
            let _ = serial::read_some(&mut *file, &mut drain);

            let req = my_canvas_core::PlcSerialCommand::create_get_io_config_packet();
            file.write_all(&req)
                .map_err(|e| format!("Failed to write to {}: {}", port_path, e))?;
            file.flush()
                .map_err(|e| format!("Failed to flush {}: {}", port_path, e))?;

            let mut rx_accum = Vec::with_capacity(64);
            let mut chunk = [0u8; 64];
            let start = Instant::now();

            while start.elapsed().as_millis() < 600 {
                match serial::read_some(&mut *file, &mut chunk) {
                    Ok(0) => thread::sleep(Duration::from_millis(10)),
                    Ok(n) => {
                        rx_accum.extend_from_slice(&chunk[..n]);
                        if let Some(pos) = rx_accum.windows(2).position(|w| w == [0xAA, 0x8B]) {
                            if pos > 0 {
                                rx_accum.drain(..pos);
                            }
                            if rx_accum.len() >= 4 {
                                let payload_len = u16::from_le_bytes([rx_accum[2], rx_accum[3]]) as usize;
                                let total_len = 5 + payload_len;
                                if rx_accum.len() >= total_len {
                                    let frame: Vec<u8> = rx_accum.drain(..total_len).collect();
                                    return my_canvas_core::PlcSerialCommand::parse_io_config_response(&frame);
                                }
                            }
                        }
                    }
                    Err(e) => return Err(format!("Serial read error: {}", e)),
                }
            }
            Err("Timed out waiting for CMD 0x8B IO config response from PLC".to_string())
        })();

        if was_online {
            self.online_mode = false;
            self.start_online_monitoring();
        }

        match res {
            Ok(cfg) => {
                self.hw_io_config = cfg;
                self.sync_doc_io_mappings_from_hw_config();
                self.save_settings();
                self.update_plc_is_error = false;
                self.update_plc_status = format!(
                    "✔ Loaded Hardware I/O Pin Mapping from PLC ({}) — Status LED: {}",
                    port_path,
                    Self::format_rp2040_pin(cfg.status_led_pin)
                );
                Ok(cfg)
            }
            Err(e) => {
                self.update_plc_is_error = true;
                self.update_plc_status = format!("✖ Read I/O Config from PLC failed: {}", e);
                Err(e)
            }
        }
    }

    /// Apply and save `self.hw_io_config` to the connected RP2040 PLC Flash via `CMD_SET_IO_CONFIG (0x0C)`
    pub fn apply_io_config_to_plc(&mut self) {
        self.hw_io_config.sanitize();
        self.sync_doc_io_mappings_from_hw_config();
        self.save_settings();
        let pkt = my_canvas_core::PlcSerialCommand::create_set_io_config_packet(&self.hw_io_config);
        self.transmit_custom_packet_to_plc(my_canvas_core::PlcSerialCommand::SetIoConfig, pkt);
    }

    /// Render the interactive RP2040 Hardware I/O Pin Mapping configuration table & Flash controls.
    /// Returns `true` if any setting was modified.
    pub fn render_rp2040_io_pin_config_ui(&mut self, ui: &mut Ui, id_prefix: &str) -> bool {
        let palette = self.theme.palette();
        let mut changed = false;

        // Candidate GPIO pins on RP2040 (excludes UART0 GP0..1 and ADC GP26..28; active UART1 TX/RX pair is filtered out dynamically)
        const ALL_CANDIDATE_PINS: [u8; 26] = [
            my_canvas_core::PIN_UNASSIGNED,
            2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
            29,
        ];
        let (rs485_tx, rs485_rx) = self.hw_io_config.rs485_tx_rx_pins();
        let assignable_pins: Vec<u8> = ALL_CANDIDATE_PINS
            .iter()
            .copied()
            .filter(|&p| p == my_canvas_core::PIN_UNASSIGNED || (p != rs485_tx && p != rs485_rx))
            .collect();

        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(8.0, 6.0);

            // Header & Action Toolbar Row
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new("🔧 RP2040 Runtime GPIO & RS-485 Pin Mapping")
                        .color(palette.accent_heading)
                        .strong()
                        .size(12.5),
                );
                ui.separator();

                let read_btn = egui::Button::new(
                    RichText::new("📥 Read from PLC (0x0B)")
                        .strong()
                        .size(11.0),
                )
                .corner_radius(egui::CornerRadius::same(5));
                if ui
                    .add(read_btn)
                    .on_hover_text("Read current GPIO & RS-485 pin mapping stored on the connected RP2040 PLC via USB-CDC (CMD 0x0B)")
                    .clicked()
                {
                    let _ = self.read_io_config_from_plc();
                }

                let apply_btn = egui::Button::new(
                    RichText::new("⚡ Apply & Save to PLC Flash (0x0C)")
                        .strong()
                        .size(11.0)
                        .color(egui::Color32::WHITE),
                )
                .fill(egui::Color32::from_rgb(39, 174, 96))
                .corner_radius(egui::CornerRadius::same(5));
                if ui
                    .add(apply_btn)
                    .on_hover_text("Immediately reconfigure RP2040 GPIO & RS-485 pins at runtime and save to Flash Sector 509 (0x1FD000) without rebuilding firmware!")
                    .clicked()
                {
                    self.apply_io_config_to_plc();
                }

                if ui
                    .button(RichText::new("↺ Reset Default").size(11.0))
                    .on_hover_text("Restore factory default RP2040 GPIO & RS-485 mapping (X0..X3 = GP10..GP13, Y0..Y5 = GP15,14,7,6,2,3, RS-485 = GP8/GP9 @ 9600, LED = GP25)")
                    .clicked()
                {
                    self.hw_io_config = my_canvas_core::HardwareIoConfig::default_rp2040();
                    self.sync_doc_io_mappings_from_hw_config();
                    changed = true;
                }
            });

            let de_re_summary = if self.hw_io_config.rs485_de_re_pin == my_canvas_core::PIN_UNASSIGNED {
                "Auto-Direction (None)".to_string()
            } else {
                format!("GP{}", self.hw_io_config.rs485_de_re_pin)
            };
            ui.label(
                RichText::new(format!(
                    "Reserved peripherals: UART0 (GP0=TX, GP1=RX) • Modbus RS-485 UART1 (GP{}=TX, GP{}=RX, DE/RE={}, {} bps) • Analog ADC (GP26..GP28)",
                    rs485_tx,
                    rs485_rx,
                    de_re_summary,
                    self.hw_io_config.rs485_baud_rate()
                ))
                .size(10.5)
                .color(palette.symbol_subtext),
            );

            // Unified 7-Column Grid for Inputs X0..X7 (Left) & Outputs Y0..Y7 (Right) — prevents any column overlap
            egui::Grid::new(format!("{}_io_pins_unified_grid", id_prefix))
                .striped(true)
                .num_columns(7)
                .min_col_width(36.0)
                .spacing([12.0, 5.0])
                .show(ui, |ui| {
                    ui.label(RichText::new("Input").strong().color(palette.accent_ok));
                    ui.label(RichText::new("IEC Addr").strong().color(palette.accent_ok));
                    ui.label(RichText::new("RP2040 GPIO Pin").strong().color(palette.accent_ok));
                    ui.label(RichText::new("│").weak());
                    ui.label(
                        RichText::new("Output")
                            .strong()
                            .color(egui::Color32::from_rgb(52, 152, 219)),
                    );
                    ui.label(
                        RichText::new("IEC Addr")
                            .strong()
                            .color(egui::Color32::from_rgb(52, 152, 219)),
                    );
                    ui.label(
                        RichText::new("RP2040 GPIO Pin")
                            .strong()
                            .color(egui::Color32::from_rgb(52, 152, 219)),
                    );
                    ui.end_row();

                    for i in 0..8 {
                        // Left: Digital Input Xi
                        ui.monospace(format!("X{}", i));
                        ui.monospace(format!("%IX0.{}", i));
                        let in_pin = self.hw_io_config.input_pins[i];
                        let in_label = if in_pin == my_canvas_core::PIN_UNASSIGNED {
                            "Disabled (-)".to_string()
                        } else {
                            format!("GP{}", in_pin)
                        };
                        egui::ComboBox::from_id_salt(format!("{}_in_pin_{}", id_prefix, i))
                            .width(115.0)
                            .selected_text(in_label)
                            .show_ui(ui, |ui| {
                                for &p in &assignable_pins {
                                    let lbl = if p == my_canvas_core::PIN_UNASSIGNED {
                                        "Disabled (-)".to_string()
                                    } else {
                                        format!("GP{}", p)
                                    };
                                    if ui
                                        .selectable_value(&mut self.hw_io_config.input_pins[i], p, lbl)
                                        .changed()
                                    {
                                        self.sync_doc_io_mappings_from_hw_config();
                                        changed = true;
                                    }
                                }
                            });

                        ui.label(RichText::new("│").weak());

                        // Right: Digital Output Yi
                        ui.monospace(format!("Y{}", i));
                        ui.monospace(format!("%QX0.{}", i));
                        let out_pin = self.hw_io_config.output_pins[i];
                        let out_label = if out_pin == my_canvas_core::PIN_UNASSIGNED {
                            "Disabled (-)".to_string()
                        } else {
                            format!("GP{}", out_pin)
                        };
                        egui::ComboBox::from_id_salt(format!("{}_out_pin_{}", id_prefix, i))
                            .width(115.0)
                            .selected_text(out_label)
                            .show_ui(ui, |ui| {
                                for &p in &assignable_pins {
                                    let lbl = if p == my_canvas_core::PIN_UNASSIGNED {
                                        "Disabled (-)".to_string()
                                    } else {
                                        format!("GP{}", p)
                                    };
                                    if ui
                                        .selectable_value(&mut self.hw_io_config.output_pins[i], p, lbl)
                                        .changed()
                                    {
                                        self.sync_doc_io_mappings_from_hw_config();
                                        changed = true;
                                    }
                                }
                            });
                        ui.end_row();
                    }
                });

            ui.separator();

            // RS-485 (Modbus RTU — UART1) Configuration Row
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(10.0, 6.0);
                ui.label(
                    RichText::new("📡 RS-485 (Modbus RTU):")
                        .strong()
                        .color(palette.accent_heading),
                );

                // 1. UART1 TX/RX Pin Pair selector
                ui.label(RichText::new("UART1 TX/RX:").strong());
                let mut use_alt_gp4_gp5 = self.hw_io_config.rs485_use_alt_pins_gp4_gp5();
                let uart1_pair_lbl = if use_alt_gp4_gp5 {
                    "GP4 (TX) / GP5 (RX) [Alt]"
                } else {
                    "GP8 (TX) / GP9 (RX) [Default]"
                };
                egui::ComboBox::from_id_salt(format!("{}_rs485_tx_rx_pair", id_prefix))
                    .width(195.0)
                    .selected_text(uart1_pair_lbl)
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_value(
                                &mut use_alt_gp4_gp5,
                                false,
                                "GP8 (TX) / GP9 (RX) [Default]",
                            )
                            .changed()
                            || ui
                                .selectable_value(
                                    &mut use_alt_gp4_gp5,
                                    true,
                                    "GP4 (TX) / GP5 (RX) [Alt]",
                                )
                                .changed()
                        {
                            self.hw_io_config
                                .set_rs485_use_alt_pins_gp4_gp5(use_alt_gp4_gp5);
                            self.hw_io_config.sanitize();
                            self.sync_doc_io_mappings_from_hw_config();
                            changed = true;
                        }
                    });

                // 2. DE/RE Direction Control Pin selector
                ui.label(RichText::new("DE/RE Pin:").strong());
                let de_pin = self.hw_io_config.rs485_de_re_pin;
                let de_lbl = if de_pin == my_canvas_core::PIN_UNASSIGNED {
                    "Auto-Direction (None)".to_string()
                } else {
                    format!("GP{}", de_pin)
                };
                egui::ComboBox::from_id_salt(format!("{}_rs485_de_re_pin", id_prefix))
                    .width(155.0)
                    .selected_text(de_lbl)
                    .show_ui(ui, |ui| {
                        for &p in &assignable_pins {
                            let lbl = if p == my_canvas_core::PIN_UNASSIGNED {
                                "Auto-Direction (None)".to_string()
                            } else if p == 6 || p == 7 || p == 16 {
                                format!("GP{} (Recommended)", p)
                            } else {
                                format!("GP{}", p)
                            };
                            if ui
                                .selectable_value(&mut self.hw_io_config.rs485_de_re_pin, p, lbl)
                                .changed()
                            {
                                self.hw_io_config.sanitize();
                                self.sync_doc_io_mappings_from_hw_config();
                                changed = true;
                            }
                        }
                    });

                // 3. Modbus RTU Baud Rate selector
                ui.label(RichText::new("Baud Rate:").strong());
                let current_baud = self.hw_io_config.rs485_baud_rate();
                egui::ComboBox::from_id_salt(format!("{}_rs485_baud", id_prefix))
                    .width(115.0)
                    .selected_text(format!("{} bps", current_baud))
                    .show_ui(ui, |ui| {
                        for &baud in &[9_600u32, 19_200, 38_400, 57_600, 115_200] {
                            let lbl = if baud == 9_600 {
                                "9600 bps (Default)".to_string()
                            } else {
                                format!("{} bps", baud)
                            };
                            if ui
                                .selectable_label(current_baud == baud, lbl)
                                .clicked()
                            {
                                self.hw_io_config.set_rs485_baud_rate(baud);
                                changed = true;
                            }
                        }
                    });
            });

            ui.separator();

            // Electrical & Status LED Options Row
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(10.0, 6.0);
                ui.label(RichText::new("Status LED Pin:").strong());
                let led_pin = self.hw_io_config.status_led_pin;
                let led_lbl = if led_pin == my_canvas_core::PIN_UNASSIGNED {
                    "Disabled (-)".to_string()
                } else if led_pin == 25 {
                    "GP25 (Onboard LED)".to_string()
                } else {
                    format!("GP{}", led_pin)
                };
                egui::ComboBox::from_id_salt(format!("{}_status_led_pin", id_prefix))
                    .width(140.0)
                    .selected_text(led_lbl)
                    .show_ui(ui, |ui| {
                        for &p in &assignable_pins {
                            let lbl = if p == my_canvas_core::PIN_UNASSIGNED {
                                "Disabled (-)".to_string()
                            } else if p == 25 {
                                "GP25 (Onboard LED)".to_string()
                            } else {
                                format!("GP{}", p)
                            };
                            if ui
                                .selectable_value(&mut self.hw_io_config.status_led_pin, p, lbl)
                                .changed()
                            {
                                changed = true;
                            }
                        }
                    });

                let mut pull_up = self.hw_io_config.input_pull_up();
                if ui
                    .checkbox(&mut pull_up, "Input Pull-Up")
                    .on_hover_text("Checked = Internal Pull-Up resistor; Unchecked = Internal Pull-Down resistor")
                    .changed()
                {
                    self.hw_io_config.set_input_pull_up(pull_up);
                    changed = true;
                }

                let mut active_low = self.hw_io_config.input_active_low();
                if ui
                    .checkbox(&mut active_low, "Input Active-Low (GND=ON)")
                    .on_hover_text("Checked = Active-Low (0V/GND triggers logic 1); Unchecked = Active-High (3.3V triggers logic 1)")
                    .changed()
                {
                    self.hw_io_config.set_input_active_low(active_low);
                    changed = true;
                }

                let mut led_hb = self.hw_io_config.status_led_heartbeat();
                if ui
                    .checkbox(&mut led_hb, "RUN LED 1Hz Heartbeat")
                    .on_hover_text("Checked = 1 Hz blinking heartbeat in RUN mode; Unchecked = Solid ON in RUN mode")
                    .changed()
                {
                    self.hw_io_config.set_status_led_heartbeat(led_hb);
                    changed = true;
                }
            });
        });

        changed
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
            self.doc.simulator.write_word(&addr, num_val);
            Ok(())
        } else {
            self.doc.simulator.write_word(&addr, num_val);
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

    /// Return whether the active document is a bundled example template or loaded from an example directory.
    pub fn is_example_file(&self) -> bool {
        if self.is_example_template {
            return true;
        }
        if let Some(ref path) = self.doc.file_path {
            let norm = path.replace('\\', "/");
            if norm.starts_with("example/") || norm.starts_with("./example/") || norm.contains("/example/") {
                return true;
            }
        }
        false
    }

    /// Return the directory where file dialogs should open:
    /// Priority: 1. `last_folder` (if exists and not example directory), 2. current working directory.
    pub fn dialog_directory(&self) -> PathBuf {
        if let Some(ref folder) = self.last_folder {
            let s = folder.to_string_lossy().replace('\\', "/");
            if folder.exists() && folder.is_dir() && s != "example" && !s.ends_with("/example") {
                return folder.clone();
            }
        }
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    }

    /// Record the parent directory of a selected file as `last_folder` and persist it.
    /// Does not store bundled example directories as `last_folder` to keep user projects clean.
    pub fn set_last_folder_from_path(&mut self, file_path: &std::path::Path) {
        if let Some(parent) = file_path.parent() {
            let s = parent.to_string_lossy().replace('\\', "/");
            if s == "example" || s == "./example" || s.ends_with("/example") {
                return;
            }
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
            let _ = self.open_path(&path);
        }
    }

    /// Load a `.il` / `.ld` / `.json` program from `path` into the active document
    /// and record it in the recent-files list. Shared by Open… and Open Recent.
    pub fn open_path(&mut self, path: &std::path::Path) -> Result<(), String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Cannot read {}: {}", path.display(), e))?;
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            let doc = serde_json::from_str(&content)
                .map_err(|e| format!("Invalid project {}: {}", path.display(), e))?;
            self.doc = doc;
        } else {
            self.doc.load_from_il_str(&content)?;
        }
        self.doc.file_path = Some(path.display().to_string());
        self.doc.is_dirty = false;
        self.is_example_template = false;
        self.add_recent_file(&path.display().to_string());
        Ok(())
    }

    /// Record `path` at the top of File → Open Recent and persist settings.
    pub fn add_recent_file(&mut self, path: &str) {
        push_recent_file(&mut self.recent_files, path);
        self.save_settings();
    }

    /// Clear File → Open Recent.
    pub fn clear_recent_files(&mut self) {
        self.recent_files.clear();
        self.save_settings();
    }

    /// Open a recent file. Prompts first if unsaved changes exist.
    pub fn request_open_recent(&mut self, path: String) {
        if self.doc.is_dirty {
            self.pending_action = Some(PendingAction::OpenRecent(path));
        } else {
            self.open_recent_file(&path);
        }
    }

    /// Open a recent file now; a file that can no longer be read is dropped from the list.
    fn open_recent_file(&mut self, path: &str) {
        let p = std::path::Path::new(path);
        if self.open_path(p).is_ok() {
            self.set_last_folder_from_path(p);
        } else if !p.exists() {
            self.recent_files.retain(|r| r != path);
            self.save_settings();
        }
    }

    /// Copy the selected elements to the internal clipboard and (as `LADUINO-ELEM:` JSON)
    /// to the system clipboard. Returns false if nothing is selected.
    pub fn copy_selected_elements(&mut self, ctx: &egui::Context) -> bool {
        let els = self.doc.copy_selected();
        if els.is_empty() {
            return false;
        }
        if let Ok(json) = serde_json::to_string(&els) {
            ctx.copy_text(format!("{}{}", CLIPBOARD_ELEM_PREFIX, json));
        }
        self.clipboard = els;
        true
    }

    /// Copy then delete the selected elements (one undo step for the delete).
    pub fn cut_selected_elements(&mut self, ctx: &egui::Context) -> bool {
        if self.copy_selected_elements(ctx) {
            self.doc.delete_selected_element();
            true
        } else {
            false
        }
    }

    /// Paste elements after the current selection (or append to the selected network).
    ///
    /// `system_text` is the system clipboard content (from `egui::Event::Paste`); if it holds
    /// `LADUINO-ELEM:` JSON it is used, otherwise the internal clipboard is pasted.
    /// Returns the ids of the pasted top-level elements.
    pub fn paste_clipboard_elements(&mut self, system_text: Option<&str>) -> Vec<u64> {
        let els = system_text
            .and_then(parse_clipboard_elements)
            .unwrap_or_else(|| self.clipboard.clone());
        if els.is_empty() {
            return Vec::new();
        }
        let pos = if self.doc.selected_ids().is_empty() {
            InsertPosition::AppendRung
        } else {
            InsertPosition::Right
        };
        self.doc.paste_elements(&els, pos)
    }

    /// Edit → Copy Network: snapshot the selected rung.
    pub fn copy_selected_network(&mut self) {
        self.copied_rung = self
            .doc
            .selected_rung_idx
            .and_then(|i| self.doc.program.rungs.get(i).cloned());
    }

    /// Edit → Paste Network Below: insert the copied rung snapshot (or duplicate the selected one).
    pub fn paste_network_below(&mut self) {
        if let Some(idx) = self.doc.selected_rung_idx {
            if let Some(rung) = self.copied_rung.clone() {
                self.doc.paste_rung(&rung, idx + 1);
            } else {
                self.doc.copy_rung_at(idx, idx + 1);
            }
        }
    }

    /// Open the Find window (Ctrl+F), pre-filling the query from the selected element.
    pub fn open_find(&mut self) {
        if !self.find.open && self.find.query.is_empty() {
            if let Some(el) = self.doc.selected_element() {
                self.find.query = el.name.clone();
            }
        }
        self.find.open = true;
        self.find.request_focus = true;
    }

    /// Jump to the next (`delta = 1`) / previous (`delta = -1`) Find result (F3 / Shift+F3).
    pub fn find_step(&mut self, delta: isize) {
        self.find.refresh(&self.doc);
        if let Some(i) = self.find.step(delta) {
            self.goto_find_result(i);
        }
    }

    /// Select the element of Find result `i` and scroll the canvas to its network.
    pub fn goto_find_result(&mut self, i: usize) {
        if let Some(hit) = self.find.results.get(i).cloned() {
            self.doc.set_single_selection(Some(hit.rung_idx), hit.elem_id);
            self.canvas.scroll_to_rung(&self.doc, hit.rung_idx);
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
                self.is_example_template = false;
                self.add_recent_file(&path.display().to_string());
                return true;
            }
        }
        false
    }

    /// Save current project to `.json` file, defaulting to `last_folder`. Returns true if saved.
    pub fn save_as_json_dialog(&mut self) -> bool {
        let mut dialog = rfd::FileDialog::new()
            .set_directory(self.dialog_directory())
            .add_filter("LaduinoPLC JSON Project (*.json)", &["json"]);
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
                    self.is_example_template = false;
                    self.add_recent_file(&path.display().to_string());
                    return true;
                }
            }
        }
        false
    }

    /// Quick-save directly to existing file_path if known; otherwise open Save dialog. Returns true if saved.
    pub fn save_or_save_as(&mut self) -> bool {
        // If current document is a demo/example template, do not overwrite original template. Prompt Save dialog.
        if self.is_example_file() {
            return self.save_program_dialog();
        }

        if let Some(current_path) = self.doc.file_path.clone() {
            let path = std::path::Path::new(&current_path);
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Ok(json) = serde_json::to_string_pretty(&self.doc) {
                    if std::fs::write(path, json).is_ok() {
                        self.doc.is_dirty = false;
                        self.is_example_template = false;
                        self.add_recent_file(&current_path);
                        return true;
                    }
                }
            } else {
                let il_text = self.doc.export_to_il_str();
                if std::fs::write(path, il_text).is_ok() {
                    self.doc.is_dirty = false;
                    self.is_example_template = false;
                    self.add_recent_file(&current_path);
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
                self.is_example_template = false;
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

    /// Request importing an IL file. Prompts if unsaved changes exist.
    pub fn request_import_il(&mut self) {
        if self.doc.is_dirty {
            self.pending_action = Some(PendingAction::ImportIl);
        } else {
            self.import_il_dialog();
        }
    }

    /// Request uploading & decompiling program from connected PLC. Prompts if unsaved changes exist.
    pub fn request_upload_from_plc(&mut self) {
        if self.doc.is_dirty {
            self.pending_action = Some(PendingAction::UploadFromPlc);
        } else {
            self.transmit_bytecode_to_plc(my_canvas_core::PlcSerialCommand::UploadProgram);
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
            .or_else(|_| std::fs::read_to_string(format!("../../{}", path_str)))
            .or_else(|_| {
                if let Ok(exe) = std::env::current_exe() {
                    let mut cur = exe.parent();
                    while let Some(dir) = cur {
                        let candidate = dir.join(path_str);
                        if candidate.exists() {
                            if let Ok(c) = std::fs::read_to_string(&candidate) {
                                return Ok(c);
                            }
                        }
                        cur = dir.parent();
                    }
                }
                Err(std::io::Error::new(std::io::ErrorKind::NotFound, "not found"))
            });
        if let Ok(c) = content {
            let _ = self.doc.load_from_il_str(&c);
            self.doc.file_path = Some(path_str.to_string());
            self.doc.is_dirty = false;
            self.is_example_template = true;
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
                PendingAction::ImportIl => {
                    self.import_il_dialog();
                }
                PendingAction::UploadFromPlc => {
                    self.transmit_bytecode_to_plc(my_canvas_core::PlcSerialCommand::UploadProgram);
                }
                PendingAction::LoadExample(path_str) => {
                    self.load_example_file(&path_str);
                }
                PendingAction::OpenRecent(path_str) => {
                    self.open_recent_file(&path_str);
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
        let is_demo = self.is_example_file();
        let file_name = self.doc.file_path.as_deref().map(|p| {
            std::path::Path::new(p)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(p)
        }).unwrap_or("Untitled Program");

        let action_desc = match self.pending_action.as_ref() {
            Some(PendingAction::NewProgram) => "creating a new program",
            Some(PendingAction::OpenProgram) => "opening another program",
            Some(PendingAction::ImportIl) => "importing an IL file",
            Some(PendingAction::UploadFromPlc) => "reading the program from PLC",
            Some(PendingAction::LoadExample(_)) => "loading an example",
            Some(PendingAction::OpenRecent(_)) => "opening a recent program",
            Some(PendingAction::ExitApp) => "closing the application",
            None => "",
        };

        let mut do_save = false;
        let mut do_discard = false;
        let mut do_cancel = false;

        egui::Window::new("⚠ Unsaved Changes")
            .order(egui::Order::Foreground)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size([460.0, 150.0])
            .show(ctx, |ui| {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("⚠").size(28.0).color(palette.accent_warn));
                    ui.add_space(8.0);
                    ui.vertical(|ui| {
                        let prompt_title = if is_demo {
                            format!("Save modified example \"{}\" as a new file?", file_name)
                        } else {
                            format!("Do you want to save changes to \"{}\"?", file_name)
                        };
                        let prompt_subtext = if is_demo {
                            format!("This example has unsaved changes. Save it to a new file before {}?", action_desc)
                        } else {
                            format!("Your changes will be lost if you don't save them before {}.", action_desc)
                        };

                        ui.label(
                            RichText::new(prompt_title)
                                .strong()
                                .size(14.0)
                                .color(palette.text_primary),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(prompt_subtext)
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
                        if ui.button("Cancel").clicked()
                            || ui.input(|i| i.key_pressed(egui::Key::Escape))
                        {
                            do_cancel = true;
                        }

                        let discard_btn = egui::Button::new(
                            RichText::new("Don't Save").color(palette.accent_danger)
                        );
                        if ui.add(discard_btn).clicked() {
                            do_discard = true;
                        }

                        let save_text = if is_demo {
                            "💾 Save As..."
                        } else {
                            "💾 Save"
                        };
                        let save_btn = egui::Button::new(
                            RichText::new(save_text).color(Color32::WHITE)
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
            } else {
                // User cancelled the file save dialog; reset pending action so modal doesn't get trapped
                self.pending_action = None;
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

            // Resolve configured port (empty = auto-detect; skips Debugprobe)
            let mut port_path = self.resolve_serial_port().unwrap_or_default();
            let configured_port = self.com_port.clone();

            if port_path.is_empty() {
                self.online_status_text = format!("⚠ Waiting for port '{}'...", self.com_port_display());
            }

            let baud = serial::parse_baud(&self.baud_rate);
            let (tx, rx) = mpsc::channel::<(PlcStatusSnapshot, String)>();
            self.online_worker_rx = Some(rx);
            let (cmd_tx, cmd_rx) = mpsc::channel::<Vec<u8>>();
            self.online_worker_cmd_tx = Some(cmd_tx);

            thread::spawn(move || {
                use std::io::Write;
                // Pre-constructed MonitorStatus request packet: [0xAA, 0x03, 0x00, 0x00, 0xBD]
                let req = [0xAAu8, 0x03, 0x00, 0x00, 0xBD];
                let mut chunk = [0u8; 512];
                let mut rx_accum: Vec<u8> = Vec::with_capacity(1024);

                while !stop_flag.load(Ordering::Relaxed) {
                    // Check if current port_path exists; if not, attempt auto-detection (hotplug)
                    if !serial::port_present(&port_path) {
                        if let Some(detected) = serial::resolve_port(&configured_port) {
                            port_path = detected;
                        } else {
                            thread::sleep(Duration::from_millis(250));
                            continue;
                        }
                    }

                    // Raw 8N1, 100 ms read timeout (equivalent to legacy `stty ... min 0 time 1`)
                    match serial::open(&port_path, baud, 100) {
                        Ok(mut file) => {
                            // Drain any stale bytes on initial open
                            let _ = serial::read_some(&mut *file, &mut chunk);
                            rx_accum.clear();
                            let mut consecutive_failures = 0u32;

                            while !stop_flag.load(Ordering::Relaxed) {
                                // 1. Check and send any pending interactive commands from UI (RUN / STOP / STEP)
                                while let Ok(cmd_packet) = cmd_rx.try_recv() {
                                    if file.write_all(&cmd_packet).is_ok() && file.flush().is_ok() {
                                        thread::sleep(Duration::from_millis(5));
                                        let mut ack = [0u8; 16];
                                        let _ = serial::read_some(&mut *file, &mut ack);
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
                                    match serial::read_some(&mut *file, &mut chunk) {
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
                                        let _ = serial::read_some(&mut *file, &mut drain);
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
                            if let Some(detected) = serial::resolve_port(&configured_port) {
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
        self.is_example_template = false;
        self.elem_modal.close();
        self.canvas.context_menu_elem = None;
        self.canvas.reset_view();
        self.active_tool = ActiveTool::Select;
        self.copied_rung = None;
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

        // Dynamic Window Title with Dirty Marker (*) and Demo indicator
        let file_title = if let Some(ref p) = self.doc.file_path {
            std::path::Path::new(p)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(p)
        } else {
            "Untitled"
        };
        let demo_tag = if self.is_example_file() { " [Demo]" } else { "" };
        let dirty_suffix = if self.doc.is_dirty { "*" } else { "" };
        let full_title = format!("LaduinoPLC - {}{}{}", file_title, demo_tag, dirty_suffix);
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

        // Drain any newly transmitted bytes from UartSend blocks into the persistent UI history buffer
        let new_uart_tx = self.doc.simulator.take_uart_tx();
        if !new_uart_tx.is_empty() {
            self.uart_tx_history.extend_from_slice(&new_uart_tx);
            if self.uart_tx_history.len() > 4096 {
                let drop_n = self.uart_tx_history.len() - 4096;
                self.uart_tx_history.drain(..drop_n);
            }
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
                    // Keep an empty (auto-detect) setting as-is; otherwise follow the worker's port
                    if !port.is_empty()
                        && !self.com_port.trim().is_empty()
                        && !serial::normalize_port_name(&self.com_port)
                            .is_some_and(|c| serial::same_port(&c, &port))
                    {
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
                        self.com_port_display()
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
            || self.updater.show_window
            || self.firmware_flasher.show_window
            || self.pending_action.is_some();

        self.updater.poll();
        self.firmware_flasher.poll();

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

            // Find (Ctrl+F) and F3 / Shift+F3 work even while typing in the Find box
            if ui.input(|i| i.modifiers.command && !i.modifiers.shift && i.key_pressed(egui::Key::F)) {
                self.open_find();
            }
            if ui.input(|i| i.key_pressed(egui::Key::F3)) {
                let back = ui.input(|i| i.modifiers.shift);
                if !self.find.query.is_empty() {
                    self.find_step(if back { -1 } else { 1 });
                } else {
                    self.open_find();
                }
            }

            if !ui.ctx().egui_wants_keyboard_input() {
                // Element clipboard: egui-winit turns Ctrl+C / Ctrl+X / Ctrl+V into Copy / Cut / Paste
                // events (no key_pressed for C/X/V), so listen for the events instead.
                let clip_events: Vec<egui::Event> = ui.input(|i| {
                    i.events
                        .iter()
                        .filter(|e| matches!(e, egui::Event::Copy | egui::Event::Cut | egui::Event::Paste(_)))
                        .cloned()
                        .collect()
                });
                for ev in clip_events {
                    match ev {
                        egui::Event::Copy => {
                            self.copy_selected_elements(ui.ctx());
                        }
                        egui::Event::Cut => {
                            self.cut_selected_elements(ui.ctx());
                        }
                        egui::Event::Paste(text) => {
                            self.paste_clipboard_elements(Some(&text));
                        }
                        _ => {}
                    }
                }
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

                    // If HW MCU reports fw_feature_level < 2 while the current program requires v2 opcodes, display orange warning banner
                    if is_hw {
                        let req_level = self.doc.compile_to_bytecode().required_feature_level();
                        if req_level >= 2 {
                            if let Some(ref snap) = self.latest_snapshot {
                                if !snap.firmware_supports(req_level) {
                                    let y_off = if !is_matched { 52.0 } else { 14.0 };
                                    let fw_banner_pos =
                                        egui::pos2(max_rect.center().x - 270.0, max_rect.min.y + y_off);
                                    egui::Area::new(egui::Id::new("online_fw_feature_banner"))
                                        .order(egui::Order::Foreground)
                                        .fixed_pos(fw_banner_pos)
                                        .interactable(false)
                                        .show(ui.ctx(), |ui| {
                                            egui::Frame::NONE
                                                .fill(egui::Color32::from_rgba_unmultiplied(
                                                    48, 26, 6, 245,
                                                ))
                                                .stroke(egui::Stroke::new(
                                                    1.2,
                                                    egui::Color32::from_rgb(245, 158, 11),
                                                ))
                                                .corner_radius(egui::CornerRadius::same(6))
                                                .inner_margin(egui::Margin::symmetric(12, 6))
                                                .show(ui, |ui| {
                                                    ui.label(
                                                        RichText::new(format!(
                                                            "⚠ FIRMWARE FEATURE LEVEL v{} (< v2): Program uses Phase 2 opcodes (PID/SCALE/LIMIT/BITWISE/SHIFT/UART)",
                                                            snap.fw_feature_level.max(1)
                                                        ))
                                                        .color(egui::Color32::from_rgb(251, 191, 36))
                                                        .strong()
                                                        .size(11.0),
                                                    );
                                                });
                                        });
                                }
                            }
                        }
                    }
                }
            });

        if let Some(cmd) = banner_cmd {
            if cmd == my_canvas_core::PlcSerialCommand::UploadProgram {
                self.request_upload_from_plc();
            } else {
                self.transmit_bytecode_to_plc(cmd);
            }
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
            Window::new("About LaduinoPLC")
                .order(egui::Order::Foreground)
                .open(&mut open_about)
                .collapsible(false)
                .resizable(false)
                .default_width(550.0)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(ui.ctx(), |ui| {
                    ui.spacing_mut().item_spacing.y = 8.0;

                    // 1. HERO HEADER BANNER
                    egui::Frame::new()
                        .fill(if palette.is_light() {
                            Color32::from_rgb(238, 243, 250)
                        } else {
                            Color32::from_rgb(20, 24, 33)
                        })
                        .corner_radius(egui::CornerRadius::same(10))
                        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(14, 165, 233).gamma_multiply(0.5)))
                        .inner_margin(egui::Margin::symmetric(14, 12))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let logo_tex = self.about_logo_texture.get_or_insert_with(|| {
                                    let img = image::load_from_memory(include_bytes!("../../../assets/laduinoplc_128.png")).expect("logo");
                                    let rgba = img.to_rgba8();
                                    let (w, h) = rgba.dimensions();
                                    let color_image = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], rgba.as_raw());
                                    ui.ctx().load_texture("laduinoplc_about_logo", color_image, Default::default())
                                });
                                ui.add(egui::Image::new(&*logo_tex).fit_to_exact_size(egui::vec2(62.0, 62.0)));

                                ui.add_space(10.0);

                                ui.vertical(|ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new("LaduinoPLC")
                                                .size(23.0)
                                                .strong()
                                                .color(if palette.is_light() {
                                                    Color32::from_rgb(12, 74, 110)
                                                } else {
                                                    Color32::from_rgb(56, 189, 248)
                                                }),
                                        );
                                        ui.add_space(6.0);
                                        // Version Tag Badge
                                        egui::Frame::new()
                                            .fill(if palette.is_light() {
                                                Color32::from_rgb(224, 231, 255)
                                            } else {
                                                Color32::from_rgb(30, 41, 59)
                                            })
                                            .corner_radius(egui::CornerRadius::same(5))
                                            .inner_margin(egui::Margin::symmetric(6, 2))
                                            .show(ui, |ui| {
                                                ui.label(
                                                    RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                                                        .size(11.0)
                                                        .strong()
                                                        .color(if palette.is_light() {
                                                            Color32::from_rgb(67, 56, 202)
                                                        } else {
                                                            Color32::from_rgb(129, 140, 248)
                                                        }),
                                                );
                                            });
                                    });

                                    ui.label(
                                        RichText::new("Visual Ladder Logic IDE & Microcontroller PLC Engine")
                                            .size(12.0)
                                            .color(palette.text_primary),
                                    );

                                    ui.add_space(2.0);

                                    // Tag pills
                                    ui.horizontal_wrapped(|ui| {
                                        let tags = [
                                            ("🦀 Rust 2021", Color32::from_rgb(234, 88, 12)),
                                            ("⚡ IEC 61131-3", Color32::from_rgb(14, 165, 233)),
                                            ("🔌 Arduino & RP2040", Color32::from_rgb(16, 185, 129)),
                                            ("📡 Modbus RTU", Color32::from_rgb(168, 85, 247)),
                                        ];
                                        for (tag_text, tag_color) in tags {
                                            egui::Frame::new()
                                                .fill(tag_color.gamma_multiply(0.12))
                                                .corner_radius(egui::CornerRadius::same(4))
                                                .stroke(egui::Stroke::new(1.0, tag_color.gamma_multiply(0.35)))
                                                .inner_margin(egui::Margin::symmetric(5, 1))
                                                .show(ui, |ui| {
                                                    ui.label(RichText::new(tag_text).size(10.0).strong().color(tag_color));
                                                });
                                        }
                                    });
                                });
                            });
                        });

                    // 2. MEANINGFUL EXPLANATION / CONCEPT
                    egui::Frame::new()
                        .fill(if palette.is_light() {
                            Color32::from_rgb(243, 244, 246)
                        } else {
                            palette.card_bg
                        })
                        .corner_radius(egui::CornerRadius::same(8))
                        .stroke(egui::Stroke::new(1.0, palette.card_border.gamma_multiply(0.6)))
                        .inner_margin(egui::Margin::symmetric(12, 9))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new("💡 Concept & Mission")
                                    .strong()
                                    .color(palette.accent_heading),
                            );
                            ui.add_space(3.0);
                            ui.label(
                                RichText::new("LaduinoPLC (Ladder + Arduino + PLC) bridges industrial automation standards (IEC 61131-3) with open microcontroller platforms. It empowers engineers, technicians, educators, and makers to design Ladder Diagrams, run real-time simulations, and deploy compiled bytecode onto microcontroller hardware with zero expensive proprietary license fees.")
                                    .size(11.5)
                                    .color(palette.text_primary),
                            );
                        });

                    // 3. PILLARS GRID (4 Core Architecture Pillars)
                    egui::Grid::new("about_pillars_grid")
                        .num_columns(2)
                        .spacing([10.0, 8.0])
                        .show(ui, |ui| {
                            // Column 1 - Card 1: CAD & Power Flow
                            egui::Frame::new()
                                .fill(if palette.is_light() { Color32::from_rgb(248, 250, 252) } else { palette.pill_bg })
                                .corner_radius(egui::CornerRadius::same(6))
                                .stroke(egui::Stroke::new(1.0, palette.card_border.gamma_multiply(0.4)))
                                .inner_margin(egui::Margin::symmetric(10, 8))
                                .show(ui, |ui| {
                                    ui.set_width(250.0);
                                    ui.label(RichText::new("⚡ Visual Ladder CAD").strong().color(palette.accent_heading));
                                    ui.label(RichText::new("CAD canvas with live power flow visualizer displaying real-time electrical conduction and coil energization (10ms cycle).").size(11.0).color(palette.rung_text_dim));
                                });

                            // Column 2 - Card 2: Micro-VM & Hardware
                            egui::Frame::new()
                                .fill(if palette.is_light() { Color32::from_rgb(248, 250, 252) } else { palette.pill_bg })
                                .corner_radius(egui::CornerRadius::same(6))
                                .stroke(egui::Stroke::new(1.0, palette.card_border.gamma_multiply(0.4)))
                                .inner_margin(egui::Margin::symmetric(10, 8))
                                .show(ui, |ui| {
                                    ui.set_width(250.0);
                                    ui.label(RichText::new("🔌 Embedded Micro-VM").strong().color(palette.accent_ok));
                                    ui.label(RichText::new("Compiles ladder logic into compact 32-bit bytecode uploaded to Arduino UNO/Nano/Mega and RP2040 Pico (Embassy Async).").size(11.0).color(palette.rung_text_dim));
                                });
                            ui.end_row();

                            // Column 1 - Card 3: Online Monitoring & Modbus
                            egui::Frame::new()
                                .fill(if palette.is_light() { Color32::from_rgb(248, 250, 252) } else { palette.pill_bg })
                                .corner_radius(egui::CornerRadius::same(6))
                                .stroke(egui::Stroke::new(1.0, palette.card_border.gamma_multiply(0.4)))
                                .inner_margin(egui::Margin::symmetric(10, 8))
                                .show(ui, |ui| {
                                    ui.set_width(250.0);
                                    ui.label(RichText::new("📡 Live Telemetry & Modbus").strong().color(palette.accent_warn));
                                    ui.label(RichText::new("USB online monitoring with real-time Data Watch Table, Force I/O, and integrated Modbus RTU master.").size(11.0).color(palette.rung_text_dim));
                                });

                            // Column 2 - Card 4: IEC 61131-3 & Interop
                            egui::Frame::new()
                                .fill(if palette.is_light() { Color32::from_rgb(248, 250, 252) } else { palette.pill_bg })
                                .corner_radius(egui::CornerRadius::same(6))
                                .stroke(egui::Stroke::new(1.0, palette.card_border.gamma_multiply(0.4)))
                                .inner_margin(egui::Margin::symmetric(10, 8))
                                .show(ui, |ui| {
                                    ui.set_width(250.0);
                                    ui.label(RichText::new("📐 IEC 61131-3 Interop").strong().color(Color32::from_rgb(168, 85, 247)));
                                    ui.label(RichText::new("Native Instruction List (.il) import and export fully compatible with OpenPLC and CODESYS.").size(11.0).color(palette.rung_text_dim));
                                });
                            ui.end_row();
                        });

                    // 4. HARDWARE COMPATIBILITY BADGES
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Hardware:").size(11.0).color(palette.rung_text_dim));
                        let hw_targets = [
                            "Arduino UNO R3",
                            "Arduino Nano",
                            "Arduino Mega 2560",
                            "RP2040 Pico",
                            "ESP32",
                        ];
                        for hw in hw_targets {
                            egui::Frame::new()
                                .fill(if palette.is_light() { Color32::from_rgb(230, 235, 245) } else { Color32::from_rgb(33, 37, 45) })
                                .corner_radius(egui::CornerRadius::same(4))
                                .inner_margin(egui::Margin::symmetric(5, 1))
                                .show(ui, |ui| {
                                    ui.label(RichText::new(hw).size(10.0).color(palette.text_primary));
                                });
                        }
                    });

                    ui.separator();

                    // 5. FOOTER & CLOSE BUTTON
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new("Created & Developed by PksofttecH")
                                    .size(11.0)
                                    .strong()
                                    .color(palette.rung_text_dim),
                            );
                            ui.label(
                                RichText::new("Open-Source Automation • Built with Rust & egui")
                                    .size(10.0)
                                    .color(palette.rung_text_dim.gamma_multiply(0.8)),
                            );
                        });

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let close_btn = egui::Button::new(
                                RichText::new("  Close (Esc)  ")
                                    .strong()
                                    .color(Color32::WHITE),
                            )
                            .fill(if palette.is_light() {
                                Color32::from_rgb(14, 165, 233)
                            } else {
                                Color32::from_rgb(14, 116, 170)
                            })
                            .corner_radius(egui::CornerRadius::same(6));

                            if ui.add(close_btn).clicked() {
                                close_about = true;
                            }
                        });
                    });
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
                .min_size([640.0, 480.0])
                .default_size([680.0, 540.0])
                .collapsible(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(ui.ctx(), |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("settings_hw_cfg_scroll")
                        .max_height(480.0)
                        .show(ui, |ui| {
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
                            if ui
                                .add(
                                    egui::TextEdit::singleline(&mut self.com_port)
                                        .hint_text("Auto-detect"),
                                )
                                .changed()
                            {
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
                    if self.render_rp2040_io_pin_config_ui(ui, "settings_dlg") {
                        settings_changed = true;
                    }

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

            let (bc, compile_report) =
                my_canvas_core::BytecodeCompiler::compile_program_with_report(&self.doc.program);
            let packet = bc.to_serial_packet(my_canvas_core::PlcSerialCommand::DownloadProgram);
            let crc8 = packet.last().copied().unwrap_or(0);
            let port_exists = serial::configured_port_available(&self.com_port);
            let req_feature_level = bc.required_feature_level();

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
                                            .hint_text("Auto-detect")
                                            .desired_width(125.0)
                                            .font(egui::TextStyle::Monospace),
                                    )
                                    .changed()
                                {
                                    settings_changed = true;
                                }
                                egui::ComboBox::from_id_salt("update_plc_port_combo")
                                    .width(105.0)
                                    .selected_text(if self.com_port.trim().is_empty() {
                                        "🔍 Auto-detect"
                                    } else {
                                        "🔍 Detect"
                                    })
                                    .show_ui(ui, |ui| {
                                        // Enumerate once per second while the popup is open
                                        let ports = serial::list_ports_cached(Duration::from_millis(1000));
                                        let detected = serial::choose_plc_port(&ports);
                                        let auto_label = match &detected {
                                            Some(p) => format!("Auto-detect (★ {})", p),
                                            None => "Auto-detect".to_string(),
                                        };
                                        if ui
                                            .selectable_value(&mut self.com_port, String::new(), auto_label)
                                            .changed()
                                        {
                                            settings_changed = true;
                                        }
                                        ui.separator();
                                        if let Some(auto_port) = detected {
                                            if ui
                                                .selectable_value(
                                                    &mut self.com_port,
                                                    auto_port.clone(),
                                                    format!("★ {} (LaduinoPLC VM)", auto_port),
                                                )
                                                .changed()
                                            {
                                                settings_changed = true;
                                            }
                                            ui.separator();
                                        }
                                        for p in ports.iter().map(|p| p.display()) {
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
                            RichText::new("📦 1. 32-Bit Bytecode")
                                .strong()
                                .size(11.5),
                        )
                        .selected(self.update_plc_tab == 0)
                        .corner_radius(egui::CornerRadius::same(6));
                        if ui.add(tab0_btn).clicked() {
                            self.update_plc_tab = 0;
                        }

                        let tab1_btn = egui::Button::new(
                            RichText::new("🗺 2. Memory Mapping (0x00..0xFF)")
                                .strong()
                                .size(11.5),
                        )
                        .selected(self.update_plc_tab == 1)
                        .corner_radius(egui::CornerRadius::same(6));
                        if ui.add(tab1_btn).clicked() {
                            self.update_plc_tab = 1;
                        }

                        let tab2_btn = egui::Button::new(
                            RichText::new("🔧 3. RP2040 Hardware I/O Pins")
                                .strong()
                                .size(11.5),
                        )
                        .selected(self.update_plc_tab == 2)
                        .corner_radius(egui::CornerRadius::same(6));
                        if ui.add(tab2_btn).clicked() {
                            self.update_plc_tab = 2;
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .small_button("📋 Copy Hex")
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
                            1 => {
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
                            _ => {
                                egui::ScrollArea::vertical()
                                    .id_salt("update_plc_io_cfg_scroll")
                                    .max_height(265.0)
                                    .show(ui, |ui| {
                                        if self.render_rp2040_io_pin_config_ui(ui, "update_plc") {
                                            settings_changed = true;
                                        }
                                    });
                            }
                        });

                    // 5. Styled Status & ACK Telemetry Banner
                    let status_text = if self.update_plc_status.is_empty() {
                        format!(
                            "Ready — Target Port: {} @ {} bps | Protocol: SOF 0xAA + CMD + LEN + PAYLOAD + CRC8",
                            self.com_port_display(), self.baud_rate
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

                    // 5a. Compiler Report (Errors / Warnings) & Firmware Feature-Level v2 Check
                    if compile_report.has_messages() {
                        let is_err = !compile_report.is_ok();
                        let (rep_bg, rep_border) = if is_err {
                            (
                                egui::Color32::from_rgba_unmultiplied(231, 76, 60, 28),
                                egui::Color32::from_rgb(231, 76, 60),
                            )
                        } else {
                            (
                                egui::Color32::from_rgba_unmultiplied(245, 158, 11, 28),
                                egui::Color32::from_rgb(245, 158, 11),
                            )
                        };
                        egui::Frame::NONE
                            .fill(rep_bg)
                            .stroke(egui::Stroke::new(1.0, rep_border))
                            .corner_radius(egui::CornerRadius::same(6))
                            .inner_margin(egui::Margin::symmetric(10, 6))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                for err in &compile_report.errors {
                                    ui.label(
                                        RichText::new(format!("✖ Compile Error: {}", err))
                                            .monospace()
                                            .size(11.0)
                                            .color(egui::Color32::from_rgb(255, 118, 105)),
                                    );
                                }
                                for warn in &compile_report.warnings {
                                    ui.label(
                                        RichText::new(format!("⚠ Compile Warning: {}", warn))
                                            .monospace()
                                            .size(11.0)
                                            .color(egui::Color32::from_rgb(251, 191, 36)),
                                    );
                                }
                            });
                    }

                    if req_feature_level >= 2 {
                        let hw_unsupported = self
                            .latest_snapshot
                            .as_ref()
                            .is_some_and(|s| !s.firmware_supports(req_feature_level));
                        if hw_unsupported {
                            let fw_ver = self
                                .latest_snapshot
                                .as_ref()
                                .map(|s| s.fw_feature_level.max(1))
                                .unwrap_or(1);
                            egui::Frame::NONE
                                .fill(egui::Color32::from_rgba_unmultiplied(245, 158, 11, 32))
                                .stroke(egui::Stroke::new(
                                    1.2,
                                    egui::Color32::from_rgb(245, 158, 11),
                                ))
                                .corner_radius(egui::CornerRadius::same(6))
                                .inner_margin(egui::Margin::symmetric(10, 6))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.label(
                                        RichText::new(format!(
                                            "⚠ Connected PLC firmware reports Feature Level v{} (< v2), but this program requires Feature Level v2 (PID / SCALE / LIMIT / BITWISE / SHIFT / UART / ADC).",
                                            fw_ver
                                        ))
                                        .strong()
                                        .size(11.0)
                                        .color(egui::Color32::from_rgb(251, 191, 36)),
                                    );
                                });
                        }
                    }

                    ui.add_space(3.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("💾 Flash Retention:")
                                .strong()
                                .size(11.0)
                                .color(palette.accent_heading),
                        );
                        ui.label(
                            RichText::new("RETAIN Area (M64..M95 / %MW96..%MW127 @ Sector 0x1FF000)")
                                .size(11.0)
                                .color(palette.accent_ok),
                        );

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let clear_retain_btn = egui::Button::new(
                                RichText::new("🗑 Clear RETAIN")
                                    .size(11.0)
                                    .strong()
                                    .color(palette.accent_danger),
                            )
                            .corner_radius(egui::CornerRadius::same(5));
                            if ui
                                .add(clear_retain_btn)
                                .on_hover_text(
                                    "Zero M64..M95 and D96..D127 in RAM and erase the RETAIN Flash sector (CMD 0x09)",
                                )
                                .clicked()
                            {
                                self.confirm_retain_action =
                                    Some(my_canvas_core::PlcSerialCommand::PersistClear);
                            }

                            let save_retain_btn = egui::Button::new(
                                RichText::new("💾 Save RETAIN to Flash")
                                    .size(11.0)
                                    .strong()
                                    .color(palette.accent_ok),
                            )
                            .corner_radius(egui::CornerRadius::same(5));
                            if ui
                                .add(save_retain_btn)
                                .on_hover_text(
                                    "Save current M64..M95 and D96..D127 values to non-volatile Flash sector 0x1FF000 (CMD 0x08)",
                                )
                                .clicked()
                            {
                                self.confirm_retain_action =
                                    Some(my_canvas_core::PlcSerialCommand::PersistSave);
                            }
                        });
                    });

                    if let Some(pending_cmd) = self.confirm_retain_action {
                        let is_clear =
                            pending_cmd == my_canvas_core::PlcSerialCommand::PersistClear;
                        let prompt_txt = if is_clear {
                            "⚠ Confirm Clear RETAIN? This zeroes M64..M95 & %MW96..%MW127 in RAM and erases Flash sector 0x1FF000 (CMD 0x09)."
                        } else {
                            "💾 Confirm Save RETAIN to Flash? This writes current M64..M95 & %MW96..%MW127 to Flash sector 0x1FF000 (CMD 0x08)."
                        };
                        egui::Frame::NONE
                            .fill(egui::Color32::from_rgba_unmultiplied(245, 158, 11, 28))
                            .stroke(egui::Stroke::new(
                                1.0,
                                if is_clear {
                                    palette.accent_danger
                                } else {
                                    palette.accent_warn
                                },
                            ))
                            .corner_radius(egui::CornerRadius::same(6))
                            .inner_margin(egui::Margin::symmetric(10, 6))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(prompt_txt)
                                            .size(11.0)
                                            .strong()
                                            .color(palette.text_primary),
                                    );
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui.small_button("Cancel").clicked() {
                                                self.confirm_retain_action = None;
                                            }
                                            let confirm_btn = egui::Button::new(
                                                RichText::new("✔ Confirm")
                                                    .size(11.0)
                                                    .strong()
                                                    .color(egui::Color32::WHITE),
                                            )
                                            .fill(if is_clear {
                                                egui::Color32::from_rgb(192, 57, 43)
                                            } else {
                                                egui::Color32::from_rgb(24, 130, 90)
                                            });
                                            if ui.add(confirm_btn).clicked() {
                                                trigger_cmd = Some(pending_cmd);
                                                self.confirm_retain_action = None;
                                            }
                                        },
                                    );
                                });
                            });
                    }

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

                        let upload_hover = if compile_report.is_ok() {
                            "Transmit 32-Bit Bytecode Packet Frame (CMD 0x01) via USB Serial and auto-start PLC execution"
                        } else {
                            "Upload disabled: resolve compile errors above before downloading to PLC"
                        };

                        if ui
                            .add_enabled(compile_report.is_ok(), upload_btn)
                            .on_hover_text(upload_hover)
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

        // Software Update & Release Manager modal
        crate::updater::render_update_dialog(&ui.ctx().clone(), self);

        // MCU Firmware Manager & USB Flasher modal
        crate::firmware_flasher::render_firmware_flasher_dialog(&ui.ctx().clone(), self);

        // Floating (non-modal) Find / Replace window
        find_dialog::render(&ui.ctx().clone(), self);
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
                    recent_files: vec!["/tmp/projects/a.il".to_string()],
                    update_use_custom_server: false,
                    update_repo_owner: default_update_repo_owner(),
                    update_repo_name: default_update_repo_name(),
                    update_custom_manifest_url: String::new(),
                    rp2040_input_pins: default_rp2040_input_pins(),
                    rp2040_output_pins: default_rp2040_output_pins(),
                    rp2040_status_led_pin: default_rp2040_status_led_pin(),
                    rp2040_io_flags: default_rp2040_io_flags(),
                    rp2040_rs485_de_re_pin: default_rp2040_rs485_de_re_pin(),
                    rp2040_rs485_config: default_rp2040_rs485_config(),
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
    fn test_app_settings_default_com_port_is_auto() {
        let s = AppSettings::default();
        assert!(s.com_port.is_empty());
        assert_eq!(serial::normalize_port_name(&s.com_port), None);
        assert!(AppSettings::config_path().ends_with("settings.json"));
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

        // 4. Dirty: request_import_il and request_upload_from_plc set PendingAction
        app.doc.is_dirty = true;
        app.request_import_il();
        assert_eq!(app.pending_action, Some(PendingAction::ImportIl));
        app.pending_action = None;

        app.request_upload_from_plc();
        assert_eq!(app.pending_action, Some(PendingAction::UploadFromPlc));
        app.pending_action = None;

        // 5. Dirty: request_exit sets PendingAction::ExitApp
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
    fn test_example_demo_file_save_and_exit_handling() {
        let mut app = CanvasApp::default();
        let ctx = egui::Context::default();

        // 1. Initial state
        assert!(!app.is_example_file());

        // 2. Load demo
        app.load_example_file("example/08_rp2040_demo_led_blink.il");
        assert!(app.is_example_file());
        assert!(app.is_example_template);
        // Ensure last_folder was not set to "example"
        assert_ne!(app.last_folder.as_ref().and_then(|p| p.to_str()), Some("example"));

        // 3. Not dirty immediately after loading demo
        assert!(!app.doc.is_dirty);

        // 4. User modifies demo (e.g. inserting an element)
        let new_id = app.doc.program.alloc_id();
        let new_elem = my_canvas_core::LadderElement::new_contact(
            new_id,
            "%IX0.0",
            "TEST_IN",
            my_canvas_core::ContactMode::NormallyOpen,
        );
        app.doc.insert_element(new_elem, my_canvas_core::InsertPosition::AppendRung);
        assert!(app.doc.is_dirty);

        // 5. Exit requested while dirty -> intercepted with pending action ExitApp
        app.request_exit(&ctx);
        assert_eq!(app.pending_action, Some(PendingAction::ExitApp));
        assert!(!app.allow_exit);

        // 6. If user cancels save, pending_action is cleared so UI is not trapped
        app.pending_action = None;
        assert!(app.pending_action.is_none());
        assert!(!app.allow_exit);

        // 7. If user chooses Don't Save / Discard, exit proceeds
        app.pending_action = Some(PendingAction::ExitApp);
        app.doc.is_dirty = false;
        app.execute_pending_action(&ctx);
        assert!(app.pending_action.is_none());
        assert!(app.allow_exit);

        // 8. Creating new program resets example template flag
        app.new_program();
        assert!(!app.is_example_file());
        assert!(!app.is_example_template);
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
    #[test]
    fn test_recent_files_serde_and_legacy_settings() {
        // Old settings file without `recent_files` still loads
        let legacy_json = r#"{ "theme": "Dark", "com_port": "ttyUSB0", "baud_rate": "115200" }"#;
        let loaded: AppSettings = serde_json::from_str(legacy_json).unwrap();
        assert!(loaded.recent_files.is_empty());

        let mut s = AppSettings::default();
        for i in 0..12 {
            push_recent_file(&mut s.recent_files, &format!("/tmp/p{}.il", i));
        }
        push_recent_file(&mut s.recent_files, "/tmp/p5.il");
        assert_eq!(s.recent_files.len(), MAX_RECENT_FILES);
        assert_eq!(s.recent_files[0], "/tmp/p5.il");
        assert_eq!(s.recent_files.iter().filter(|p| *p == "/tmp/p5.il").count(), 1);
        assert_eq!(s.recent_files[1], "/tmp/p11.il");

        let json = serde_json::to_string(&s).unwrap();
        let back: AppSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(back.recent_files, s.recent_files);
    }

    #[test]
    fn test_default_starts_with_new_empty_ladder() {
        let app = CanvasApp::default();
        assert_eq!(app.doc.program.rungs.len(), 1);
        assert!(app.doc.program.rungs[0].elements.is_empty());
        assert!(app.doc.file_path.is_none());
        assert!(!app.doc.is_dirty);
    }

    #[test]
    fn test_open_recent_pending_action_when_dirty() {
        let mut app = CanvasApp::default();
        app.doc.is_dirty = true;
        app.request_open_recent("/tmp/does_not_exist.il".to_string());
        assert_eq!(
            app.pending_action,
            Some(PendingAction::OpenRecent("/tmp/does_not_exist.il".to_string()))
        );
    }

    fn all_element_ids(doc: &Document) -> Vec<u64> {
        fn walk(el: &my_canvas_core::LadderElement, out: &mut Vec<u64>) {
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

    fn sample_app() -> CanvasApp {
        let mut app = CanvasApp::default();
        app.doc = Document::new_empty();
        let a = app.doc.program.alloc_id();
        let b = app.doc.program.alloc_id();
        app.doc.program.rungs[0].elements.push(my_canvas_core::LadderElement::new_contact(
            a,
            "%IX0.0",
            "START",
            my_canvas_core::ContactMode::NormallyOpen,
        ));
        app.doc.program.rungs[0].elements.push(my_canvas_core::LadderElement::new_coil(
            b,
            "%QX0.0",
            "MOTOR",
            my_canvas_core::CoilMode::Normal,
        ));
        app
    }

    #[test]
    fn test_copy_paste_elements_unique_ids_and_undo() {
        let mut app = sample_app();
        let ctx = egui::Context::default();
        let first = app.doc.program.rungs[0].elements[0].id;
        app.doc.set_single_selection(Some(0), first);
        assert!(app.copy_selected_elements(&ctx));
        assert_eq!(app.clipboard.len(), 1);

        // Internal clipboard paste
        let undo_before = app.doc.undo_stack.len();
        let ids1 = app.paste_clipboard_elements(None);
        assert_eq!(ids1.len(), 1);
        assert_eq!(app.doc.undo_stack.len(), undo_before + 1);

        // System clipboard paste (LADUINO-ELEM: JSON)
        let text = format!(
            "{}{}",
            CLIPBOARD_ELEM_PREFIX,
            serde_json::to_string(&app.clipboard).unwrap()
        );
        assert_eq!(parse_clipboard_elements(&text).map(|v| v.len()), Some(1));
        assert!(parse_clipboard_elements("plain text").is_none());
        let ids2 = app.paste_clipboard_elements(Some(&text));
        assert_eq!(ids2.len(), 1);

        let ids = all_element_ids(&app.doc);
        let mut dedup = ids.clone();
        dedup.sort();
        dedup.dedup();
        assert_eq!(ids.len(), dedup.len(), "duplicate ids: {:?}", ids);
        assert_eq!(app.doc.program.rungs[0].elements.len(), 4);

        assert!(app.doc.undo());
        assert!(app.doc.undo());
        assert_eq!(app.doc.program.rungs[0].elements.len(), 2);

        // Cut removes the element, paste brings it back with a new id
        app.doc.set_single_selection(Some(0), first);
        assert!(app.cut_selected_elements(&ctx));
        assert_eq!(app.doc.program.rungs[0].elements.len(), 1);
        let ids3 = app.paste_clipboard_elements(None);
        assert_ne!(ids3[0], first);
    }

    #[test]
    fn test_copy_network_stores_snapshot() {
        let mut app = sample_app();
        app.doc.selected_rung_idx = Some(0);
        app.copy_selected_network();
        app.doc.program.rungs[0].elements.clear();
        app.paste_network_below();
        assert_eq!(app.doc.program.rungs.len(), 2);
        assert_eq!(app.doc.program.rungs[1].elements.len(), 2);
    }

    #[test]
    fn test_find_and_replace_all() {
        let mut app = sample_app();
        app.find.query = "%QX0.0".to_string();
        app.find_step(1);
        assert_eq!(app.find.results.len(), 1);
        let coil_id = app.doc.program.rungs[0].elements[1].id;
        assert_eq!(app.doc.selected_element_id, Some(coil_id));

        app.find.query = "x0.".to_string();
        app.find.refresh(&app.doc);
        assert_eq!(app.find.results.len(), 2);
        app.find.opts.match_case = true;
        app.find.refresh(&app.doc);
        assert!(app.find.results.is_empty());

        let undo_before = app.doc.undo_stack.len();
        let n = app.doc.replace_all(
            "START",
            "RUN_PB",
            my_canvas_core::document::FindOptions { match_case: true, whole_word: true },
        );
        assert_eq!(n, 1);
        assert_eq!(app.doc.undo_stack.len(), undo_before + 1);
        assert_eq!(app.doc.program.rungs[0].elements[0].symbol, "RUN_PB");
        app.doc.undo();
        assert_eq!(app.doc.program.rungs[0].elements[0].symbol, "START");
    }
}
