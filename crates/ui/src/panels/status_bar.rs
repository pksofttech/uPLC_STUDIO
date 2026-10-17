use egui::{RichText, Ui};
use crate::app::CanvasApp;
use my_canvas_core::SimState;

pub fn render(ui: &mut Ui, app: &CanvasApp) {
    let palette = app.theme.palette();

    ui.horizontal(|ui| {
        // 1. Hardware Name
        ui.label(
            RichText::new(format!("Hardware: {}", app.doc.program.hardware))
                .color(palette.accent_heading)
                .strong(),
        );
        ui.separator();

        // 2. Online Monitoring / Simulation Status
        if app.online_mode {
            let target = if app.online_target_hw { "HW MCU" } else { "SIM" };
            match app.online_program_match_info() {
                Some((true, _, ide_crc, _, _)) => {
                    ui.label(
                        RichText::new(format!(
                            "🟢 ONLINE ({}) ✔ CODE MATCH [0x{:04X}]",
                            target, ide_crc
                        ))
                        .color(palette.accent_ok)
                        .strong(),
                    );
                }
                Some((false, _, ide_crc, _, plc_crc)) => {
                    ui.label(
                        RichText::new(format!(
                            "⚠ ONLINE ({}) CODE MISMATCH [IDE:0x{:04X} ≠ PLC:0x{:04X}]",
                            target, ide_crc, plc_crc
                        ))
                        .color(palette.accent_warn)
                        .strong(),
                    );
                }
                None => {
                    ui.label(
                        RichText::new(format!("🟢 ONLINE MONITORING ({})", target))
                            .color(palette.accent_ok)
                            .strong(),
                    );
                }
            }
        } else {
            match app.doc.simulator.state {
                SimState::Running => {
                    ui.label(
                        RichText::new("● SIMULATION RUNNING")
                            .color(palette.accent_ok)
                            .strong(),
                    );
                }
                SimState::Step => {
                    ui.label(
                        RichText::new("● SIMULATION STEP")
                            .color(palette.accent_warn)
                            .strong(),
                    );
                }
                SimState::Stopped => {
                    ui.label(
                        RichText::new("■ READY (EDIT MODE)").color(palette.rung_text_dim),
                    );
                }
            }
        }
        ui.separator();

        // 3. COM Port Info
        ui.label(format!("Port: {} @ {} bps", app.com_port, app.baud_rate));
        ui.separator();

        let (scan_cnt, cycle_ms) = if app.online_mode && app.online_target_hw {
            let sc = app.latest_snapshot.as_ref().map_or(0, |s| s.scan_count as u64);
            (sc, 1u32) // Deterministic 1ms scan cycle on RP2040 hardware
        } else {
            (app.doc.simulator.scan_count, app.doc.simulator.cycle_time_ms)
        };
        ui.label(format!("Scans: {} | Cycle: {}ms", scan_cnt, cycle_ms));

        if app.online_mode || app.doc.simulator.is_active() {
            ui.separator();
            let (acc_b, acc_w) = if app.online_mode && app.online_target_hw {
                if let Some(snap) = app.latest_snapshot.as_ref() {
                    (snap.acc_bool, snap.acc_word)
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
