//! Find / Replace window (Ctrl+F, F3 / Shift+F3).
//!
//! Searches element `name / symbol / comment / target_reg / fb.in1 / fb.in2`
//! via [`Document::find_text`](my_canvas_core::Document::find_text) and supports
//! program-wide Replace All (single undo step).

use egui::{RichText, Window};
use my_canvas_core::document::{FindHit, FindOptions};

use crate::app::CanvasApp;

/// UI state of the Find window.
#[derive(Debug, Default)]
pub struct FindState {
    /// Window visible.
    pub open: bool,
    /// Text to find.
    pub query: String,
    /// Replacement text for Replace All.
    pub replace: String,
    /// Match case / Whole word.
    pub opts: FindOptions,
    /// Latest results (refreshed every frame while the window is open).
    pub results: Vec<FindHit>,
    /// Index into `results` of the current hit.
    pub cursor: Option<usize>,
    /// Focus the query field on the next frame.
    pub request_focus: bool,
    /// Feedback line (e.g. "Replaced 3 occurrence(s)").
    pub status: String,
}

impl FindState {
    /// Re-run the search against the current document.
    pub fn refresh(&mut self, doc: &my_canvas_core::Document) {
        self.results = doc.find_text(&self.query, self.opts);
        if let Some(c) = self.cursor {
            if c >= self.results.len() {
                self.cursor = if self.results.is_empty() { None } else { Some(self.results.len() - 1) };
            }
        }
    }

    /// Step the cursor by `delta` (wrapping). Returns the new index.
    pub fn step(&mut self, delta: isize) -> Option<usize> {
        let n = self.results.len();
        if n == 0 {
            self.cursor = None;
            return None;
        }
        let next = match self.cursor {
            None if delta >= 0 => 0,
            None => n - 1,
            Some(c) => (c as isize + delta).rem_euclid(n as isize) as usize,
        };
        self.cursor = Some(next);
        Some(next)
    }
}

