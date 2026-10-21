use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};
use egui::{Color32, CornerRadius, RichText, Stroke, Window};
use crate::app::CanvasApp;

/// Messages sent from the background build/flash thread to the UI.
#[derive(Debug, Clone)]
pub enum FlashWorkerMsg {
    LogLine(String),
    Finished {
        success: bool,
        summary: String,
        generated_fw_path: Option<PathBuf>,
    },
}

pub struct FirmwareFlasher {
    pub show_window: bool,
    pub firmware_path: String,
    pub bootsel_drive_path: Option<PathBuf>,
    pub custom_bootsel_dir: String,
    pub is_busy: bool,
    pub status_summary: String,
    pub status_is_error: bool,
    pub console_log: String,
    last_drive_scan: Option<Instant>,
    rx: Option<Receiver<FlashWorkerMsg>>,
}

impl Default for FirmwareFlasher {
    fn default() -> Self {
        let default_fw = Self::find_default_firmware_path()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        Self {
            show_window: false,
            firmware_path: default_fw,
            bootsel_drive_path: None,
            custom_bootsel_dir: String::new(),
            is_busy: false,
            status_summary: "Ready — Connect RP2040 via USB to flash firmware.".to_string(),
            status_is_error: false,
            console_log: String::new(),
            last_drive_scan: None,
            rx: None,
        }
    }
}

impl FirmwareFlasher {
    /// Locate the `firmware/rp2040-embassy` directory relative to the current working directory or executable.
    pub fn find_firmware_project_dir() -> Option<PathBuf> {
        let rel = Path::new("firmware/rp2040-embassy");
        if rel.join("Cargo.toml").exists() {
            return std::fs::canonicalize(rel).ok().or_else(|| Some(rel.to_path_buf()));
        }
        if let Ok(exe) = std::env::current_exe() {
            let mut cur = exe.parent();
            while let Some(dir) = cur {
                let candidate = dir.join(rel);
                if candidate.join("Cargo.toml").exists() {
                    return Some(candidate);
                }
                cur = dir.parent();
            }
        }
        None
    }

