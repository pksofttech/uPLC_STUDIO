use egui::{
    pos2, vec2, Align2, Color32, CornerRadius, FontId, Rect, RichText, Sense, Stroke, StrokeKind,
    Ui, Window,
};
use my_canvas_core::{
    CoilMode, CoilStyle, ContactMode, ContactStyle, Document, LadderElement, TypeTag,
};
use crate::icons::{paint_icon, VectorIcon};
use crate::theme::ThemePalette;

#[derive(Debug, Clone, Default)]
pub struct ElementConfigModal {
    pub is_open: bool,
    pub target_id: u64,
    pub rung_idx: Option<usize>,
    pub rung_comment: String,
    pub draft: LadderElement,
}

pub fn draw_modal_backdrop(ctx: &egui::Context, _palette: ThemePalette) {
    let screen_rect = ctx.content_rect();
    egui::Area::new(egui::Id::new("app_modal_backdrop_shield"))
        .order(egui::Order::Middle)
        .fixed_pos(screen_rect.min)
        .interactable(true)
        .show(ctx, |ui| {
            let (_, painter) =
                ui.allocate_painter(screen_rect.size(), Sense::click_and_drag());
            // Standard modal backdrop overlay (translucent dark dim ~50%)
            painter.rect_filled(screen_rect, 0.0, Color32::from_black_alpha(128));
        });
}

impl ElementConfigModal {
    pub fn open_for(&mut self, elem: &LadderElement, rung_idx: Option<usize>, rung_comment: &str) {
        self.is_open = true;
        self.target_id = elem.id;
        self.rung_idx = rung_idx;
        self.rung_comment = rung_comment.to_string();
        self.draft = elem.clone();
    }

    pub fn open_for_rung(&mut self, rung_idx: usize, rung: &my_canvas_core::Rung) {
        self.is_open = true;
        self.target_id = rung.elements.first().map(|e| e.id).unwrap_or(0);
        self.rung_idx = Some(rung_idx);
        self.rung_comment = rung.comment.clone();
        if let Some(first_el) = rung.elements.first() {
            self.draft = first_el.clone();
        } else {
            self.draft = LadderElement::default();
        }
    }

    pub fn close(&mut self) {
        self.is_open = false;
    }

    pub fn show(&mut self, ctx: &egui::Context, doc: &mut Document, palette: ThemePalette) {
        if !self.is_open {
            return;
        }

        let screen_rect = ctx.content_rect();

        let mut apply_clicked = false;
        let mut cancel_clicked = false;
        let mut delete_clicked = false;

        // Keyboard shortcuts inside modal: Enter to Apply, Escape to Cancel
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            cancel_clicked = true;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift) {
            apply_clicked = true;
        }

        let title = format!(
            "Element Configuration — {} (#{})",
            self.draft.element_type.display_name(),
            self.target_id
        );

        let mut open_flag = self.is_open;
        let max_w = (screen_rect.width() - 32.0).max(560.0);
        let target_w = 680.0_f32.min(max_w);
        let max_h = (screen_rect.height() - 24.0).max(580.0);
        // Only enable vertical scroll if the viewport is too short to fit the full dialog
        let needs_vscroll = screen_rect.height() < 660.0;

        Window::new(title)
            .order(egui::Order::Foreground)
            .open(&mut open_flag)
            .collapsible(false)
            .resizable(true)
            .min_width(target_w)
            .default_width(target_w)
            .max_width(max_w)
            .max_height(max_h)
            .vscroll(needs_vscroll)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.set_min_width(target_w - 24.0);

                // --- 1. Live Schematic Preview Box (like MICROPLC pictureBox1, upgraded) ---
                self.draw_live_preview_banner(ui, doc.contact_style, doc.coil_style, palette);

                ui.add_space(8.0);

                // --- 2. IEC Address, Symbol & Cross-Reference Section ---
                ui.group(|ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        RichText::new("1. IEC Address & Symbol Assignment")
                            .color(palette.accent_heading)
                            .strong(),
                    );
                    ui.separator();

                    egui::Grid::new("elem_modal_addr_grid")
                        .num_columns(2)
                        .spacing([12.0, 8.0])
                        .min_col_width(125.0)
                        .show(ui, |ui| {
                            ui.label("IEC Address:");
                            ui.horizontal(|ui| {
                                let resp = ui.add(
                                    egui::TextEdit::singleline(&mut self.draft.name)
                                        .font(egui::TextStyle::Monospace)
                                        .hint_text("e.g. %IX0.0, %QX0.0, %MX0.0")
                                        .desired_width(160.0),
                                );
                                if resp.changed() {
                                    self.draft.name = self.draft.name.to_ascii_uppercase();
                                    // Auto-resolve symbol from Global Symbol Table if present
                                    if let Some(sym) = doc.resolve_symbol_for_address(&self.draft.name) {
                                        if !sym.is_empty() {
                                            self.draft.symbol = sym;
                                        }
                                    }
                                }

                                // Dropdown to pick from existing IEC elements or I/O mappings
                                let existing_iec = doc.program.collect_all_iec_elements();
                                let combo_w = (ui.available_width() - 4.0).max(260.0);
                                egui::ComboBox::from_id_salt("existing_tag_picker")
                                    .selected_text("Pick IEC / Symbol...")
                                    .width(combo_w)
                                    .show_ui(ui, |ui| {
                                        for (_raw, iec_addr, sym, t_type, _) in &existing_iec {
                                            if ui
                                                .selectable_label(
                                                    self.draft.name == *iec_addr,
                                                    format!("{} — {} ({})", iec_addr, sym, t_type.display_name()),
                                                )
                                                .clicked()
                                            {
                                                self.draft.name = iec_addr.clone();
                                                self.draft.symbol = sym.clone();
                                            }
                                        }
                                        for (idx, io) in doc.program.io_mappings.iter().enumerate() {
                                            let iec_addr = io.iec_address(idx);
                                            if ui
                                                .selectable_label(
                                                    self.draft.name == iec_addr || self.draft.name == io.plc_address,
                                                    format!(
                                                        "{} — {} [Pin {}]",
                                                        iec_addr, io.symbol, io.hw_pin
                                                    ),
                                                )
                                                .clicked()
                                            {
                                                self.draft.name = iec_addr;
                                                self.draft.symbol = io.symbol.clone();
                                            }
                                        }
                                    });
                            });
                            ui.end_row();

                            ui.label("Symbol:");
                            let full_field_w = (ui.available_width() - 4.0).max(430.0);
                            ui.vertical(|ui| {
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.draft.symbol)
                                        .hint_text("e.g. START_PB, MOTOR_RUN")
                                        .desired_width(full_field_w),
                                );
                                // Global Symbol Table conflict / usage indicators
                                if doc.program.symbols.is_symbol_taken_by_other(&self.draft.symbol, &self.draft.name) {
                                    ui.colored_label(
                                        Color32::from_rgb(230, 126, 34),
                                        "⚠ Warning: Symbol is already used by another address in Global Symbol Table",
                                    );
                                }
                                let usages = doc.find_address_usages(&self.draft.name);
                                if usages.len() > 1 {
                                    ui.label(
                                        RichText::new(format!(
                                            "ℹ Synced across {} elements in project (Single Source of Truth)",
                                            usages.len()
                                        ))
                                        .small()
                                        .color(palette.rung_text_dim),
                                    );
                                }
                            });
                            ui.end_row();

                            ui.label("Remark:");
                            let full_field_w = (ui.available_width() - 4.0).max(430.0);
                            ui.add(
                                egui::TextEdit::singleline(&mut self.draft.comment)
                                    .hint_text("Element description / remark...")
                                    .desired_width(full_field_w),
                            );
                            ui.end_row();