/// Render the floating Find window (non-modal).
pub fn render(ctx: &egui::Context, app: &mut CanvasApp) {
    if !app.find.open {
        return;
    }
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.find.open = false;
        return;
    }
    app.find.refresh(&app.doc);

    let palette = app.theme.palette();
    let mut open = app.find.open;
    let mut goto: Option<usize> = None;
    let mut do_replace = false;
    let mut step: Option<isize> = None;

    Window::new("Find / Replace")
        .open(&mut open)
        .default_size([540.0, 360.0])
        .min_width(460.0)
        .collapsible(false)
        .resizable(true)
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
            let st = &mut app.find;

            // Search / Replace inputs + options + action buttons card
            egui::Frame::group(ui.style())
                .fill(palette.card_bg)
                .stroke(egui::Stroke::new(1.0, palette.card_border))
                .corner_radius(egui::CornerRadius::same(6))
                .inner_margin(egui::Margin::symmetric(10, 8))
                .show(ui, |ui| {
                    egui::Grid::new("find_grid")
                        .num_columns(2)
                        .min_col_width(58.0)
                        .spacing([8.0, 6.0])
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new("Find:")
                                    .size(11.5)
                                    .color(palette.rung_text_dim),
                            );
                            let resp = ui.add(
                                egui::TextEdit::singleline(&mut st.query)
                                    .hint_text("variable / address / symbol / comment")
                                    .desired_width(ui.available_width()),
                            );
                            if st.request_focus {
                                resp.request_focus();
                                st.request_focus = false;
                            }
                            if resp.changed() {
                                st.cursor = None;
                                st.status.clear();
                            }
                            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                step = Some(if ui.input(|i| i.modifiers.shift) { -1 } else { 1 });
                                resp.request_focus();
                            }
                            ui.end_row();

                            ui.label(
                                RichText::new("Replace:")
                                    .size(11.5)
                                    .color(palette.rung_text_dim),
                            );
                            ui.add(
                                egui::TextEdit::singleline(&mut st.replace)
                                    .hint_text("replacement text")
                                    .desired_width(ui.available_width()),
                            );
                            ui.end_row();
                        });

                    ui.add_space(4.0);

                    ui.horizontal(|ui| {
                        ui.checkbox(&mut st.opts.match_case, "Match case");
                        ui.checkbox(&mut st.opts.whole_word, "Whole word");

                        // Subtle match summary / status badge
                        let badge_text = if !st.status.is_empty() {
                            Some((st.status.clone(), palette.accent_ok))
                        } else if st.query.is_empty() {
                            None
                        } else if st.results.is_empty() {
                            Some(("No matches".to_string(), palette.accent_warn))
                        } else if let Some(c) = st.cursor {
                            Some((format!("{} / {}", c + 1, st.results.len()), palette.text_primary))
                        } else {
                            Some((format!("{} found", st.results.len()), palette.rung_text_dim))
                        };

                        if let Some((text, color)) = badge_text {
                            egui::Frame::new()
                                .fill(palette.pill_bg)
                                .stroke(egui::Stroke::new(1.0, palette.card_border))
                                .corner_radius(egui::CornerRadius::same(4))
                                .inner_margin(egui::Margin::symmetric(6, 2))
                                .show(ui, |ui| {
                                    ui.label(RichText::new(text).size(10.5).strong().color(color));
                                });
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let replace_btn = egui::Button::new("Replace All")
                                .corner_radius(egui::CornerRadius::same(4));
                            if ui
                                .add_enabled(
                                    !st.query.is_empty() && !st.results.is_empty(),
                                    replace_btn,
                                )
                                .clicked()
                            {
                                do_replace = true;
                            }

                            let next_btn = egui::Button::new("Next >")
                                .corner_radius(egui::CornerRadius::same(4));
                            if ui
                                .add(next_btn)
                                .on_hover_text("Next match [F3]")
                                .clicked()
                            {
                                step = Some(1);
                            }

                            let prev_btn = egui::Button::new("< Prev")
                                .corner_radius(egui::CornerRadius::same(4));
                            if ui
                                .add(prev_btn)
                                .on_hover_text("Previous match [Shift+F3]")
                                .clicked()
                            {
                                step = Some(-1);
                            }
                        });
                    });
                });

            // Scrollable striped results table card
            egui::Frame::group(ui.style())
                .fill(palette.card_bg)
                .stroke(egui::Stroke::new(1.0, palette.card_border))
                .corner_radius(egui::CornerRadius::same(6))
                .inner_margin(egui::Margin::symmetric(8, 6))
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            egui::Grid::new("find_results")
                                .num_columns(4)
                                .striped(true)
                                .spacing([12.0, 4.0])
                                .show(ui, |ui| {
                                    ui.label(
                                        RichText::new("Network #")
                                            .size(11.0)
                                            .strong()
                                            .color(palette.rung_text_dim),
                                    );
                                    ui.label(
                                        RichText::new("Type")
                                            .size(11.0)
                                            .strong()
                                            .color(palette.rung_text_dim),
                                    );
                                    ui.label(
                                        RichText::new("Field")
                                            .size(11.0)
                                            .strong()
                                            .color(palette.rung_text_dim),
                                    );
                                    ui.label(
                                        RichText::new("Text")
                                            .size(11.0)
                                            .strong()
                                            .color(palette.rung_text_dim),
                                    );
                                    ui.end_row();
                                    for (i, hit) in st.results.iter().enumerate() {
                                        let selected = st.cursor == Some(i);
                                        if ui
                                            .selectable_label(
                                                selected,
                                                format!("{:02}", hit.rung_idx + 1),
                                            )
                                            .clicked()
                                        {
                                            goto = Some(i);
                                        }
                                        ui.label(hit.element_type.display_name());
                                        ui.label(hit.field.label());
                                        if ui
                                            .selectable_label(
                                                selected,
                                                RichText::new(&hit.text).monospace(),
                                            )
                                            .clicked()
                                        {
                                            goto = Some(i);
                                        }
                                        ui.end_row();
                                    }
                                });
                        });
                });
        });

    app.find.open = open;

    if do_replace {
        let n = app.doc.replace_all(&app.find.query, &app.find.replace, app.find.opts);
        app.find.status = format!("Replaced {} occurrence(s)", n);
        app.find.cursor = None;
        app.find.refresh(&app.doc);
    }
    if let Some(d) = step {
        app.find_step(d);
    } else if let Some(i) = goto {
        app.find.cursor = Some(i);
        app.goto_find_result(i);
    }
}