    /// Find a pre-built `.uf2` or `.elf` firmware image in the application directory or workspace.
    pub fn find_default_firmware_path() -> Option<PathBuf> {
        // 1. Check alongside executable and `firmware/` subfolder for bundled product `.uf2`
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                for rel in [
                    "firmware/uplc-rp2040-embassy.uf2",
                    "uplc-rp2040-embassy.uf2",
                    "firmware.uf2",
                ] {
                    let p = dir.join(rel);
                    if p.exists() {
                        return Some(p);
                    }
                }
            }
        }

        // 2. Check workspace `firmware/uplc-rp2040-embassy.uf2`
        let root_uf2 = Path::new("firmware/uplc-rp2040-embassy.uf2");
        if root_uf2.exists() {
            return std::fs::canonicalize(root_uf2)
                .ok()
                .or_else(|| Some(root_uf2.to_path_buf()));
        }

        // 3. Check workspace `firmware/rp2040-embassy/target/thumbv6m-none-eabi/release/`
        if let Some(proj_dir) = Self::find_firmware_project_dir() {
            let rel_dir = proj_dir
                .join("target")
                .join("thumbv6m-none-eabi")
                .join("release");
            let uf2 = rel_dir.join("uplc-rp2040-embassy.uf2");
            if uf2.exists() {
                return Some(uf2);
            }
            let elf = rel_dir.join("uplc-rp2040-embassy");
            if elf.exists() {
                return Some(elf);
            }
        }
        None
    }

    /// Verify if a directory is an RP2040 USB BOOTSEL Mass Storage volume (`RPI-RP2` containing `INFO_UF2.TXT`).
    pub fn is_bootsel_drive(dir: &Path) -> bool {
        dir.is_dir() && (dir.join("INFO_UF2.TXT").exists() || dir.join("INDEX.HTM").exists())
    }

    /// Automatically detect a mounted `RPI-RP2` USB BOOTSEL drive across Linux, Windows, and macOS.
    pub fn detect_bootsel_drive(custom_dir: &str) -> Option<PathBuf> {
        let trimmed = custom_dir.trim();
        if !trimmed.is_empty() {
            let p = PathBuf::from(trimmed);
            if Self::is_bootsel_drive(&p) {
                return Some(p);
            }
        }

        // Windows drive letters D:\ .. Z:\
        #[cfg(windows)]
        {
            for letter in b'D'..=b'Z' {
                let drive = PathBuf::from(format!("{}:\\", letter as char));
                if Self::is_bootsel_drive(&drive) {
                    return Some(drive);
                }
            }
        }

        // macOS `/Volumes/RPI-RP2`
        #[cfg(target_os = "macos")]
        {
            let vol = PathBuf::from("/Volumes/RPI-RP2");
            if Self::is_bootsel_drive(&vol) {
                return Some(vol);
            }
        }

        // Linux `/run/media/$USER/RPI-RP2`, `/media/$USER/RPI-RP2`, `/media/RPI-RP2`, `/mnt/RPI-RP2`
        let mut candidates = vec![
            PathBuf::from("/media/RPI-RP2"),
            PathBuf::from("/mnt/RPI-RP2"),
            PathBuf::from("/run/media/RPI-RP2"),
        ];
        if let Ok(user) = std::env::var("USER") {
            candidates.insert(0, PathBuf::from(format!("/run/media/{}/RPI-RP2", user)));
            candidates.insert(1, PathBuf::from(format!("/media/{}/RPI-RP2", user)));
        }
        // Also scan children of `/run/media` and `/media` in case the volume has a suffix like `RPI-RP21`
        for base in ["/run/media", "/media"] {
            if let Ok(entries) = std::fs::read_dir(base) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("RPI-RP2")) {
                        candidates.push(path.clone());
                    }
                    if let Ok(sub_entries) = std::fs::read_dir(&path) {
                        for sub in sub_entries.flatten() {
                            let sp = sub.path();
                            if sp.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("RPI-RP2")) {
                                candidates.push(sp);
                            }
                        }
                    }
                }
            }
        }

        candidates.into_iter().find(|p| Self::is_bootsel_drive(p))
    }

    /// Poll background worker logs and status updates.
    pub fn poll(&mut self) {
        if let Some(rx) = &self.rx {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    FlashWorkerMsg::LogLine(line) => {
                        if !self.console_log.is_empty() {
                            self.console_log.push('\n');
                        }
                        self.console_log.push_str(&line);
                    }
                    FlashWorkerMsg::Finished {
                        success,
                        summary,
                        generated_fw_path,
                    } => {
                        self.is_busy = false;
                        self.status_is_error = !success;
                        self.status_summary = summary;
                        if let Some(p) = generated_fw_path {
                            self.firmware_path = p.display().to_string();
                        }
                    }
                }
            }
        }

        // Periodically refresh BOOTSEL drive detection while the window is open
        if self.show_window {
            let should_scan = self
                .last_drive_scan
                .map_or(true, |t| t.elapsed() >= Duration::from_millis(800));
            if should_scan {
                self.bootsel_drive_path = Self::detect_bootsel_drive(&self.custom_bootsel_dir);
                self.last_drive_scan = Some(Instant::now());
            }
        }
    }

    pub fn append_log(&mut self, line: impl AsRef<str>) {
        if !self.console_log.is_empty() {
            self.console_log.push('\n');
        }
        self.console_log.push_str(line.as_ref());
    }

    /// Build `firmware/rp2040-embassy` from source (`cargo build --release`).
    pub fn start_build_firmware(&mut self, ctx: egui::Context) {
        let Some(proj_dir) = Self::find_firmware_project_dir() else {
            self.status_is_error = true;
            self.status_summary = "✖ Could not find 'firmware/rp2040-embassy' directory.".to_string();
            return;
        };

        let (tx, rx): (Sender<FlashWorkerMsg>, Receiver<FlashWorkerMsg>) = mpsc::channel();
        self.rx = Some(rx);
        self.is_busy = true;
        self.status_is_error = false;
        self.status_summary = "🔨 Building RP2040 firmware (cargo build --release)...".to_string();
        self.console_log.clear();
        self.append_log(format!("$ cd {} && cargo build --release", proj_dir.display()));

        std::thread::spawn(move || {
            let child = Command::new("cargo")
                .arg("build")
                .arg("--release")
                .current_dir(&proj_dir)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn();

            match child {
                Ok(mut proc) => {
                    if let Some(stderr) = proc.stderr.take() {
                        let tx_err = tx.clone();
                        let ctx_err = ctx.clone();
                        std::thread::spawn(move || {
                            let reader = BufReader::new(stderr);
                            for line in reader.lines().map_while(Result::ok) {
                                let _ = tx_err.send(FlashWorkerMsg::LogLine(line));
                                ctx_err.request_repaint();
                            }
                        });
                    }
                    if let Some(stdout) = proc.stdout.take() {
                        let reader = BufReader::new(stdout);
                        for line in reader.lines().map_while(Result::ok) {
                            let _ = tx.send(FlashWorkerMsg::LogLine(line));
                            ctx.request_repaint();
                        }
                    }

                    match proc.wait() {
                        Ok(status) if status.success() => {
                            let elf_path = proj_dir
                                .join("target")
                                .join("thumbv6m-none-eabi")
                                .join("release")
                                .join("uplc-rp2040-embassy");
                            let uf2_path = proj_dir
                                .join("target")
                                .join("thumbv6m-none-eabi")
                                .join("release")
                                .join("uplc-rp2040-embassy.uf2");

                            // Try optional conversion to .uf2 if elf2uf2-rs is installed
                            let _ = Command::new("elf2uf2-rs")
                                .arg(&elf_path)
                                .arg(&uf2_path)
                                .output();

                            let final_path = if uf2_path.exists() {
                                let _ = tx.send(FlashWorkerMsg::LogLine(format!(
                                    "✔ Generated UF2 image: {}",
                                    uf2_path.display()
                                )));
                                uf2_path
                            } else {
                                let _ = tx.send(FlashWorkerMsg::LogLine(format!(
                                    "✔ Built ELF image: {}",
                                    elf_path.display()
                                )));
                                elf_path
                            };

                            let _ = tx.send(FlashWorkerMsg::Finished {
                                success: true,
                                summary: format!("✔ Firmware build succeeded ({})", final_path.display()),
                                generated_fw_path: Some(final_path),
                            });
                        }
                        Ok(status) => {
                            let _ = tx.send(FlashWorkerMsg::Finished {
                                success: false,
                                summary: format!("✖ Firmware build failed (exit code {:?})", status.code()),
                                generated_fw_path: None,
                            });
                        }
                        Err(e) => {
                            let _ = tx.send(FlashWorkerMsg::Finished {
                                success: false,
                                summary: format!("✖ Failed waiting on cargo build: {}", e),
                                generated_fw_path: None,
                            });
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(FlashWorkerMsg::Finished {
                        success: false,
                        summary: format!("✖ Could not start cargo build: {}", e),
                        generated_fw_path: None,
                    });
                }
            }
            ctx.request_repaint();
        });
    }

    /// Flash firmware directly to the mounted `RPI-RP2` USB BOOTSEL drive (or via `elf2uf2-rs` / `picotool`).
    pub fn start_flash_usb(&mut self, ctx: egui::Context) {
        let fw_path = PathBuf::from(self.firmware_path.trim());
        if !fw_path.exists() {
            self.status_is_error = true;
            self.status_summary = format!("✖ Firmware file not found: {}", fw_path.display());
            return;
        }

        let bootsel_drive = Self::detect_bootsel_drive(&self.custom_bootsel_dir);
        let (tx, rx): (Sender<FlashWorkerMsg>, Receiver<FlashWorkerMsg>) = mpsc::channel();
        self.rx = Some(rx);
        self.is_busy = true;
        self.status_is_error = false;
        self.status_summary = "🔥 Flashing firmware to RP2040 via USB...".to_string();
        self.console_log.clear();

        std::thread::spawn(move || {
            let is_uf2 = fw_path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("uf2"));

            // Case 1: Direct `.uf2` file copy to mounted `RPI-RP2` drive (zero external dependencies!)
            if is_uf2 {
                if let Some(ref drive) = bootsel_drive {
                    let dest = drive.join("uplc_firmware.uf2");
                    let _ = tx.send(FlashWorkerMsg::LogLine(format!(
                        "Copying UF2 image '{}' -> '{}'...",
                        fw_path.display(),
                        dest.display()
                    )));
                    match std::fs::copy(&fw_path, &dest) {
                        Ok(bytes) => {
                            let _ = tx.send(FlashWorkerMsg::LogLine(format!(
                                "✔ Transferred {} bytes to RPI-RP2. MCU is rebooting into PLC mode.",
                                bytes
                            )));
                            let _ = tx.send(FlashWorkerMsg::Finished {
                                success: true,
                                summary: format!(
                                    "✔ Flashed {} bytes to {} — RP2040 is rebooting!",
                                    bytes,
                                    drive.display()
                                ),
                                generated_fw_path: None,
                            });
                            ctx.request_repaint();
                            return;
                        }
                        Err(e) => {
                            let _ = tx.send(FlashWorkerMsg::LogLine(format!(
                                "⚠ Direct UF2 copy failed ({}), trying USB CLI tools...",
                                e
                            )));
                        }
                    }
                }
            }

            // Case 2: Try `elf2uf2-rs -d <fw_path>` or `picotool load -x <fw_path>`
            let tools: &[(&str, &[&str])] = if is_uf2 {
                &[("picotool", &["load", "-x"])]
            } else {
                &[
                    ("elf2uf2-rs", &["-d", "-s"]),
                    ("picotool", &["load", "-x"]),
                ]
            };

            for &(bin, args) in tools {
                let _ = tx.send(FlashWorkerMsg::LogLine(format!(
                    "$ {} {} {}",
                    bin,
                    args.join(" "),
                    fw_path.display()
                )));
                if let Ok(output) = Command::new(bin).args(args).arg(&fw_path).output() {
                    let out_str = String::from_utf8_lossy(&output.stdout);
                    let err_str = String::from_utf8_lossy(&output.stderr);
                    for l in out_str.lines().chain(err_str.lines()) {
                        if !l.trim().is_empty() {
                            let _ = tx.send(FlashWorkerMsg::LogLine(l.to_string()));
                        }
                    }
                    if output.status.success() {
                        let _ = tx.send(FlashWorkerMsg::Finished {
                            success: true,
                            summary: format!("✔ Firmware flashed successfully via `{}`!", bin),
                            generated_fw_path: None,
                        });
                        ctx.request_repaint();
                        return;
                    }
                }
            }

            let hint = if bootsel_drive.is_none() {
                "RP2040 BOOTSEL drive (RPI-RP2) not detected. Click '🔄 Reboot PLC to BOOTSEL' or hold BOOTSEL while plugging in USB."
            } else {
                "Select a .uf2 file for direct USB copy, or install `elf2uf2-rs` / `picotool` for ELF files."
            };
            let _ = tx.send(FlashWorkerMsg::Finished {
                success: false,
                summary: format!("✖ Flash failed: {}", hint),
                generated_fw_path: None,
            });
            ctx.request_repaint();
        });
    }
}