                            let rung_lbl = if let Some(r_idx) = self.rung_idx {
                                format!("Network #{:02} Comment:", r_idx + 1)
                            } else {
                                "Network Comment:".to_string()
                            };
                            ui.label(rung_lbl);
                            let full_field_w = (ui.available_width() - 4.0).max(430.0);
                            ui.add(
                                egui::TextEdit::singleline(&mut self.rung_comment)
                                    .hint_text("Enter comment for this network...")
                                    .desired_width(full_field_w),
                            );
                            ui.end_row();
                        });

                    ui.add_space(4.0);

                    // Quick IEC Address Bank (%IX / %QX / %MX / %MW / T / C)
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new("Quick IEC Bank:").small().color(palette.rung_text_dim));
                        let banks: &[(&str, &str)] = match self.draft.element_type {
                            TypeTag::Coil => &[
                                ("%QX0.0", "MOTOR_RUN"),
                                ("%QX0.1", "LAMP_RUN"),
                                ("%QX0.2", "ALARM_BUZZ"),
                                ("%QX0.3", "VALVE_OUT"),
                                ("%MX0.0", "RELAY_M0"),
                                ("%MX0.1", "RELAY_M1"),
                            ],
                            TypeTag::Ton | TypeTag::Tof | TypeTag::Tp => &[
                                ("T0", "TIMER_0"),
                                ("T1", "TIMER_1"),
                                ("T2", "TIMER_2"),
                                ("T3", "TIMER_3"),
                            ],
                            TypeTag::Ctu | TypeTag::Ctd | TypeTag::Ctud => &[
                                ("C0", "COUNT_0"),
                                ("C1", "COUNT_1"),
                                ("C2", "COUNT_2"),
                                ("C3", "COUNT_3"),
                            ],
                            TypeTag::Move | TypeTag::Math | TypeTag::Compare => &[
                                ("%MW0", "VAR_0"),
                                ("%MW1", "VAR_1"),
                                ("C0", "COUNT_0"),
                                ("T0", "TIMER_0"),
                                ("%IW0", "ADC_0"),
                            ],
                            TypeTag::Adc => &[
                                ("%IW0", "ADC_0"),
                                ("%IW1", "ADC_1"),
                                ("%IW2", "ADC_2"),
                                ("%IW3", "ADC_3"),
                            ],
                            TypeTag::Bitwise
                            | TypeTag::ShiftRegisters
                            | TypeTag::ShiftRegInbit
                            | TypeTag::ShiftRegReset
                            | TypeTag::ShiftLeft
                            | TypeTag::ShiftRight
                            | TypeTag::ShiftVariable => &[
                                ("%MW0", "VAR_0"),
                                ("%MW1", "VAR_1"),
                                ("%MW30", "SHIFT_REG"),
                                ("%MW96", "RETAIN_0"),
                            ],
                            TypeTag::Pid | TypeTag::Scale | TypeTag::Limit => &[
                                ("PID0", "PID_LOOP"),
                                ("SCALE0", "SCALE_0"),
                                ("LIMIT0", "LIMIT_0"),
                            ],
                            TypeTag::UartSend | TypeTag::UartRecv => &[
                                ("U_TX0", "UART_SEND"),
                                ("U_RX0", "UART_RECV"),
                            ],
                            _ => &[
                                ("%IX0.0", "START_PB"),
                                ("%IX0.1", "STOP_PB"),
                                ("%IX0.2", "SENSOR_1"),
                                ("%IX0.3", "LIMIT_SW"),
                                ("%QX0.0", "MOTOR_RUN"),
                                ("%MX0.0", "RELAY_M0"),
                                ("T0", "T0_DONE"),
                                ("C0", "C0_DONE"),
                            ],
                        };
                        for &(addr, sym) in banks {
                            let is_sel = self.draft.name == addr;
                            if ui
                                .selectable_label(is_sel, RichText::new(addr).monospace())
                                .on_hover_text(sym)
                                .clicked()
                            {
                                self.draft.name = addr.to_string();
                                if self.draft.symbol.is_empty()
                                    || self.draft.symbol.starts_with("IN_")
                                    || self.draft.symbol.starts_with("OUT_")
                                    || self.draft.symbol.starts_with('%')
                                    || self.draft.symbol.starts_with('X')
                                    || self.draft.symbol.starts_with('Y')
                                {
                                    self.draft.symbol = sym.to_string();
                                }
                            }
                        }
                    });

                    // Cross-Reference & Hardware Pin Info Bar
                    let usages = doc.find_address_usages(&self.draft.name);
                    let hw_pin = doc
                        .program
                        .io_mappings
                        .iter()
                        .find(|m| {
                            m.iec_address.eq_ignore_ascii_case(&self.draft.name)
                                || m.plc_address.eq_ignore_ascii_case(&self.draft.name)
                        })
                        .map(|m| format!("{} ({})", m.hw_pin, if m.is_input { "Input" } else { "Output" }))
                        .unwrap_or_else(|| "Internal / Virtual".to_string());

                    ui.add_space(4.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.label(
                            RichText::new(format!("Pin Mapping: {}", hw_pin))
                                .small()
                                .color(palette.accent_ok),
                        );
                        ui.separator();
                        // RETAIN badge: address (or OUT / IN register) lies in the Flash-retained area
                        let retain_addr = [
                            &self.draft.name,
                            &self.draft.target_reg,
                            &self.draft.fb.in1,
                            &self.draft.fb.in2,
                        ]
                        .into_iter()
                        .find(|a| is_retain_address(a))
                        .cloned();
                        if let Some(addr) = retain_addr {
                            ui.label(
                                RichText::new("🔒 RETAIN")
                                    .small()
                                    .strong()
                                    .color(palette.accent_ok)
                                    .background_color(palette.accent_ok.gamma_multiply(0.15)),
                            )
                            .on_hover_text(format!(
                                "{} is in the RETAIN area (M64..M95 / %MW96..%MW127): saved to Flash by the \
                                 firmware and restored after power loss",
                                addr
                            ));
                            ui.separator();
                        }

                        // Type Classifier Badge (Memory Helper vs Physical Hardware)
                        let norm_addr = self.draft.name.trim().to_ascii_uppercase();
                        if norm_addr.starts_with("%MX")
                            || (norm_addr.starts_with('M')
                                && norm_addr.len() > 1
                                && norm_addr[1..].chars().next().map_or(false, |c| c.is_ascii_digit()))
                        {
                            ui.label(
                                RichText::new("🧠 Memory Relay (%M)")
                                    .small()
                                    .strong()
                                    .color(Color32::from_rgb(190, 130, 255))
                                    .background_color(Color32::from_rgb(190, 130, 255).gamma_multiply(0.15)),
                            )
                            .on_hover_text("Internal CPU Memory Bit: auxiliary relay for program logic (no physical terminal)");
                            ui.separator();
                        } else if norm_addr.starts_with("%QX")
                            || (norm_addr.starts_with('Y')
                                && norm_addr.len() > 1
                                && norm_addr[1..].chars().next().map_or(false, |c| c.is_ascii_digit()))
                        {
                            ui.label(
                                RichText::new("🔌 Physical Output (%Q)")
                                    .small()
                                    .strong()
                                    .color(palette.accent_heading)
                                    .background_color(palette.accent_heading.gamma_multiply(0.15)),
                            )
                            .on_hover_text("Hardware Output Terminal: physically drives relay, actuator or motor");
                            ui.separator();
                        } else if norm_addr.starts_with("%IX")
                            || (norm_addr.starts_with('X')
                                && norm_addr.len() > 1
                                && norm_addr[1..].chars().next().map_or(false, |c| c.is_ascii_digit()))
                        {
                            ui.label(
                                RichText::new("📥 Physical Input (%I)")
                                    .small()
                                    .strong()
                                    .color(palette.accent_warn)
                                    .background_color(palette.accent_warn.gamma_multiply(0.15)),
                            )
                            .on_hover_text("Hardware Input Terminal: receives signal from external switch/sensor");
                            ui.separator();
                        }
                        let usage_str = if usages.is_empty() {
                            "New IEC address (not used elsewhere yet)".to_string()
                        } else {
                            let r_list: Vec<String> = usages
                                .iter()
                                .map(|(r, _, t)| format!("R{:02}({})", r + 1, t.display_name()))
                                .collect();
                            format!("Cross-Ref ({}): {}", usages.len(), r_list.join(", "))
                        };
                        ui.label(RichText::new(usage_str).small().color(palette.rung_text_dim));
                    });
                });

                ui.add_space(8.0);

                // --- 3. Type-Specific Parameters Section ---
                ui.group(|ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        RichText::new("2. Instruction Mode & Parameters")
                            .color(palette.accent_warn)
                            .strong(),
                    );
                    ui.separator();

                    match self.draft.element_type {
                        TypeTag::Contact
                        | TypeTag::RTrig
                        | TypeTag::FTrig => {
                            if self.draft.element_type == TypeTag::Contact {
                                ui.label("Contact Logic Type:");
                                let card_w = ((ui.available_width() - 10.0) * 0.5).max(260.0);
                                egui::Grid::new("contact_mode_cards_grid")
                                    .num_columns(2)
                                    .spacing([10.0, 8.0])
                                    .show(ui, |ui| {
                                        mode_select_card(
                                            ui,
                                            card_w,
                                            VectorIcon::ContactNo,
                                            "Normally Open (NO)",
                                            "Conducts when input is ON (1)",
                                            self.draft.contact_mode == ContactMode::NormallyOpen,
                                            palette,
                                            || self.draft.contact_mode = ContactMode::NormallyOpen,
                                        );
                                        mode_select_card(
                                            ui,
                                            card_w,
                                            VectorIcon::ContactNc,
                                            "Normally Closed (NC)",
                                            "Conducts when input is OFF (0)",
                                            self.draft.contact_mode == ContactMode::NormallyClosed,
                                            palette,
                                            || self.draft.contact_mode = ContactMode::NormallyClosed,
                                        );
                                        ui.end_row();

                                        mode_select_card(
                                            ui,
                                            card_w,
                                            VectorIcon::ContactNo,
                                            "Positive Transition (P)",
                                            "Pulse 1 scan on rising edge",
                                            self.draft.contact_mode == ContactMode::PositiveTransition,
                                            palette,
                                            || self.draft.contact_mode = ContactMode::PositiveTransition,
                                        );
                                        mode_select_card(
                                            ui,
                                            card_w,
                                            VectorIcon::ContactNc,
                                            "Negative Transition (N)",
                                            "Pulse 1 scan on falling edge",
                                            self.draft.contact_mode == ContactMode::NegativeTransition,
                                            palette,
                                            || self.draft.contact_mode = ContactMode::NegativeTransition,
                                        );
                                        ui.end_row();
                                    });
                                ui.add_space(6.0);
                            }

                            ui.horizontal_wrapped(|ui| {
                                ui.label("Signal Trigger Mode:");
                                if ui
                                    .selectable_label(
                                        self.draft.element_type == TypeTag::Contact,
                                        "Standard Level",
                                    )
                                    .clicked()
                                {
                                    self.draft.element_type = TypeTag::Contact;
                                }
                                if ui
                                    .selectable_label(
                                        self.draft.element_type == TypeTag::RTrig,
                                        "^ R_TRIG (Rising Edge FB)",
                                    )
                                    .clicked()
                                {
                                    self.draft.element_type = TypeTag::RTrig;
                                }
                                if ui
                                    .selectable_label(
                                        self.draft.element_type == TypeTag::FTrig,
                                        "v F_TRIG (Falling Edge FB)",
                                    )
                                    .clicked()
                                {
                                    self.draft.element_type = TypeTag::FTrig;
                                }
                            });
                        }

                        TypeTag::Coil => {
                            ui.label("Output Coil Action:");
                            let card_w = ((ui.available_width() - 10.0) * 0.5).max(260.0);
                            egui::Grid::new("coil_mode_cards_grid")
                                .num_columns(2)
                                .spacing([10.0, 8.0])
                                .show(ui, |ui| {
                                    mode_select_card(
                                        ui,
                                        card_w,
                                        VectorIcon::Coil,
                                        "Normal ( )",
                                        "Energized while network is true",
                                        self.draft.coil_mode == CoilMode::Normal,
                                        palette,
                                        || self.draft.coil_mode = CoilMode::Normal,
                                    );
                                    mode_select_card(
                                        ui,
                                        card_w,
                                        VectorIcon::Coil,
                                        "Negated (/)",
                                        "Inverted output state",
                                        self.draft.coil_mode == CoilMode::Negated,
                                        palette,
                                        || self.draft.coil_mode = CoilMode::Negated,
                                    );
                                    ui.end_row();

                                    mode_select_card(
                                        ui,
                                        card_w,
                                        VectorIcon::CoilSet,
                                        "Set Latch (S)",
                                        "Latches ON when network is true",
                                        self.draft.coil_mode == CoilMode::SetLatch,
                                        palette,
                                        || self.draft.coil_mode = CoilMode::SetLatch,
                                    );
                                    mode_select_card(
                                        ui,
                                        card_w,
                                        VectorIcon::CoilReset,
                                        "Reset Unlatch (R)",
                                        "Resets OFF when network is true",
                                        self.draft.coil_mode == CoilMode::ResetUnlatch,
                                        palette,
                                        || self.draft.coil_mode = CoilMode::ResetUnlatch,
                                    );
                                    ui.end_row();

                                    mode_select_card(
                                        ui,
                                        card_w,
                                        VectorIcon::Coil,
                                        "Positive Edge (P)",
                                        "Pulse 1 scan on rising edge",
                                        self.draft.coil_mode == CoilMode::PositiveTransition,
                                        palette,
                                        || self.draft.coil_mode = CoilMode::PositiveTransition,
                                    );
                                    mode_select_card(
                                        ui,
                                        card_w,
                                        VectorIcon::Coil,
                                        "Negative Edge (N)",
                                        "Pulse 1 scan on falling edge",
                                        self.draft.coil_mode == CoilMode::NegativeTransition,
                                        palette,
                                        || self.draft.coil_mode = CoilMode::NegativeTransition,
                                    );
                                    ui.end_row();
                                });

                            ui.add_space(8.0);
                            ui.label(RichText::new("Coil Visual Style (Symbol Representation):").strong());
                            ui.horizontal_wrapped(|ui| {
                                let is_auto = self.draft.coil_style.is_none();
                                if ui.selectable_label(is_auto, "Auto (Global)").clicked() {
                                    self.draft.coil_style = None;
                                }
                                for &st in &CoilStyle::ALL {
                                    let selected = self.draft.coil_style == Some(st);
                                    let label = match st {
                                        CoilStyle::IecParentheses => "IEC —( )—",
                                        CoilStyle::CircleRelay => "DIN Relay —○—",
                                        CoilStyle::NemaBox => "NEMA Starter —[ ]—",
                                        CoilStyle::SolenoidValve => "Solenoid —[ ⧖ ]—",
                                        CoilStyle::PilotLight => "Pilot Lamp —( ⨂ )—",
                                    };
                                    if ui.selectable_label(selected, label).clicked() {
                                        self.draft.coil_style = Some(st);
                                    }
                                }
                            });
                        }

                        TypeTag::Ton | TypeTag::Tof | TypeTag::Tp => {
                            ui.horizontal(|ui| {
                                ui.label("Timer Function:");
                                for (t_type, lbl) in [
                                    (TypeTag::Ton, "TON (On-Delay)"),
                                    (TypeTag::Tof, "TOF (Off-Delay)"),
                                    (TypeTag::Tp, "TP (Pulse Timer)"),
                                ] {
                                    if ui
                                        .selectable_label(self.draft.element_type == t_type, lbl)
                                        .clicked()
                                    {
                                        self.draft.element_type = t_type;
                                    }
                                }
                            });

                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                ui.label("Preset Time PT (ms):");
                                ui.add(
                                    egui::DragValue::new(&mut self.draft.preset_value)
                                        .range(10..=3_600_000)
                                        .speed(50)
                                        .suffix(" ms"),
                                );
                                ui.label(
                                    RichText::new(format!(
                                        "({:.2} sec)",
                                        self.draft.preset_value as f32 / 1000.0
                                    ))
                                    .monospace()
                                    .color(palette.accent_ok),
                                );
                            });

                            if self.draft.element_type == TypeTag::Tpc {
                                ui.horizontal(|ui| {
                                    ui.label("Pulse T2 Interval (ms):");
                                    ui.add(
                                        egui::DragValue::new(&mut self.draft.preset_value1)
                                            .range(10..=3_600_000)
                                            .speed(50)
                                            .suffix(" ms"),
                                    );
                                });
                            }

                            ui.add_space(4.0);
                            ui.horizontal_wrapped(|ui| {
                                ui.label(RichText::new("Quick Presets:").small().color(palette.rung_text_dim));
                                for (ms, lbl) in [
                                    (100, "100ms"),
                                    (250, "250ms"),
                                    (500, "0.5s"),
                                    (1000, "1.0s"),
                                    (2000, "2.0s"),
                                    (3000, "3.0s"),
                                    (5000, "5.0s"),
                                    (10000, "10s"),
                                ] {
                                    if ui
                                        .selectable_label(self.draft.preset_value == ms, lbl)
                                        .clicked()
                                    {
                                        self.draft.preset_value = ms;
                                    }
                                }
                            });
                        }

                        TypeTag::Ctu | TypeTag::Ctd | TypeTag::Ctud => {
                            ui.horizontal(|ui| {
                                ui.label("Counter Function:");
                                for (c_type, lbl) in [
                                    (TypeTag::Ctu, "CTU (Count Up)"),
                                    (TypeTag::Ctd, "CTD (Count Down)"),
                                    (TypeTag::Ctud, "CTUD (Up/Down Counter)"),
                                ] {
                                    if ui
                                        .selectable_label(self.draft.element_type == c_type, lbl)
                                        .clicked()
                                    {
                                        self.draft.element_type = c_type;
                                    }
                                }
                            });

                            ui.add_space(6.0);
                            egui::Grid::new("counter_params_grid")
                                .num_columns(2)
                                .spacing([12.0, 6.0])
                                .show(ui, |ui| {
                                    ui.label("Target Preset (PV):");
                                    ui.horizontal(|ui| {
                                        ui.add(
                                            egui::DragValue::new(&mut self.draft.preset_value)
                                                .range(1..=100_000)
                                                .speed(1),
                                        );
                                        for pv in [1, 3, 5, 10, 20, 50, 100] {
                                            if ui
                                                .selectable_label(
                                                    self.draft.preset_value == pv,
                                                    format!("{}", pv),
                                                )
                                                .clicked()
                                            {
                                                self.draft.preset_value = pv;
                                            }
                                        }
                                    });
                                    ui.end_row();

                                    ui.label("Initial Value (SV):");
                                    ui.add(
                                        egui::DragValue::new(&mut self.draft.preset_value1)
                                            .range(0..=100_000)
                                            .speed(1),
                                    );
                                    ui.end_row();
                                });
                        }

                        TypeTag::Compare => {
                            ui.label("Comparison Operator:");
                            ui.horizontal_wrapped(|ui| {
                                for (op, desc) in [
                                    ("==", "Equal"),
                                    ("!=", "Not Equal"),
                                    (">", "Greater Than"),
                                    (">=", "Greater or Equal"),
                                    ("<", "Less Than"),
                                    ("<=", "Less or Equal"),
                                ] {
                                    if ui
                                        .selectable_label(
                                            self.draft.compare_op == op,
                                            format!("{} ({})", op, desc),
                                        )
                                        .clicked()
                                    {
                                        self.draft.compare_op = op.to_string();
                                    }
                                }
                            });
                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                ui.label("Compare Value / Operand:");
                                ui.add(
                                    egui::DragValue::new(&mut self.draft.preset_value)
                                        .range(-100_000..=100_000)
                                        .speed(1),
                                );
                            });
                        }

                        TypeTag::Move | TypeTag::Math => {
                            ui.horizontal_wrapped(|ui| {
                                ui.label("Operation:");
                                if ui
                                    .selectable_label(
                                        self.draft.element_type == TypeTag::Move,
                                        "MOVE (:=)",
                                    )
                                    .clicked()
                                {
                                    self.draft.element_type = TypeTag::Move;
                                }
                                for (op, lbl) in [
                                    ("+", "ADD (+=)"),
                                    ("-", "SUB (-=)"),
                                    ("*", "MUL (*=)"),
                                    ("/", "DIV (/=)"),
                                    ("%", "MOD (%=)"),
                                ] {
                                    let is_cur = self.draft.element_type == TypeTag::Math
                                        && self.draft.math_op == op;
                                    if ui.selectable_label(is_cur, lbl).clicked() {
                                        self.draft.element_type = TypeTag::Math;
                                        self.draft.math_op = op.to_string();
                                    }
                                }
                            });
                            ui.add_space(6.0);
                            egui::Grid::new("move_math_params_grid")
                                .num_columns(2)
                                .spacing([12.0, 6.0])
                                .show(ui, |ui| {
                                    ui.label("Destination Register (OUT):");
                                    ui.horizontal(|ui| {
                                        let resp = ui.add(
                                            egui::TextEdit::singleline(&mut self.draft.name)
                                                .font(egui::TextStyle::Monospace)
                                                .hint_text("e.g. %MW0, D0")
                                                .desired_width(140.0),
                                        );
                                        if resp.changed() {
                                            self.draft.name = self.draft.name.to_ascii_uppercase();
                                            self.draft.target_reg = self.draft.name.clone();
                                        }
                                        for r in ["%MW0", "%MW1", "%MW2", "%MW96"] {
                                            if ui.selectable_label(self.draft.name == r, r).clicked() {
                                                self.draft.name = r.to_string();
                                                self.draft.target_reg = r.to_string();
                                            }
                                        }
                                    });
                                    ui.end_row();

                                    ui.label("Source Value / Constant:");
                                    ui.add(
                                        egui::DragValue::new(&mut self.draft.preset_value)
                                            .range(-100_000..=100_000)
                                            .speed(1),
                                    );
                                    ui.end_row();
                                });
                        }

                        TypeTag::Adc => {
                            egui::Grid::new("adc_params_grid")
                                .num_columns(2)
                                .spacing([12.0, 6.0])
                                .show(ui, |ui| {
                                    ui.label("ADC Channel (%IW):");
                                    ui.horizontal_wrapped(|ui| {
                                        for (ch, lbl) in [
                                            ("%IW0", "%IW0 (GPIO26 / ADC0)"),
                                            ("%IW1", "%IW1 (GPIO27 / ADC1)"),
                                            ("%IW2", "%IW2 (GPIO28 / ADC2)"),
                                            ("%IW3", "%IW3 (GPIO29 / ADC3)"),
                                        ] {
                                            if ui
                                                .selectable_label(
                                                    self.draft.name.eq_ignore_ascii_case(ch),
                                                    lbl,
                                                )
                                                .clicked()
                                            {
                                                self.draft.name = ch.to_string();
                                            }
                                        }
                                    });
                                    ui.end_row();

                                    ui.label("Destination Register (OUT):");
                                    ui.horizontal(|ui| {
                                        let resp = ui.add(
                                            egui::TextEdit::singleline(&mut self.draft.target_reg)
                                                .font(egui::TextStyle::Monospace)
                                                .hint_text("e.g. %MW0 (optional)")
                                                .desired_width(140.0),
                                        );
                                        if resp.changed() {
                                            self.draft.target_reg =
                                                self.draft.target_reg.to_ascii_uppercase();
                                        }
                                        for r in ["%MW0", "%MW1", "%MW2", "%MW3"] {
                                            if ui
                                                .selectable_label(self.draft.target_reg == r, r)
                                                .clicked()
                                            {
                                                self.draft.target_reg = r.to_string();
                                            }
                                        }
                                    });
                                    ui.end_row();
                                });
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new("Reads 12-bit analog input (0..4095) into accumulator and OUT register when rung is TRUE.")
                                    .small()
                                    .color(palette.rung_text_dim),
                            );
                        }

                        TypeTag::Pid => {
                            if self.draft.fb.k[3] <= 0 {
                                self.draft.fb.k[3] = 1000;
                            }
                            egui::Grid::new("pid_params_grid")
                                .num_columns(2)
                                .spacing([12.0, 6.0])
                                .show(ui, |ui| {
                                    ui.label("Process Variable (PV):");
                                    ui.horizontal(|ui| {
                                        let r = ui.add(
                                            egui::TextEdit::singleline(&mut self.draft.fb.in1)
                                                .font(egui::TextStyle::Monospace)
                                                .hint_text("e.g. %IW0, %MW0")
                                                .desired_width(140.0),
                                        );
                                        if r.changed() {
                                            self.draft.fb.in1 =
                                                self.draft.fb.in1.to_ascii_uppercase();
                                        }
                                        for p in ["%IW0", "%IW1", "%MW0", "%MW1"] {
                                            if ui.selectable_label(self.draft.fb.in1 == p, p).clicked() {
                                                self.draft.fb.in1 = p.to_string();
                                            }
                                        }
                                    });
                                    ui.end_row();

                                    ui.label("Setpoint (SP):");
                                    ui.horizontal(|ui| {
                                        let r = ui.add(
                                            egui::TextEdit::singleline(&mut self.draft.fb.in2)
                                                .font(egui::TextStyle::Monospace)
                                                .hint_text("Register (%MW1) or literal (500)")
                                                .desired_width(140.0),
                                        );
                                        if r.changed() {
                                            self.draft.fb.in2 =
                                                self.draft.fb.in2.to_ascii_uppercase();
                                        }
                                        for p in ["500", "1000", "2048", "%MW1"] {
                                            if ui.selectable_label(self.draft.fb.in2 == p, p).clicked() {
                                                self.draft.fb.in2 = p.to_string();
                                            }
                                        }
                                    });
                                    ui.end_row();

                                    ui.label("Output Register (OUT):");
                                    ui.horizontal(|ui| {
                                        let r = ui.add(
                                            egui::TextEdit::singleline(&mut self.draft.target_reg)
                                                .font(egui::TextStyle::Monospace)
                                                .hint_text("e.g. %MW10, %QW0")
                                                .desired_width(140.0),
                                        );
                                        if r.changed() {
                                            self.draft.target_reg =
                                                self.draft.target_reg.to_ascii_uppercase();
                                        }
                                        for p in ["%MW10", "%QW0", "%QW1"] {
                                            if ui.selectable_label(self.draft.target_reg == p, p).clicked() {
                                                self.draft.target_reg = p.to_string();
                                            }
                                        }
                                    });
                                    ui.end_row();

                                    ui.label("Kp (×100):");
                                    ui.horizontal(|ui| {
                                        ui.add(
                                            egui::DragValue::new(&mut self.draft.fb.k[0])
                                                .range(0..=100_000)
                                                .speed(5),
                                        );
                                        ui.label(
                                            RichText::new(format!(
                                                "= {:.2}",
                                                self.draft.fb.k[0] as f64 / 100.0
                                            ))
                                            .monospace()
                                            .color(palette.accent_ok),
                                        );
                                    });
                                    ui.end_row();

                                    ui.label("Ki (×100, 1/s):");
                                    ui.horizontal(|ui| {
                                        ui.add(
                                            egui::DragValue::new(&mut self.draft.fb.k[1])
                                                .range(0..=100_000)
                                                .speed(2),
                                        );
                                        ui.label(
                                            RichText::new(format!(
                                                "= {:.2} /s",
                                                self.draft.fb.k[1] as f64 / 100.0
                                            ))
                                            .monospace()
                                            .color(palette.accent_ok),
                                        );
                                    });
                                    ui.end_row();

                                    ui.label("Kd (×100, s):");
                                    ui.horizontal(|ui| {
                                        ui.add(
                                            egui::DragValue::new(&mut self.draft.fb.k[2])
                                                .range(0..=100_000)
                                                .speed(2),
                                        );
                                        ui.label(
                                            RichText::new(format!(
                                                "= {:.2} s",
                                                self.draft.fb.k[2] as f64 / 100.0
                                            ))
                                            .monospace()
                                            .color(palette.accent_ok),
                                        );
                                    });
                                    ui.end_row();

                                    ui.label("Output Clamp Max (out_max):");
                                    ui.horizontal(|ui| {
                                        ui.add(
                                            egui::DragValue::new(&mut self.draft.fb.k[3])
                                                .range(1..=100_000)
                                                .speed(10),
                                        );
                                        for m in [255, 1000, 4095, 10000] {
                                            if ui
                                                .selectable_label(
                                                    self.draft.fb.k[3] == m,
                                                    format!("0..{}", m),
                                                )
                                                .clicked()
                                            {
                                                self.draft.fb.k[3] = m;
                                            }
                                        }
                                    });
                                    ui.end_row();
                                });
                        }

                        TypeTag::Scale => {
                            egui::Grid::new("scale_params_grid")
                                .num_columns(2)
                                .spacing([12.0, 6.0])
                                .show(ui, |ui| {
                                    ui.label("Input (IN):");
                                    ui.horizontal(|ui| {
                                        let r = ui.add(
                                            egui::TextEdit::singleline(&mut self.draft.fb.in1)
                                                .font(egui::TextStyle::Monospace)
                                                .hint_text("e.g. %IW0, %MW0")
                                                .desired_width(140.0),
                                        );
                                        if r.changed() {
                                            self.draft.fb.in1 =
                                                self.draft.fb.in1.to_ascii_uppercase();
                                        }
                                        for p in ["%IW0", "%IW1", "%MW0"] {
                                            if ui.selectable_label(self.draft.fb.in1 == p, p).clicked() {
                                                self.draft.fb.in1 = p.to_string();
                                            }
                                        }
                                    });
                                    ui.end_row();

                                    ui.label("Output Register (OUT):");
                                    ui.horizontal(|ui| {
                                        let r = ui.add(
                                            egui::TextEdit::singleline(&mut self.draft.target_reg)
                                                .font(egui::TextStyle::Monospace)
                                                .hint_text("e.g. %MW20, %QW0")
                                                .desired_width(140.0),
                                        );
                                        if r.changed() {
                                            self.draft.target_reg =
                                                self.draft.target_reg.to_ascii_uppercase();
                                        }
                                        for p in ["%MW20", "%MW21", "%QW0"] {
                                            if ui.selectable_label(self.draft.target_reg == p, p).clicked() {
                                                self.draft.target_reg = p.to_string();
                                            }
                                        }
                                    });
                                    ui.end_row();

                                    ui.label("Input Range [in_min .. in_max]:");
                                    ui.horizontal(|ui| {
                                        ui.add(
                                            egui::DragValue::new(&mut self.draft.fb.k[0])
                                                .range(-100_000..=100_000)
                                                .speed(5)
                                                .prefix("min: "),
                                        );
                                        ui.add(
                                            egui::DragValue::new(&mut self.draft.fb.k[1])
                                                .range(-100_000..=100_000)
                                                .speed(5)
                                                .prefix("max: "),
                                        );
                                        for (mn, mx, lbl) in [
                                            (0, 4095, "0..4095 (12b)"),
                                            (0, 1023, "0..1023 (10b)"),
                                            (0, 1000, "0..1000"),
                                        ] {
                                            if ui
                                                .selectable_label(
                                                    self.draft.fb.k[0] == mn
                                                        && self.draft.fb.k[1] == mx,
                                                    lbl,
                                                )
                                                .clicked()
                                            {
                                                self.draft.fb.k[0] = mn;
                                                self.draft.fb.k[1] = mx;
                                            }
                                        }
                                    });
                                    ui.end_row();

                                    ui.label("Output Range [out_min .. out_max]:");
                                    ui.horizontal(|ui| {
                                        ui.add(
                                            egui::DragValue::new(&mut self.draft.fb.k[2])
                                                .range(-100_000..=100_000)
                                                .speed(5)
                                                .prefix("min: "),
                                        );
                                        ui.add(
                                            egui::DragValue::new(&mut self.draft.fb.k[3])
                                                .range(-100_000..=100_000)
                                                .speed(5)
                                                .prefix("max: "),
                                        );
                                        for (mn, mx, lbl) in [
                                            (0, 1000, "0..1000"),
                                            (0, 100, "0..100%"),
                                            (-1000, 1000, "-1000..1000"),
                                        ] {
                                            if ui
                                                .selectable_label(
                                                    self.draft.fb.k[2] == mn
                                                        && self.draft.fb.k[3] == mx,
                                                    lbl,
                                                )
                                                .clicked()
                                            {
                                                self.draft.fb.k[2] = mn;
                                                self.draft.fb.k[3] = mx;
                                            }
                                        }
                                    });
                                    ui.end_row();
                                });
                            ui.add_space(4.0);
                            let [in_min, in_max, out_min, out_max] = self.draft.fb.k;
                            let mid_in = (in_min + in_max) / 2;
                            let mid_out = if in_max != in_min {
                                ((mid_in - in_min) as i64 * (out_max - out_min) as i64
                                    / (in_max - in_min) as i64
                                    + out_min as i64) as i32
                            } else {
                                out_min
                            };
                            ui.label(
                                RichText::new(format!(
                                    "Formula: OUT = (IN - {}) * ({} - {}) / ({} - {}) + {}   (e.g. IN={} → OUT={})",
                                    in_min, out_max, out_min, in_max, in_min, out_min, mid_in, mid_out
                                ))
                                .monospace()
                                .small()
                                .color(palette.accent_ok),
                            );
                        }

                        TypeTag::Limit => {
                            egui::Grid::new("limit_params_grid")
                                .num_columns(2)
                                .spacing([12.0, 6.0])
                                .show(ui, |ui| {
                                    ui.label("Input (IN):");
                                    ui.horizontal(|ui| {
                                        let r = ui.add(
                                            egui::TextEdit::singleline(&mut self.draft.fb.in1)
                                                .font(egui::TextStyle::Monospace)
                                                .hint_text("e.g. %MW20, %IW0")
                                                .desired_width(140.0),
                                        );
                                        if r.changed() {
                                            self.draft.fb.in1 =
                                                self.draft.fb.in1.to_ascii_uppercase();
                                        }
                                        for p in ["%MW20", "%MW0", "%IW0"] {
                                            if ui.selectable_label(self.draft.fb.in1 == p, p).clicked() {
                                                self.draft.fb.in1 = p.to_string();
                                            }
                                        }
                                    });
                                    ui.end_row();

                                    ui.label("Output Register (OUT):");
                                    ui.horizontal(|ui| {
                                        let r = ui.add(
                                            egui::TextEdit::singleline(&mut self.draft.target_reg)
                                                .font(egui::TextStyle::Monospace)
                                                .hint_text("e.g. %MW21, %QW0")
                                                .desired_width(140.0),
                                        );
                                        if r.changed() {
                                            self.draft.target_reg =
                                                self.draft.target_reg.to_ascii_uppercase();
                                        }
                                        for p in ["%MW21", "%MW1", "%QW0"] {
                                            if ui.selectable_label(self.draft.target_reg == p, p).clicked() {
                                                self.draft.target_reg = p.to_string();
                                            }
                                        }
                                    });
                                    ui.end_row();

                                    ui.label("Minimum Clamp (MIN):");
                                    ui.add(
                                        egui::DragValue::new(&mut self.draft.fb.k[0])
                                            .range(-100_000..=100_000)
                                            .speed(5),
                                    );
                                    ui.end_row();

                                    ui.label("Maximum Clamp (MAX):");
                                    ui.add(
                                        egui::DragValue::new(&mut self.draft.fb.k[1])
                                            .range(-100_000..=100_000)
                                            .speed(5),
                                    );
                                    ui.end_row();
                                });
                        }

                        TypeTag::Bitwise => {
                            let cur_op = if self.draft.math_op.trim().is_empty() {
                                "AND".to_string()
                            } else {
                                self.draft.math_op.trim().to_ascii_uppercase()
                            };
                            ui.horizontal_wrapped(|ui| {
                                ui.label("Bitwise Operator:");
                                for (op, lbl) in [
                                    ("AND", "AND (&)"),
                                    ("OR", "OR (|)"),
                                    ("XOR", "XOR (^)"),
                                    ("NOT", "NOT (~)"),
                                ] {
                                    if ui.selectable_label(cur_op == op, lbl).clicked() {
                                        self.draft.math_op = op.to_string();
                                    }
                                }
                            });
                            ui.add_space(6.0);
                            let is_not = self.draft.math_op.eq_ignore_ascii_case("NOT");
                            egui::Grid::new("bitwise_params_grid")
                                .num_columns(2)
                                .spacing([12.0, 6.0])
                                .show(ui, |ui| {
                                    ui.label("Source A (IN1):");
                                    ui.horizontal(|ui| {
                                        let r = ui.add(
                                            egui::TextEdit::singleline(&mut self.draft.fb.in1)
                                                .font(egui::TextStyle::Monospace)
                                                .hint_text("e.g. %MW0")
                                                .desired_width(140.0),
                                        );
                                        if r.changed() {
                                            self.draft.fb.in1 =
                                                self.draft.fb.in1.to_ascii_uppercase();
                                        }
                                        for p in ["%MW0", "%MW1", "16#FFFF"] {
                                            if ui.selectable_label(self.draft.fb.in1 == p, p).clicked() {
                                                self.draft.fb.in1 = p.to_string();
                                            }
                                        }
                                    });
                                    ui.end_row();

                                    if !is_not {
                                        ui.label("Source B / Mask (IN2):");
                                        ui.horizontal(|ui| {
                                            let r = ui.add(
                                                egui::TextEdit::singleline(&mut self.draft.fb.in2)
                                                    .font(egui::TextStyle::Monospace)
                                                    .hint_text("e.g. 16#00FF, %MW1")
                                                    .desired_width(140.0),
                                            );
                                            if r.changed() {
                                                self.draft.fb.in2 =
                                                    self.draft.fb.in2.to_ascii_uppercase();
                                            }
                                            for p in ["16#00FF", "16#FF00", "16#000F", "16#0001"] {
                                                if ui.selectable_label(self.draft.fb.in2 == p, p).clicked() {
                                                    self.draft.fb.in2 = p.to_string();
                                                }
                                            }
                                        });
                                        ui.end_row();
                                    }

                                    ui.label("Destination Register (OUT):");
                                    ui.horizontal(|ui| {
                                        let r = ui.add(
                                            egui::TextEdit::singleline(&mut self.draft.target_reg)
                                                .font(egui::TextStyle::Monospace)
                                                .hint_text("e.g. %MW1")
                                                .desired_width(140.0),
                                        );
                                        if r.changed() {
                                            self.draft.target_reg =
                                                self.draft.target_reg.to_ascii_uppercase();
                                            self.draft.name = self.draft.target_reg.clone();
                                        }
                                        for p in ["%MW1", "%MW2", "%MW3"] {
                                            if ui.selectable_label(self.draft.target_reg == p, p).clicked() {
                                                self.draft.target_reg = p.to_string();
                                                self.draft.name = p.to_string();
                                            }
                                        }
                                    });
                                    ui.end_row();
                                });
                        }

                        TypeTag::ShiftRegisters
                        | TypeTag::ShiftRegInbit
                        | TypeTag::ShiftRegReset
                        | TypeTag::ShiftLeft
                        | TypeTag::ShiftRight
                        | TypeTag::ShiftVariable => {
                            ui.horizontal_wrapped(|ui| {
                                ui.label("Shift Mode:");
                                for (t_type, lbl) in [
                                    (TypeTag::ShiftRegisters, "Array Shift (SFT)"),
                                    (TypeTag::ShiftRegInbit, "Bit Shift In (SBIT)"),
                                    (TypeTag::ShiftLeft, "Shift Left (SHL)"),
                                    (TypeTag::ShiftRight, "Shift Right (SHR)"),
                                    (TypeTag::ShiftVariable, "Shift Variable (SHV)"),
                                    (TypeTag::ShiftRegReset, "Shift Reset (SRST)"),
                                ] {
                                    if ui
                                        .selectable_label(self.draft.element_type == t_type, lbl)
                                        .clicked()
                                    {
                                        self.draft.element_type = t_type;
                                    }
                                }
                            });
                            ui.add_space(6.0);
                            egui::Grid::new("shift_params_grid")
                                .num_columns(2)
                                .spacing([12.0, 6.0])
                                .show(ui, |ui| {
                                    ui.label("Target / Base Word:");
                                    ui.horizontal(|ui| {
                                        let r = ui.add(
                                            egui::TextEdit::singleline(&mut self.draft.name)
                                                .font(egui::TextStyle::Monospace)
                                                .hint_text("e.g. %MW30")
                                                .desired_width(140.0),
                                        );
                                        if r.changed() {
                                            self.draft.name = self.draft.name.to_ascii_uppercase();
                                        }
                                        for p in ["%MW30", "%MW0", "%MW96"] {
                                            if ui.selectable_label(self.draft.name == p, p).clicked() {
                                                self.draft.name = p.to_string();
                                            }
                                        }
                                    });
                                    ui.end_row();

                                    match self.draft.element_type {
                                        TypeTag::ShiftRegisters => {
                                            ui.label("Stage Count (LEN, 1..16):");
                                            ui.add(
                                                egui::DragValue::new(&mut self.draft.preset_value)
                                                    .range(1..=16)
                                                    .speed(1),
                                            );
                                            ui.end_row();

                                            ui.label("Shift-In Source (IN):");
                                            ui.add(
                                                egui::TextEdit::singleline(&mut self.draft.fb.in1)
                                                    .font(egui::TextStyle::Monospace)
                                                    .hint_text("e.g. %MW0 or literal")
                                                    .desired_width(140.0),
                                            );
                                            ui.end_row();
                                        }
                                        TypeTag::ShiftRegInbit => {
                                            ui.label("Bit Input (IN_BIT):");
                                            ui.horizontal(|ui| {
                                                let r = ui.add(
                                                    egui::TextEdit::singleline(&mut self.draft.fb.in1)
                                                        .font(egui::TextStyle::Monospace)
                                                        .hint_text("e.g. %IX0.0, %MX0.0")
                                                        .desired_width(140.0),
                                                );
                                                if r.changed() {
                                                    self.draft.fb.in1 =
                                                        self.draft.fb.in1.to_ascii_uppercase();
                                                }
                                                for p in ["%IX0.0", "%IX0.1", "%MX0.0"] {
                                                    if ui.selectable_label(self.draft.fb.in1 == p, p).clicked() {
                                                        self.draft.fb.in1 = p.to_string();
                                                    }
                                                }
                                            });
                                            ui.end_row();
                                        }
                                        TypeTag::ShiftLeft | TypeTag::ShiftRight => {
                                            ui.label("Shift Count (N bits, 0..15):");
                                            ui.add(
                                                egui::DragValue::new(&mut self.draft.preset_value)
                                                    .range(0..=15)
                                                    .speed(1),
                                            );
                                            ui.end_row();
                                        }
                                        TypeTag::ShiftVariable => {
                                            ui.label("Shift Count Operand (COUNT):");
                                            ui.horizontal(|ui| {
                                                ui.add(
                                                    egui::TextEdit::singleline(&mut self.draft.fb.in1)
                                                        .font(egui::TextStyle::Monospace)
                                                        .hint_text("e.g. %MW31 (+SHL / -SHR)")
                                                        .desired_width(140.0),
                                                );
                                                ui.label(
                                                    RichText::new("(>0: SHL, <0: SHR)")
                                                        .small()
                                                        .color(palette.rung_text_dim),
                                                );
                                            });
                                            ui.end_row();
                                        }
                                        TypeTag::ShiftRegReset => {
                                            ui.label("Action:");
                                            ui.label(
                                                RichText::new("Clears target word to 0 when rung is TRUE")
                                                    .small()
                                                    .color(palette.rung_text_dim),
                                            );
                                            ui.end_row();
                                        }
                                        _ => {}
                                    }
                                });
                        }

                        TypeTag::UartSend => {
                            egui::Grid::new("uart_send_params_grid")
                                .num_columns(2)
                                .spacing([12.0, 6.0])
                                .show(ui, |ui| {
                                    ui.label("Source Operand / Byte (IN):");
                                    ui.horizontal(|ui| {
                                        let r = ui.add(
                                            egui::TextEdit::singleline(&mut self.draft.fb.in1)
                                                .font(egui::TextStyle::Monospace)
                                                .hint_text("e.g. %MW0, 65 ('A'), 16#41")
                                                .desired_width(150.0),
                                        );
                                        if r.changed() {
                                            self.draft.fb.in1 =
                                                self.draft.fb.in1.to_ascii_uppercase();
                                        }
                                        for (p, desc) in [
                                            ("%MW0", "%MW0"),
                                            ("65", "65 ('A')"),
                                            ("48", "48 ('0')"),
                                        ] {
                                            if ui.selectable_label(self.draft.fb.in1 == p, desc).clicked() {
                                                self.draft.fb.in1 = p.to_string();
                                            }
                                        }
                                    });
                                    ui.end_row();

                                    ui.label("Line Ending:");
                                    let mut crlf = self.draft.fb.mode != 0;
                                    if ui
                                        .checkbox(&mut crlf, "Append \\r\\n (CRLF) after byte")
                                        .changed()
                                    {
                                        self.draft.fb.mode = u8::from(crlf);
                                    }
                                    ui.end_row();
                                });
                        }

                        TypeTag::UartRecv => {
                            egui::Grid::new("uart_recv_params_grid")
                                .num_columns(2)
                                .spacing([12.0, 6.0])
                                .show(ui, |ui| {
                                    ui.label("Destination Register (OUT):");
                                    ui.horizontal(|ui| {
                                        let r = ui.add(
                                            egui::TextEdit::singleline(&mut self.draft.target_reg)
                                                .font(egui::TextStyle::Monospace)
                                                .hint_text("e.g. %MW1")
                                                .desired_width(150.0),
                                        );
                                        if r.changed() {
                                            self.draft.target_reg =
                                                self.draft.target_reg.to_ascii_uppercase();
                                        }
                                        for p in ["%MW1", "%MW2", "%MW3"] {
                                            if ui.selectable_label(self.draft.target_reg == p, p).clicked() {
                                                self.draft.target_reg = p.to_string();
                                            }
                                        }
                                    });
                                    ui.end_row();
                                });
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new("Dequeues 1 byte (0..255) into OUT and sets rung TRUE; writes -1 and sets FALSE if RX FIFO is empty.")
                                    .small()
                                    .color(palette.rung_text_dim),
                            );
                        }

                        TypeTag::Sr | TypeTag::Rs => {
                            ui.horizontal(|ui| {
                                ui.label("Bistable Function:");
                                if ui
                                    .selectable_label(
                                        self.draft.element_type == TypeTag::Sr,
                                        "SR (Set-Dominant)",
                                    )
                                    .clicked()
                                {
                                    self.draft.element_type = TypeTag::Sr;
                                }
                                if ui
                                    .selectable_label(
                                        self.draft.element_type == TypeTag::Rs,
                                        "RS (Reset-Dominant)",
                                    )
                                    .clicked()
                                {
                                    self.draft.element_type = TypeTag::Rs;
                                }
                            });
                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                let r_lbl = if self.draft.element_type == TypeTag::Sr {
                                    "Reset Input (R):"
                                } else {
                                    "Reset Input (R1):"
                                };
                                ui.label(r_lbl);
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.draft.target_reg)
                                        .font(egui::TextStyle::Monospace)
                                        .hint_text("e.g. %IX0.1")
                                        .desired_width(160.0),
                                );
                            });
                        }

                        TypeTag::Jump | TypeTag::Label => {
                            ui.horizontal(|ui| {
                                ui.label("Flow Control Type:");
                                if ui
                                    .selectable_label(
                                        self.draft.element_type == TypeTag::Jump,
                                        "JMP (Conditional Jump)",
                                    )
                                    .clicked()
                                {
                                    self.draft.element_type = TypeTag::Jump;
                                }
                                if ui
                                    .selectable_label(
                                        self.draft.element_type == TypeTag::Label,
                                        "LBL (Target Label)",
                                    )
                                    .clicked()
                                {
                                    self.draft.element_type = TypeTag::Label;
                                }
                            });
                        }

                        TypeTag::Comment => {
                            ui.label("Network / Element Comment Text:");
                            ui.add(
                                egui::TextEdit::singleline(&mut self.draft.comment)
                                    .desired_width(f32::INFINITY),
                            );
                        }

                        _ => {
                            ui.label("Standard instruction element.");
                        }
                    }
                });

                ui.add_space(10.0);

                // --- 4. Action Buttons Bar ---
                ui.horizontal(|ui| {
                    let apply_btn = egui::Button::new(
                        RichText::new("Apply Changes (Enter)")
                            .color(Color32::WHITE)
                            .strong(),
                    )
                    .fill(Color32::from_rgb(28, 135, 70))
                    .min_size(vec2(165.0, 30.0));

                    if ui.add(apply_btn).clicked() {
                        apply_clicked = true;
                    }

                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new("Delete Element").color(palette.accent_danger),
                            )
                            .min_size(vec2(120.0, 30.0)),
                        )
                        .clicked()
                    {
                        delete_clicked = true;
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(egui::Button::new("Cancel (Esc)").min_size(vec2(105.0, 30.0)))
                            .clicked()
                        {
                            cancel_clicked = true;
                        }
                    });
                });
            });

        if !open_flag || cancel_clicked {
            self.is_open = false;
        } else if delete_clicked {
            doc.selected_element_id = Some(self.target_id);
            doc.delete_selected_element();
            self.is_open = false;
        } else if apply_clicked {
            doc.push_undo_snapshot();
            let clean_name = self.draft.name.trim().replace(' ', "_");
            let clean_symbol = self.draft.symbol.trim().to_string();

            if let Some(target) = doc.element_by_id_mut(self.target_id) {
                let mut updated = self.draft.clone();
                if !clean_name.is_empty() {
                    updated.name = clean_name.clone();
                }
                updated.symbol = clean_symbol.clone();
                *target = updated;
            }

            // Global Symbol Table (Single Source of Truth) update & cascade propagate to all elements with this address
            if !clean_name.is_empty() {
                doc.set_global_symbol(&clean_name, &clean_symbol, Some(&self.draft.comment));
            }

            if let Some(r_idx) = self.rung_idx {
                if let Some(rung) = doc.program.rungs.get_mut(r_idx) {
                    rung.comment = self.rung_comment.trim().to_string();
                }
            }
            doc.is_dirty = true;
            self.is_open = false;
        }
    }

    fn draw_live_preview_banner(
        &self,
        ui: &mut Ui,
        global_contact_style: ContactStyle,
        global_coil_style: CoilStyle,
        palette: ThemePalette,
    ) {
        let width = ui.available_width();
        let height = 74.0;
        let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
        let p = ui.painter();

        p.rect_filled(rect, CornerRadius::same(8), palette.canvas_bg);
        p.rect_stroke(
            rect,
            CornerRadius::same(8),
            Stroke::new(1.5, palette.banner_border),
            StrokeKind::Inside,
        );

        // Left Schematic Preview Area
        let schem_w = (width * 0.34).clamp(185.0, 225.0);
        let schem_rect = Rect::from_min_max(
            rect.min,
            pos2(rect.min.x + schem_w, rect.max.y),
        );
        let c = schem_rect.center();
        let l_rail = schem_rect.min.x + 18.0;
        let r_rail = schem_rect.max.x - 18.0;

        // Mini L1 & L2 rails
        p.line_segment(
            [pos2(l_rail, rect.min.y + 12.0), pos2(l_rail, rect.max.y - 12.0)],
            Stroke::new(3.0, palette.l1_rail),
        );
        p.line_segment(
            [pos2(r_rail, rect.min.y + 12.0), pos2(r_rail, rect.max.y - 12.0)],
            Stroke::new(3.0, palette.l2_rail),
        );
        p.line_segment(
            [pos2(l_rail, c.y), pos2(c.x - 24.0, c.y)],
            Stroke::new(2.0, palette.wire_idle),
        );
        p.line_segment(
            [pos2(c.x + 24.0, c.y), pos2(r_rail, c.y)],
            Stroke::new(2.0, palette.wire_idle),
        );

        if self.draft.element_type == TypeTag::Contact {
            let active_style = global_contact_style;
            let is_nc = self.draft.contact_mode == ContactMode::NormallyClosed;
            let stroke_col = palette.accent_heading;
            let wire_col = palette.wire_idle;

            match active_style {
                ContactStyle::IecStandard => {
                    let gap = 10.0;
                    let plate_h = 13.0;
                    p.line_segment([pos2(c.x - 24.0, c.y), pos2(c.x - gap, c.y)], Stroke::new(2.0, wire_col));
                    p.line_segment([pos2(c.x + gap, c.y), pos2(c.x + 24.0, c.y)], Stroke::new(2.0, wire_col));
                    p.line_segment([pos2(c.x - gap, c.y - plate_h), pos2(c.x - gap, c.y + plate_h)], Stroke::new(2.6, stroke_col));
                    p.line_segment([pos2(c.x + gap, c.y - plate_h), pos2(c.x + gap, c.y + plate_h)], Stroke::new(2.6, stroke_col));
                    if is_nc {
                        p.line_segment([pos2(c.x - gap - 3.0, c.y + plate_h + 2.0), pos2(c.x + gap + 3.0, c.y - plate_h - 2.0)], Stroke::new(2.2, palette.accent_danger));
                    } else if self.draft.contact_mode == ContactMode::PositiveTransition {
                        p.text(c, Align2::CENTER_CENTER, "P", FontId::monospace(13.0), stroke_col);
                    } else if self.draft.contact_mode == ContactMode::NegativeTransition {
                        p.text(c, Align2::CENTER_CENTER, "N", FontId::monospace(13.0), stroke_col);
                    }
                }
                ContactStyle::JicSchematic => {
                    let gap = 15.0;
                    let term_r = 3.5;
                    let p_left = pos2(c.x - gap, c.y);
                    let p_right = pos2(c.x + gap, c.y);
                    p.line_segment([pos2(c.x - 24.0, c.y), pos2(p_left.x - term_r, c.y)], Stroke::new(2.0, wire_col));
                    p.line_segment([pos2(p_right.x + term_r, c.y), pos2(c.x + 24.0, c.y)], Stroke::new(2.0, wire_col));
                    p.circle_stroke(p_left, term_r, Stroke::new(2.0, stroke_col));
                    p.circle_stroke(p_right, term_r, Stroke::new(2.0, stroke_col));
                    if is_nc {
                        p.line_segment([pos2(p_left.x + term_r, c.y), pos2(p_right.x - term_r, c.y)], Stroke::new(2.6, stroke_col));
                        p.line_segment([pos2(c.x - 5.0, c.y + 6.0), pos2(c.x + 5.0, c.y - 6.0)], Stroke::new(2.0, palette.accent_danger));
                    } else {
                        p.line_segment([pos2(p_left.x + 1.0, c.y - 2.0), pos2(p_right.x - 2.0, c.y - 12.0)], Stroke::new(2.6, stroke_col));
                    }
                }
                ContactStyle::PushButton => {
                    let gap = 14.0;
                    let term_r = 3.5;
                    let p_left = pos2(c.x - gap, c.y);
                    let p_right = pos2(c.x + gap, c.y);
                    p.line_segment([pos2(c.x - 24.0, c.y), pos2(p_left.x - term_r, c.y)], Stroke::new(2.0, wire_col));
                    p.line_segment([pos2(p_right.x + term_r, c.y), pos2(c.x + 24.0, c.y)], Stroke::new(2.0, wire_col));
                    p.circle_stroke(p_left, term_r, Stroke::new(2.0, stroke_col));
                    p.circle_stroke(p_right, term_r, Stroke::new(2.0, stroke_col));
                    let bar_y = if is_nc { c.y - term_r } else { c.y - 8.0 };
                    let bar_col = if is_nc { palette.accent_ok } else { stroke_col };
                    p.line_segment([pos2(c.x - gap - 2.0, bar_y), pos2(c.x + gap + 2.0, bar_y)], Stroke::new(2.6, bar_col));
                    p.line_segment([pos2(c.x, bar_y), pos2(c.x, bar_y - 8.0)], Stroke::new(2.0, palette.rung_text_dim));
                    p.line_segment([pos2(c.x - 6.0, bar_y - 8.0), pos2(c.x + 6.0, bar_y - 8.0)], Stroke::new(3.0, palette.accent_warn));
                }
                ContactStyle::DinEuropean => {
                    let gap = 11.0;
                    let c_left = pos2(c.x - gap, c.y);
                    let c_right = pos2(c.x + gap, c.y);

                    p.line_segment([pos2(c.x - 24.0, c.y), c_left], Stroke::new(2.0, wire_col));
                    p.line_segment([c_right, pos2(c.x + 24.0, c.y)], Stroke::new(2.0, wire_col));

                    // Left hinge terminal dot
                    p.circle_filled(c_left, 3.0, stroke_col);

                    let stub_h = 7.5;
                    if is_nc {
                        let stub_bottom = pos2(c_right.x, c.y + stub_h);
                        p.line_segment([c_right, stub_bottom], Stroke::new(2.0, stroke_col));
                        p.line_segment([c_left, pos2(stub_bottom.x + 1.5, stub_bottom.y)], Stroke::new(2.4, stroke_col));
                    } else {
                        let stub_top = pos2(c_right.x, c.y - stub_h);
                        p.line_segment([c_right, stub_top], Stroke::new(2.0, stroke_col));
                        p.line_segment([c_left, pos2(c_right.x - 2.5, c.y - stub_h - 7.0)], Stroke::new(2.4, stroke_col));
                    }

                    let text_y = c.y + 11.0;
                    if self.draft.contact_mode == ContactMode::PositiveTransition {
                        p.text(pos2(c.x, text_y), Align2::CENTER_CENTER, "P", FontId::monospace(10.0), stroke_col);
                    } else if self.draft.contact_mode == ContactMode::NegativeTransition {
                        p.text(pos2(c.x, text_y), Align2::CENTER_CENTER, "N", FontId::monospace(10.0), stroke_col);
                    }
                }
                ContactStyle::ToggleSwitch => {
                    let sw_w = 42.0;
                    let sw_h = 19.0;
                    let sw_rect = Rect::from_center_size(c, vec2(sw_w, sw_h));
                    let knob_r = 7.0;
                    let (bg, knob_x, label, label_col) = if is_nc {
                        (palette.card_bg, sw_rect.min.x + knob_r + 3.0, "NC", palette.accent_danger)
                    } else {
                        (palette.card_bg, sw_rect.max.x - knob_r - 3.0, "NO", palette.accent_ok)
                    };
                    p.rect_filled(sw_rect, CornerRadius::same(9), bg);
                    p.rect_stroke(sw_rect, CornerRadius::same(9), Stroke::new(1.5, stroke_col), StrokeKind::Inside);
                    p.circle_filled(pos2(knob_x, c.y), knob_r, stroke_col);
                    p.text(
                        pos2(if is_nc { sw_rect.max.x - 11.0 } else { sw_rect.min.x + 11.0 }, c.y),
                        Align2::CENTER_CENTER,
                        label,
                        FontId::monospace(9.0),
                        label_col,
                    );
                }
            }
        } else if self.draft.element_type == TypeTag::Coil {
            let active_style = self.draft.coil_style.unwrap_or(global_coil_style);
            let is_memory = self.draft.name.trim_start_matches('%').starts_with('M');
            let stroke_col = if is_memory {
                Color32::from_rgb(190, 130, 255)
            } else {
                palette.accent_heading
            };
            let wire_col = palette.wire_idle;

            match active_style {
                CoilStyle::IecParentheses => {
                    let gap = 12.0;
                    let arc_half_h = 13.0;
                    let tip_dx = 4.5;

                    p.line_segment([pos2(c.x - 24.0, c.y), pos2(c.x - gap, c.y)], Stroke::new(2.0, wire_col));
                    p.line_segment([pos2(c.x + gap, c.y), pos2(c.x + 24.0, c.y)], Stroke::new(2.0, wire_col));

                    let lp0 = pos2(c.x - gap + tip_dx, c.y - arc_half_h);
                    let lp1 = pos2(c.x - gap - tip_dx, c.y);
                    let lp2 = pos2(c.x - gap + tip_dx, c.y + arc_half_h);
                    let mut prev_l = lp0;
                    for i in 1..=8 {
                        let t = i as f32 / 8.0;
                        let omt = 1.0 - t;
                        let cur = pos2(
                            omt * omt * lp0.x + 2.0 * omt * t * lp1.x + t * t * lp2.x,
                            omt * omt * lp0.y + 2.0 * omt * t * lp1.y + t * t * lp2.y,
                        );
                        p.line_segment([prev_l, cur], Stroke::new(2.4, stroke_col));
                        prev_l = cur;
                    }

                    let rp0 = pos2(c.x + gap - tip_dx, c.y - arc_half_h);
                    let rp1 = pos2(c.x + gap + tip_dx, c.y);
                    let rp2 = pos2(c.x + gap - tip_dx, c.y + arc_half_h);
                    let mut prev_r = rp0;
                    for i in 1..=8 {
                        let t = i as f32 / 8.0;
                        let omt = 1.0 - t;
                        let cur = pos2(
                            omt * omt * rp0.x + 2.0 * omt * t * rp1.x + t * t * rp2.x,
                            omt * omt * rp0.y + 2.0 * omt * t * rp1.y + t * t * rp2.y,
                        );
                        p.line_segment([prev_r, cur], Stroke::new(2.4, stroke_col));
                        prev_r = cur;
                    }

                    match self.draft.coil_mode {
                        CoilMode::Normal => {}
                        CoilMode::Negated => {
                            p.line_segment(
                                [pos2(c.x - 4.5, c.y + 9.0), pos2(c.x + 4.5, c.y - 9.0)],
                                Stroke::new(2.0, stroke_col),
                            );
                        }
                        CoilMode::SetLatch => {
                            p.text(c, Align2::CENTER_CENTER, "S", FontId::monospace(12.0), stroke_col);
                        }
                        CoilMode::ResetUnlatch => {
                            p.text(c, Align2::CENTER_CENTER, "R", FontId::monospace(12.0), stroke_col);
                        }
                        CoilMode::PositiveTransition => {
                            p.text(c, Align2::CENTER_CENTER, "P", FontId::monospace(12.0), stroke_col);
                        }
                        CoilMode::NegativeTransition => {
                            p.text(c, Align2::CENTER_CENTER, "N", FontId::monospace(12.0), stroke_col);
                        }
                    }
                }

                CoilStyle::CircleRelay => {
                    let coil_r = 14.0;
                    p.line_segment([pos2(c.x - 24.0, c.y), pos2(c.x - coil_r, c.y)], Stroke::new(2.0, wire_col));
                    p.line_segment([pos2(c.x + coil_r, c.y), pos2(c.x + 24.0, c.y)], Stroke::new(2.0, wire_col));
                    p.circle_filled(c, coil_r, palette.card_bg);
                    p.circle_stroke(c, coil_r, Stroke::new(2.4, stroke_col));

                    match self.draft.coil_mode {
                        CoilMode::Normal => {}
                        CoilMode::Negated => {
                            p.line_segment(
                                [
                                    pos2(c.x - coil_r * 0.707, c.y + coil_r * 0.707),
                                    pos2(c.x + coil_r * 0.707, c.y - coil_r * 0.707),
                                ],
                                Stroke::new(2.0, stroke_col),
                            );
                        }
                        CoilMode::SetLatch => {
                            p.text(c, Align2::CENTER_CENTER, "S", FontId::monospace(12.0), stroke_col);
                        }
                        CoilMode::ResetUnlatch => {
                            p.text(c, Align2::CENTER_CENTER, "R", FontId::monospace(12.0), stroke_col);
                        }
                        CoilMode::PositiveTransition => {
                            p.text(c, Align2::CENTER_CENTER, "P", FontId::monospace(12.0), stroke_col);
                        }
                        CoilMode::NegativeTransition => {
                            p.text(c, Align2::CENTER_CENTER, "N", FontId::monospace(12.0), stroke_col);
                        }
                    }
                }

                CoilStyle::NemaBox => {
                    let box_w = 34.0;
                    let box_h = 24.0;
                    let box_rect = Rect::from_center_size(c, vec2(box_w, box_h));
                    p.line_segment([pos2(c.x - 24.0, c.y), pos2(box_rect.min.x, c.y)], Stroke::new(2.0, wire_col));
                    p.line_segment([pos2(box_rect.max.x, c.y), pos2(c.x + 24.0, c.y)], Stroke::new(2.0, wire_col));
                    p.rect_filled(box_rect, CornerRadius::same(3), palette.card_bg);
                    p.rect_stroke(box_rect, CornerRadius::same(3), Stroke::new(2.0, stroke_col), StrokeKind::Inside);

                    let box_txt = match self.draft.coil_mode {
                        CoilMode::Normal => "M",
                        CoilMode::Negated => "/",
                        CoilMode::SetLatch => "S",
                        CoilMode::ResetUnlatch => "R",
                        CoilMode::PositiveTransition => "P",
                        CoilMode::NegativeTransition => "N",
                    };
                    p.text(c, Align2::CENTER_CENTER, box_txt, FontId::monospace(12.0), stroke_col);
                }

                CoilStyle::SolenoidValve => {
                    let sol_w = 36.0;
                    let sol_h = 24.0;
                    let sol_rect = Rect::from_center_size(c, vec2(sol_w, sol_h));
                    p.line_segment([pos2(c.x - 24.0, c.y), pos2(sol_rect.min.x, c.y)], Stroke::new(2.0, wire_col));
                    p.line_segment([pos2(sol_rect.max.x, c.y), pos2(c.x + 24.0, c.y)], Stroke::new(2.0, wire_col));
                    p.rect_filled(sol_rect, CornerRadius::same(2), palette.card_bg);
                    p.rect_stroke(sol_rect, CornerRadius::same(2), Stroke::new(2.0, stroke_col), StrokeKind::Inside);

                    p.line_segment(
                        [pos2(sol_rect.min.x, sol_rect.max.y), pos2(sol_rect.max.x, sol_rect.min.y)],
                        Stroke::new(1.6, palette.rung_text_dim),
                    );

                    let sol_txt = match self.draft.coil_mode {
                        CoilMode::Normal => "SOL",
                        CoilMode::Negated => "SOL/",
                        CoilMode::SetLatch => "S",
                        CoilMode::ResetUnlatch => "R",
                        CoilMode::PositiveTransition => "P",
                        CoilMode::NegativeTransition => "N",
                    };
                    p.text(
                        pos2(c.x, sol_rect.min.y + 6.5),
                        Align2::CENTER_CENTER,
                        sol_txt,
                        FontId::monospace(8.0),
                        stroke_col,
                    );
                }

                CoilStyle::PilotLight => {
                    let lamp_r = 14.0;
                    p.line_segment([pos2(c.x - 24.0, c.y), pos2(c.x - lamp_r, c.y)], Stroke::new(2.0, wire_col));
                    p.line_segment([pos2(c.x + lamp_r, c.y), pos2(c.x + 24.0, c.y)], Stroke::new(2.0, wire_col));
                    p.circle_filled(c, lamp_r, palette.card_bg);
                    p.circle_stroke(c, lamp_r, Stroke::new(2.2, stroke_col));

                    let d = lamp_r * 0.7071;
                    p.line_segment([pos2(c.x - d, c.y - d), pos2(c.x + d, c.y + d)], Stroke::new(1.8, stroke_col));
                    p.line_segment([pos2(c.x - d, c.y + d), pos2(c.x + d, c.y - d)], Stroke::new(1.8, stroke_col));

                    match self.draft.coil_mode {
                        CoilMode::Normal => {}
                        CoilMode::Negated => {
                            p.circle_filled(pos2(c.x + lamp_r - 2.0, c.y - lamp_r + 2.0), 3.5, palette.accent_warn);
                        }
                        CoilMode::SetLatch => {
                            p.text(pos2(c.x + lamp_r + 3.0, c.y - lamp_r + 3.0), Align2::LEFT_TOP, "S", FontId::monospace(9.0), stroke_col);
                        }
                        CoilMode::ResetUnlatch => {
                            p.text(pos2(c.x + lamp_r + 3.0, c.y - lamp_r + 3.0), Align2::LEFT_TOP, "R", FontId::monospace(9.0), stroke_col);
                        }
                        CoilMode::PositiveTransition => {
                            p.text(pos2(c.x + lamp_r + 3.0, c.y - lamp_r + 3.0), Align2::LEFT_TOP, "P", FontId::monospace(9.0), stroke_col);
                        }
                        CoilMode::NegativeTransition => {
                            p.text(pos2(c.x + lamp_r + 3.0, c.y - lamp_r + 3.0), Align2::LEFT_TOP, "N", FontId::monospace(9.0), stroke_col);
                        }
                    }
                }
            }
        } else {
            let icon = match self.draft.element_type {
                TypeTag::Coil => VectorIcon::Coil,
                TypeTag::Ton | TypeTag::Tof | TypeTag::Tp => VectorIcon::Timer,
                TypeTag::Ctu | TypeTag::Ctd | TypeTag::Ctud => VectorIcon::Counter,
                TypeTag::RTrig | TypeTag::FTrig => VectorIcon::MasterRelay,
                TypeTag::Compare
                | TypeTag::Move
                | TypeTag::Math
                | TypeTag::Adc
                | TypeTag::Pid
                | TypeTag::Scale
                | TypeTag::Limit
                | TypeTag::Bitwise => VectorIcon::FunctionBlock,
                TypeTag::ShiftRegisters
                | TypeTag::ShiftRegInbit
                | TypeTag::ShiftRegReset
                | TypeTag::ShiftLeft
                | TypeTag::ShiftRight
                | TypeTag::ShiftVariable => VectorIcon::ShiftReg,
                TypeTag::UartSend | TypeTag::UartRecv => VectorIcon::SerialComm,
                TypeTag::Comment => VectorIcon::Comment,
                _ => VectorIcon::MasterRelay,
            };

            let icon_box = Rect::from_center_size(c, vec2(42.0, 42.0));
            paint_icon(p, icon_box, icon, palette.accent_heading);
        }

        // Top Address Badge inside schematic preview
        p.text(
            pos2(c.x, rect.min.y + 13.0),
            Align2::CENTER_CENTER,
            &self.draft.name,
            FontId::monospace(12.0),
            palette.accent_warn,
        );
        p.text(
            pos2(c.x, rect.max.y - 12.0),
            Align2::CENTER_CENTER,
            &self.draft.symbol,
            FontId::proportional(10.5),
            palette.symbol_subtext,
        );

        // Right Summary Info Area
        let info_x = schem_rect.max.x + 12.0;
        p.text(
            pos2(info_x, rect.min.y + 18.0),
            Align2::LEFT_CENTER,
            format!("LIVE PREVIEW : {}", self.draft.element_type.display_name()),
            FontId::proportional(12.5),
            palette.banner_title,
        );

        let detail_summary = if let Some(fb_sum) = self.draft.fb_value_summary() {
            format!("{}  |  {}", self.draft.name, fb_sum)
        } else {
            match self.draft.element_type {
                TypeTag::Contact => {
                    let st = global_contact_style;
                    let mode_str = match self.draft.contact_mode {
                        ContactMode::NormallyOpen => "Normally Open (NO)",
                        ContactMode::NormallyClosed => "Normally Closed (NC)",
                        ContactMode::PositiveTransition => "Positive Transition (P)",
                        ContactMode::NegativeTransition => "Negative Transition (N)",
                    };
                    format!(
                        "Address: {}  |  Mode: {}  |  Style: {}",
                        self.draft.name,
                        mode_str,
                        st.short_name()
                    )
                }
                TypeTag::Coil => {
                    let st = self.draft.coil_style.unwrap_or(global_coil_style);
                    let norm_n = self.draft.name.trim().to_ascii_uppercase();
                    let is_mem = norm_n.starts_with("%MX")
                        || (norm_n.starts_with('M')
                            && norm_n.len() > 1
                            && norm_n[1..].chars().next().map_or(false, |c| c.is_ascii_digit()));
                    let role_tag = if is_mem { "🧠 Memory Relay (%M)" } else { "🔌 Physical Output (%Q)" };
                    format!(
                        "Target: {} ({})  |  Action: {:?}  |  Symbol: {}",
                        self.draft.name,
                        role_tag,
                        self.draft.coil_mode,
                        st.short_name()
                    )
                }
                TypeTag::Ton | TypeTag::Tof | TypeTag::Tp => format!(
                    "Timer: {}  |  PT = {} ms ({:.2}s)",
                    self.draft.name,
                    self.draft.preset_value,
                    self.draft.preset_value as f32 / 1000.0
                ),
                TypeTag::Ctu | TypeTag::Ctd | TypeTag::Ctud => format!(
                    "Counter: {}  |  PV = {}  |  SV = {}",
                    self.draft.name, self.draft.preset_value, self.draft.preset_value1
                ),
                TypeTag::RTrig | TypeTag::FTrig => format!(
                    "Edge Trigger: {} ({})",
                    self.draft.name,
                    if self.draft.element_type == TypeTag::RTrig { "R_TRIG Rising Edge" } else { "F_TRIG Falling Edge" }
                ),
                TypeTag::Compare => format!(
                    "Condition: {} {} {}",
                    self.draft.name, self.draft.compare_op, self.draft.preset_value
                ),
                TypeTag::Move => format!("Assignment: {} := {}", self.draft.name, self.draft.preset_value),
                TypeTag::Math => format!(
                    "Expression: {} {}= {}",
                    self.draft.name, self.draft.math_op, self.draft.preset_value
                ),
                _ => format!("Tag: {} ({})", self.draft.name, self.draft.symbol),
            }
        };

        p.text(
            pos2(info_x, rect.min.y + 39.0),
            Align2::LEFT_CENTER,
            detail_summary,
            FontId::monospace(11.0),
            palette.text_primary,
        );

        p.text(
            pos2(info_x, rect.min.y + 57.0),
            Align2::LEFT_CENTER,
            "Tip: Press [Enter] to Apply or [Esc] to Cancel",
            FontId::proportional(10.5),
            palette.rung_text_dim,
        );
    }
}

