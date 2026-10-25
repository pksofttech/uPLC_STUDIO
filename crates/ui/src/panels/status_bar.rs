use egui::{RichText, Ui};
use crate::app::{CanvasApp, WorkMode};
use my_canvas_core::SimState;

pub fn render(ui: &mut Ui, app: &CanvasApp) {
    let palette = app.theme.palette();

    ui.horizontal(|ui| {
        // 1. Operating Mode & Live Status
        match app.work_mode {
            WorkMode::OnlineDebug => {
                let is_connected = app.is_hw_online_connected();
                if !is_connected {
                    ui.label(
                        RichText::new("⚠ MODE: ONLINE PLC (DISCONNECTED - AUTO-RECONNECTING...)")
                            .color(palette.accent_warn)
                            .strong(),
                    );
                } else {
                    match app.online_program_match_info() {
                        Some((true, _, ide_crc, _, _)) => {
                            ui.label(
                                RichText::new(format!(
                                    "🔌 MODE: ONLINE PLC (RP2040) ✔ CODE MATCH [0x{:04X}]",
                                    ide_crc
                                ))
                                .color(palette.accent_ok)
                                .strong(),
                            );
                        }
                        Some((false, _, ide_crc, _, plc_crc)) => {
                            ui.label(
                                RichText::new(format!(
                                    "⚠ MODE: ONLINE PLC (CODE MISMATCH IDE:0x{:04X} ≠ PLC:0x{:04X})",
                                    ide_crc, plc_crc
                                ))
                                .color(palette.accent_warn)
                                .strong(),
                            );
                        }
                        None => {
                            ui.label(
                                RichText::new("🔌 MODE: ONLINE PLC (RP2040 MONITORING)")
                                    .color(palette.accent_ok)
                                    .strong(),
                            );
                        }
                    }
                }
            }
            WorkMode::Simulator => match app.doc.simulator.state {
                SimState::Running => {
                    ui.label(
                        RichText::new("💻 MODE: SIMULATOR (● RUNNING)")
                            .color(palette.accent_ok)
                            .strong(),
                    );
                }
                SimState::Step => {
                    ui.label(
                        RichText::new("💻 MODE: SIMULATOR (⏭ SINGLE STEP)")
                            .color(palette.accent_warn)
                            .strong(),
                    );
                }
                SimState::Stopped => {
                    ui.label(
                        RichText::new("💻 MODE: SIMULATOR (⏸ PAUSED)")
                            .color(palette.accent_heading)
                            .strong(),
                    );
                }
            },
            WorkMode::Edit => {
                ui.label(
                    RichText::new("✏ MODE: EDIT (OFFLINE CIRCUIT DESIGN)")
                        .color(palette.accent_heading)
                        .strong(),
                );
            }
        }
        ui.separator();

        // 2. Hardware & Port Info
        ui.label(format!(
            "HW: {} | Port: {} @ {} bps",
            app.doc.program.hardware,
            app.com_port_display(),
            app.baud_rate
        ));
        ui.separator();

        let (scan_cnt, cycle_ms) = if app.online_mode && app.online_target_hw {
            let sc = if app.is_hw_online_connected() {
                app.latest_snapshot.as_ref().map_or(0, |s| s.scan_count as u64)
            } else {
                0
            };
            (sc, 10u32) // Deterministic 10ms scan cycle on RP2040 hardware (SCAN_CYCLE_MS)
        } else {
            (app.doc.simulator.scan_count, app.doc.simulator.cycle_time_ms)
        };
        ui.label(format!("Scans: {} | Cycle: {}ms", scan_cnt, cycle_ms));

        if app.online_mode || app.doc.simulator.is_active() {
            ui.separator();
            let (acc_b, acc_w) = if app.online_mode && app.online_target_hw {
                if app.is_hw_online_connected() {
                    if let Some(snap) = app.latest_snapshot.as_ref() {
                        (snap.acc_bool, snap.acc_word)
                    } else {
                        (false, 0)
                    }
                } else {
                    (false, 0)
                }
            } else {
                (app.doc.simulator.acc_bool, app.doc.simulator.acc_word)
            };
            ui.label(
                RichText::new(format!(
                    "ACC: Bit={} | Word={}",
                    u8::from(acc_b),
                    acc_w
                ))
                .monospace()
                .color(if acc_b {
                    palette.accent_ok
                } else {
                    palette.accent_heading
                }),
            );
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(format!("Theme: {}", app.theme.label()));
            ui.separator();
            ui.label(format!("Zoom: {:.0}%", app.canvas.zoom * 100.0));
            ui.separator();
            ui.label(format!("Tool: {}", app.active_tool.name()));
            ui.separator();
            if let Some(r_idx) = app.doc.selected_rung_idx {
                ui.label(format!("Network #{:02}", r_idx + 1));
            }
        });
    });
}