/// Render the USB MCU Firmware Flasher modal window.
pub fn render_firmware_flasher_dialog(ctx: &egui::Context, app: &mut CanvasApp) {
    if !app.firmware_flasher.show_window {
        return;
    }

    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && !app.firmware_flasher.is_busy {
        app.firmware_flasher.show_window = false;
        return;
    }

    let palette = app.theme.palette();
    let mut open_win = app.firmware_flasher.show_window;
    let mut close_win = false;
    let mut trigger_enter_bootsel = false;
    let mut trigger_flash = false;
    let mut trigger_rescan = false;

    let detected_plc_port = CanvasApp::detect_uplc_usb_port();
    let detected_bootsel = app.firmware_flasher.bootsel_drive_path.clone();

    Window::new("🔥 MCU Firmware Manager & USB Flasher (RP2040)")
        .order(egui::Order::Foreground)
        .open(&mut open_win)
        .min_size([560.0, 440.0])
        .max_width(620.0)
        .collapsible(false)
        .resizable(true)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            // 1. Target USB Device Status Card (PLC Serial Mode vs USB BOOTSEL Drive Mode)
            egui::Frame::new()
                .fill(if palette.is_light() {
                    Color32::from_rgb(241, 245, 249)
                } else {
                    palette.card_bg
                })
                .stroke(Stroke::new(1.0, palette.card_border))
                .corner_radius(CornerRadius::same(6))
                .inner_margin(egui::Margin::symmetric(12, 10))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("🔌 USB Target Device Status")
                                .size(13.0)
                                .strong()
                                .color(palette.accent_heading),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("🔄 Rescan USB").clicked() {
                                trigger_rescan = true;
                            }
                        });
                    });

                    ui.separator();

                    // Row A: PLC USB-CDC Serial Port
                    ui.horizontal(|ui| {
                        if let Some(ref port) = detected_plc_port {
                            ui.label(
                                RichText::new(format!("🟢 PLC Serial Port: {} (Running Micro-VM)", port))
                                    .monospace()
                                    .size(11.5)
                                    .strong()
                                    .color(palette.accent_ok),
                            );
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let bootsel_btn = egui::Button::new(
                                    RichText::new("⚡ Reboot PLC to BOOTSEL Mode")
                                        .size(11.0)
                                        .strong()
                                        .color(Color32::WHITE),
                                )
                                .fill(Color32::from_rgb(217, 119, 6));

                                if ui
                                    .add_enabled(!app.firmware_flasher.is_busy, bootsel_btn)
                                    .on_hover_text(
                                        "Send USB command (0x0A) to safely save RETAIN variables and reboot RP2040 into RPI-RP2 BOOTSEL drive mode without pressing the physical button",
                                    )
                                    .clicked()
                                {
                                    trigger_enter_bootsel = true;
                                }
                            });
                        } else {
                            ui.label(
                                RichText::new("⚪ PLC Serial Port: Not detected (or currently in BOOTSEL mode)")
                                    .size(11.5)
                                    .color(palette.rung_text_dim),
                            );
                        }
                    });

                    ui.add_space(4.0);

                    // Row B: USB BOOTSEL Mass Storage Drive (`RPI-RP2`)
                    ui.horizontal(|ui| {
                        if let Some(ref drive) = detected_bootsel {
                            ui.label(
                                RichText::new(format!(
                                    "🟡 USB BOOTSEL Drive Ready: {}",
                                    drive.display()
                                ))
                                .monospace()
                                .size(11.5)
                                .strong()
                                .color(Color32::from_rgb(245, 158, 11)),
                            );
                        } else {
                            ui.label(
                                RichText::new(
                                    "⚪ USB BOOTSEL Drive (RPI-RP2): Waiting for BOOTSEL mode...",
                                )
                                .size(11.5)
                                .color(palette.rung_text_dim),
                            );
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .small_button("📂 Locate RPI-RP2...")
                                .on_hover_text("Manually select the mounted RPI-RP2 drive folder")
                                .clicked()
                            {
                                if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                                    app.firmware_flasher.custom_bootsel_dir =
                                        folder.display().to_string();
                                    trigger_rescan = true;
                                }
                            }
                        });
                    });
                });

            ui.add_space(6.0);

            // 2. Firmware Image Selection Card
            ui.group(|ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("📦 Firmware Binary (.uf2 / .elf)")
                            .size(12.5)
                            .strong()
                            .color(palette.accent_heading),
                    );
                    let fw_p = Path::new(app.firmware_flasher.firmware_path.trim());
                    if fw_p.exists() {
                        if let Ok(meta) = std::fs::metadata(fw_p) {
                            let kb = (meta.len() as f64) / 1024.0;
                            ui.label(
                                RichText::new(format!("({:.1} KB)", kb))
                                    .monospace()
                                    .size(11.0)
                                    .color(palette.accent_ok),
                            );
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("📂 Browse...").clicked() {
                            let mut dlg = rfd::FileDialog::new()
                                .add_filter("RP2040 Firmware (*.uf2, *.elf, *)", &["uf2", "elf", "bin"]);
                            if let Some(proj) = FirmwareFlasher::find_firmware_project_dir() {
                                dlg = dlg.set_directory(proj);
                            }
                            if let Some(file) = dlg.pick_file() {
                                app.firmware_flasher.firmware_path = file.display().to_string();
                            }
                        }
                        if ui
                            .small_button("📦 Use Bundled Firmware")
                            .on_hover_text("Select the official pre-built RP2040 firmware bundled with uPLC Studio")
                            .clicked()
                        {
                            if let Some(default_p) = FirmwareFlasher::find_default_firmware_path() {
                                app.firmware_flasher.firmware_path = default_p.display().to_string();
                            }
                        }
                    });
                });

                ui.add(
                    egui::TextEdit::singleline(&mut app.firmware_flasher.firmware_path)
                        .font(egui::TextStyle::Monospace)
                        .hint_text("Path to uplc-rp2040-embassy.uf2 or ELF binary...")
                        .desired_width(ui.available_width()),
                );

                ui.label(
                    RichText::new(
                        "ℹ Flashing firmware updates Sector 0.. (Micro-VM engine) while preserving Ladder Bytecode (Sector 510) & RETAIN memory (Sector 511).",
                    )
                    .size(10.5)
                    .color(palette.rung_text_dim),
                );
            });

            ui.add_space(6.0);

            // 3. Status Banner & Console Output Log
            egui::Frame::new()
                .fill(if app.firmware_flasher.status_is_error {
                    Color32::from_rgba_unmultiplied(180, 40, 40, 28)
                } else {
                    palette.pill_bg
                })
                .stroke(Stroke::new(
                    1.0,
                    if app.firmware_flasher.status_is_error {
                        palette.accent_danger
                    } else {
                        palette.card_border
                    },
                ))
                .corner_radius(CornerRadius::same(5))
                .inner_margin(egui::Margin::symmetric(10, 6))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        if app.firmware_flasher.is_busy {
                            ui.spinner();
                        }
                        ui.label(
                            RichText::new(&app.firmware_flasher.status_summary)
                                .size(11.5)
                                .strong()
                                .color(if app.firmware_flasher.status_is_error {
                                    palette.accent_danger
                                } else {
                                    palette.text_primary
                                }),
                        );
                    });
                });

            ui.add_space(4.0);

            egui::ScrollArea::vertical()
                .id_salt("fw_flasher_console_scroll")
                .max_height(150.0)
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    let mut log_view = if app.firmware_flasher.console_log.is_empty() {
                        String::from(
                            "Steps to Flash Firmware via USB:\n\
                             1. Click '⚡ Reboot PLC to BOOTSEL Mode' (or hold BOOTSEL on the Pico while plugging in USB).\n\
                             2. Verify 'USB BOOTSEL Drive Ready (RPI-RP2)' appears above.\n\
                             3. Click '🔥 Flash Firmware via USB' below.",
                        )
                    } else {
                        app.firmware_flasher.console_log.clone()
                    };
                    ui.add(
                        egui::TextEdit::multiline(&mut log_view)
                            .font(egui::TextStyle::Monospace)
                            .desired_width(ui.available_width())
                            .desired_rows(6)
                            .interactive(false),
                    );
                });

            ui.add_space(8.0);

            // 4. Bottom Action Bar (Product One-Click USB Flash)
            ui.horizontal(|ui| {
                let busy = app.firmware_flasher.is_busy;
                let has_fw_file = !app.firmware_flasher.firmware_path.trim().is_empty();

                let flash_btn = egui::Button::new(
                    RichText::new("🔥 Flash Firmware via USB")
                        .size(12.5)
                        .strong()
                        .color(Color32::WHITE),
                )
                .fill(Color32::from_rgb(24, 130, 90))
                .corner_radius(egui::CornerRadius::same(6))
                .min_size(egui::vec2(210.0, 32.0));

                if ui
                    .add_enabled(!busy && has_fw_file, flash_btn)
                    .on_hover_text("Copy .uf2 directly to RPI-RP2 USB BOOTSEL drive")
                    .clicked()
                {
                    trigger_flash = true;
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let close_btn = egui::Button::new(RichText::new("Close").size(12.0))
                        .corner_radius(CornerRadius::same(6))
                        .min_size(egui::vec2(76.0, 32.0));
                    if ui.add_enabled(!busy, close_btn).clicked() {
                        close_win = true;
                    }
                });
            });
        });

    if trigger_rescan {
        app.firmware_flasher.bootsel_drive_path =
            FirmwareFlasher::detect_bootsel_drive(&app.firmware_flasher.custom_bootsel_dir);
        app.firmware_flasher.last_drive_scan = Some(Instant::now());
    }
    if trigger_enter_bootsel {
        app.transmit_bytecode_to_plc(my_canvas_core::PlcSerialCommand::EnterBootsel);
        app.firmware_flasher.status_is_error = false;
        app.firmware_flasher.status_summary =
            "⚡ Sent CMD 0x0A (Enter BOOTSEL) — Waiting for RP2040 to mount as RPI-RP2 drive...".to_string();
        app.firmware_flasher
            .append_log("Sent CMD_ENTER_BOOTSEL (0x0A) over USB Serial. Waiting for RPI-RP2 volume...");
    }
    if trigger_flash {
        app.firmware_flasher.start_flash_usb(ctx.clone());
    }
    if (!open_win || close_win) && !app.firmware_flasher.is_busy {
        app.firmware_flasher.show_window = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bootsel_drive_validation_with_temp_dir() {
        let tmp = self_update::TempDir::new().unwrap();
        assert!(!FirmwareFlasher::is_bootsel_drive(tmp.path()));

        // Create INFO_UF2.TXT to simulate an RPI-RP2 volume
        std::fs::write(tmp.path().join("INFO_UF2.TXT"), "UF2 Bootloader v3.0\nModel: Raspberry Pi RP2\n").unwrap();
        assert!(FirmwareFlasher::is_bootsel_drive(tmp.path()));

        let detected = FirmwareFlasher::detect_bootsel_drive(&tmp.path().display().to_string());
        assert_eq!(detected.as_deref(), Some(tmp.path()));
    }

    #[test]
    fn test_enter_bootsel_serial_packet_encoding() {
        let dummy = my_canvas_core::BytecodeProgram {
            hardware: String::new(),
            instructions: Vec::new(),
            memory_map: Vec::new(),
        };
        let pkt = dummy.to_serial_packet(my_canvas_core::PlcSerialCommand::EnterBootsel);
        assert_eq!(pkt.len(), 5);
        assert_eq!(pkt[0], 0xAA);
        assert_eq!(pkt[1], 0x0A);
        assert_eq!(pkt[2], 0x00);
        assert_eq!(pkt[3], 0x00);
        assert_eq!(
            pkt[4],
            my_canvas_core::BytecodeProgram::compute_crc8(&pkt[1..4])
        );
    }
}