fn mode_select_card(
    ui: &mut Ui,
    card_w: f32,
    icon: VectorIcon,
    title: &str,
    subtitle: &str,
    selected: bool,
    palette: ThemePalette,
    on_click: impl FnOnce(),
) {
    let (rect, resp) = ui.allocate_exact_size(vec2(card_w, 48.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let hovered = resp.hovered();
        let bg = if selected {
            palette.rung_badge_sel_bg
        } else if hovered {
            palette.rung_badge_hov_bg
        } else {
            palette.card_bg
        };
        let border = if selected {
            palette.rung_badge_sel_border
        } else if hovered {
            palette.accent_heading
        } else {
            palette.rung_badge_border
        };
        let p = ui.painter();
        p.rect_filled(rect, CornerRadius::same(6), bg);
        p.rect_stroke(
            rect,
            CornerRadius::same(6),
            Stroke::new(if selected { 1.8 } else { 1.0 }, border),
            StrokeKind::Inside,
        );
        let icon_r = Rect::from_center_size(pos2(rect.min.x + 24.0, rect.center().y), vec2(26.0, 26.0));
        paint_icon(
            p,
            icon_r,
            icon,
            if selected {
                palette.rung_text_sel
            } else {
                palette.accent_heading
            },
        );
        p.text(
            pos2(rect.min.x + 46.0, rect.min.y + 16.0),
            Align2::LEFT_CENTER,
            title,
            FontId::proportional(12.0),
            if selected {
                palette.rung_text_sel
            } else {
                palette.text_primary
            },
        );
        p.text(
            pos2(rect.min.x + 46.0, rect.min.y + 33.0),
            Align2::LEFT_CENTER,
            subtitle,
            FontId::proportional(10.0),
            palette.rung_text_dim,
        );
    }
    if resp.clicked() {
        on_click();
    }
}

/// `true` if `addr` lies in the RETAIN area saved to Flash by the firmware
/// (`M64..M95` / `%MX8.0..%MX11.7`, `D96..D127` / `V96..V127` / `%MW96..%MW127`).
/// Delegates to the core helper [`IlParser::check_persistent_address`](my_canvas_core::IlParser::check_persistent_address).
pub fn is_retain_address(addr: &str) -> bool {
    !addr.trim().is_empty() && my_canvas_core::IlParser::check_persistent_address(addr).is_some()
}

#[cfg(test)]
mod retain_tests {
    use super::is_retain_address;

    #[test]
    fn retain_address_aliases() {
        for a in ["M64", "m95", "%MX8.0", "%MX11.7", "D96", "V127", "%MW96", "%mw127"] {
            assert!(is_retain_address(a), "{} should be RETAIN", a);
        }
        for a in ["", "M63", "M96", "%MX7.7", "%MX12.0", "D95", "D128", "%MW95", "%MW128", "%IX0.0", "T0"] {
            assert!(!is_retain_address(a), "{} should not be RETAIN", a);
        }
    }
}
