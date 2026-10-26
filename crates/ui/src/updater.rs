use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use egui::{Color32, CornerRadius, RichText, Stroke, Window};
use serde::{Deserialize, Serialize};
use crate::app::CanvasApp;

/// Binary executable name inside release archives.
pub const BIN_NAME: &str = "laduino-plc";

/// Default GitHub repository owner for official releases.
pub const DEFAULT_REPO_OWNER: &str = "pksofttech";

/// Default GitHub repository name for official releases.
pub const DEFAULT_REPO_NAME: &str = "uPLC_STUDIO";

/// JSON manifest format for a Private Server (`latest.json`).
///
/// Example `latest.json` hosted on a private HTTP/HTTPS server or S3 bucket:
/// ```json
/// {
///   "version": "0.2.0",
///   "notes": "- Added AutoCAD toolbar\n- Added Global Symbol Table",
///   "platforms": {
///     "x86_64-unknown-linux-gnu": {
///       "url": "https://updates.example.com/laduino-plc-v0.2.0-x86_64-unknown-linux-gnu.tar.gz"
///     },
///     "x86_64-pc-windows-msvc": {
///       "url": "https://updates.example.com/laduino-plc-v0.2.0-x86_64-pc-windows-msvc.zip"
///     }
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomReleaseManifest {
    pub version: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub pub_date: String,
    #[serde(default)]
    pub platforms: HashMap<String, CustomPlatformAsset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomPlatformAsset {
    pub url: String,
    #[serde(default)]
    pub sha256: Option<String>,
}

impl CustomReleaseManifest {
    /// Find the download URL matching the current target triple (e.g. `x86_64-unknown-linux-gnu`)
    /// or OS-arch shorthand (e.g. `linux-x86_64`, `windows-x86_64`).
    pub fn asset_for_target(&self, target: &str) -> Option<&CustomPlatformAsset> {
        if let Some(asset) = self.platforms.get(target) {
            return Some(asset);
        }
        let shorthand = if target.contains("linux") && target.contains("x86_64") {
            "linux-x86_64"
        } else if target.contains("windows") && target.contains("x86_64") {
            "windows-x86_64"
        } else if target.contains("apple") && target.contains("aarch64") {
            "macos-aarch64"
        } else if target.contains("apple") && target.contains("x86_64") {
            "macos-x86_64"
        } else {
            ""
        };
        if !shorthand.is_empty() {
            if let Some(asset) = self.platforms.get(shorthand) {
                return Some(asset);
            }
        }
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateStatus {
    Idle,
    Checking,
    UpToDate {
        current_version: String,
    },
    UpdateAvailable {
        current_version: String,
        latest_version: String,
        notes: String,
        download_url: Option<String>,
    },
    Downloading {
        target_version: String,
    },
    ReadyToRestart {
        new_version: String,
    },
    Error(String),
}

pub struct AppUpdater {
    pub show_window: bool,
    pub use_custom_server: bool,
    pub repo_owner: String,
    pub repo_name: String,
    pub custom_manifest_url: String,
    pub status: UpdateStatus,
    rx: Option<Receiver<UpdateStatus>>,
}

impl Default for AppUpdater {
    fn default() -> Self {
        Self {
            show_window: false,
            use_custom_server: false,
            repo_owner: DEFAULT_REPO_OWNER.to_string(),
            repo_name: DEFAULT_REPO_NAME.to_string(),
            custom_manifest_url: String::new(),
            status: UpdateStatus::Idle,
            rx: None,
        }
    }
}

impl AppUpdater {
    pub fn with_settings(
        use_custom_server: bool,
        repo_owner: String,
        repo_name: String,
        custom_manifest_url: String,
    ) -> Self {
        Self {
            show_window: false,
            use_custom_server,
            repo_owner,
            repo_name,
            custom_manifest_url,
            status: UpdateStatus::Idle,
            rx: None,
        }
    }

    pub fn current_version() -> &'static str {
        self_update::cargo_crate_version!()
    }

    pub fn current_target() -> &'static str {
        self_update::get_target()
    }

    /// Normalize a version string by stripping any leading `v` or `V`.
    pub fn clean_version(v: &str) -> &str {
        v.trim().trim_start_matches(['v', 'V'])
    }

    /// Compare whether `candidate` is strictly newer than `current`.
    pub fn is_newer_version(current: &str, candidate: &str) -> bool {
        let cur = Self::clean_version(current);
        let cand = Self::clean_version(candidate);
        self_update::version::bump_is_greater(cur, cand).unwrap_or(false)
    }

    /// Poll background worker channel once per UI frame.
    pub fn poll(&mut self) {
        if let Some(rx) = &self.rx {
            while let Ok(next) = rx.try_recv() {
                self.status = next;
            }
        }
    }

    /// Returns the version string if an update is known to be available or ready to restart.
    pub fn has_pending_update(&self) -> Option<&str> {
        match &self.status {
            UpdateStatus::UpdateAvailable { latest_version, .. } => Some(latest_version.as_str()),
            UpdateStatus::ReadyToRestart { new_version } => Some(new_version.as_str()),
            _ => None,
        }
    }

    /// Industrial safety check: disallow binary replacement while Online Monitoring or Simulation is active.
    pub fn safety_block_reason(online_mode: bool, sim_active: bool, is_dirty: bool) -> Option<&'static str> {
        if online_mode {
            Some("Cannot install update while Online Monitoring is active. Please disconnect from PLC first.")
        } else if sim_active {
            Some("Cannot install update while Simulation is running. Please stop simulation first.")
        } else if is_dirty {
            Some("Project has unsaved changes. Please save your Ladder program before updating.")
        } else {
            None
        }
    }

    /// Spawn a non-blocking background thread to check for updates from GitHub Releases or Private Server.
    pub fn check_for_updates(&mut self, ctx: egui::Context) {
        let (tx, rx): (Sender<UpdateStatus>, Receiver<UpdateStatus>) = mpsc::channel();
        self.rx = Some(rx);
        self.status = UpdateStatus::Checking;

        let use_custom = self.use_custom_server;
        let owner = self.repo_owner.trim().to_string();
        let name = self.repo_name.trim().to_string();
        let custom_url = self.custom_manifest_url.trim().to_string();

        std::thread::spawn(move || {
            let cur_ver = Self::current_version().to_string();
            let next_status = if use_custom {
                Self::check_custom_server(&cur_ver, &custom_url)
            } else {
                Self::check_github_releases(&cur_ver, &owner, &name)
            };

            let _ = tx.send(next_status);
            ctx.request_repaint();
        });
    }

    fn check_github_releases(cur_ver: &str, owner: &str, name: &str) -> UpdateStatus {
        if owner.is_empty() || name.is_empty() {
            return UpdateStatus::Error("GitHub Repository Owner and Name must not be empty.".to_string());
        }

        let res = self_update::backends::github::ReleaseList::configure()
            .repo_owner(owner)
            .repo_name(name)
            .build()
            .and_then(|list| list.fetch());

        match res {
            Ok(releases) => {
                if let Some(latest) = releases.first() {
                    let latest_ver = Self::clean_version(&latest.version).to_string();
                    if Self::is_newer_version(cur_ver, &latest_ver) {
                        let target = Self::current_target();
                        let download_url = latest
                            .asset_for(target, None)
                            .map(|a| a.download_url);
                        UpdateStatus::UpdateAvailable {
                            current_version: cur_ver.to_string(),
                            latest_version: latest_ver,
                            notes: latest.body.clone().unwrap_or_default(),
                            download_url,
                        }
                    } else {
                        UpdateStatus::UpToDate {
                            current_version: cur_ver.to_string(),
                        }
                    }
                } else {
                    UpdateStatus::UpToDate {
                        current_version: cur_ver.to_string(),
                    }
                }
            }
            Err(e) => UpdateStatus::Error(format!("Failed to query GitHub Releases ({}/{}): {}", owner, name, e)),
        }
    }

    fn check_custom_server(cur_ver: &str, manifest_url: &str) -> UpdateStatus {
        if manifest_url.is_empty() {
            return UpdateStatus::Error("Private Server Manifest URL is empty.".to_string());
        }

        let tmp_dir = match self_update::TempDir::new() {
            Ok(d) => d,
            Err(e) => return UpdateStatus::Error(format!("Failed to create temp dir: {}", e)),
        };
        let manifest_path = tmp_dir.path().join("latest.json");
        let mut manifest_file = match std::fs::File::create(&manifest_path) {
            Ok(f) => f,
            Err(e) => return UpdateStatus::Error(format!("Failed to create temp manifest file: {}", e)),
        };

        let dl = self_update::Download::from_url(manifest_url);
        if let Err(e) = dl.download_to(&mut manifest_file) {
            return UpdateStatus::Error(format!("Failed to download manifest from {}: {}", manifest_url, e));
        }

        let json_str = match std::fs::read_to_string(&manifest_path) {
            Ok(s) => s,
            Err(e) => return UpdateStatus::Error(format!("Failed to read manifest JSON: {}", e)),
        };

        let manifest: CustomReleaseManifest = match serde_json::from_str(&json_str) {
            Ok(m) => m,
            Err(e) => return UpdateStatus::Error(format!("Invalid manifest JSON format: {}", e)),
        };

        let latest_ver = Self::clean_version(&manifest.version).to_string();
        if Self::is_newer_version(cur_ver, &latest_ver) {
            let target = Self::current_target();
            let download_url = manifest.asset_for_target(target).map(|a| a.url.clone());
            UpdateStatus::UpdateAvailable {
                current_version: cur_ver.to_string(),
                latest_version: latest_ver,
                notes: manifest.notes,
                download_url,
            }
        } else {
            UpdateStatus::UpToDate {
                current_version: cur_ver.to_string(),
            }
        }
    }

    /// Spawn a background thread to download, extract, and atomically replace the current binary.
    pub fn perform_update(&mut self, ctx: egui::Context, target_version: String, custom_download_url: Option<String>) {
        let (tx, rx): (Sender<UpdateStatus>, Receiver<UpdateStatus>) = mpsc::channel();
        self.rx = Some(rx);
        self.status = UpdateStatus::Downloading {
            target_version: target_version.clone(),
        };

        let use_custom = self.use_custom_server;
        let owner = self.repo_owner.trim().to_string();
        let name = self.repo_name.trim().to_string();

        std::thread::spawn(move || {
            let next_status = if use_custom {
                match custom_download_url {
                    Some(url) => Self::install_from_direct_url(&target_version, &url),
                    None => UpdateStatus::Error(format!(
                        "No release package URL found in manifest for target '{}'",
                        Self::current_target()
                    )),
                }
            } else {
                let res = self_update::backends::github::Update::configure()
                    .repo_owner(&owner)
                    .repo_name(&name)
                    .bin_name(BIN_NAME)
                    .show_download_progress(false)
                    .show_output(false)
                    .no_confirm(true)
                    .current_version(Self::current_version())
                    .build()
                    .and_then(|u| u.update());

                match res {
                    Ok(status) => UpdateStatus::ReadyToRestart {
                        new_version: status.version().to_string(),
                    },
                    Err(e) => UpdateStatus::Error(format!("Update failed: {}", e)),
                }
            };

            let _ = tx.send(next_status);
            ctx.request_repaint();
        });
    }

    fn install_from_direct_url(target_version: &str, archive_url: &str) -> UpdateStatus {
        let tmp_dir = match self_update::TempDir::new() {
            Ok(d) => d,
            Err(e) => return UpdateStatus::Error(format!("Failed to create temp directory: {}", e)),
        };

        let archive_filename = archive_url
            .rsplit('/')
            .next()
            .filter(|s| !s.is_empty())
            .unwrap_or("update_pkg.tar.gz");
        let archive_path = tmp_dir.path().join(archive_filename);
        let mut archive_file = match std::fs::File::create(&archive_path) {
            Ok(f) => f,
            Err(e) => return UpdateStatus::Error(format!("Failed to create archive file: {}", e)),
        };

        let dl = self_update::Download::from_url(archive_url);
        if let Err(e) = dl.download_to(&mut archive_file) {
            return UpdateStatus::Error(format!("Download failed: {}", e));
        }

        let bin_name: PathBuf = if cfg!(windows) {
            PathBuf::from(format!("{}.exe", BIN_NAME))
        } else {
            PathBuf::from(BIN_NAME)
        };

        if let Err(e) = self_update::Extract::from_source(&archive_path).extract_file(tmp_dir.path(), &bin_name) {
            return UpdateStatus::Error(format!("Archive extraction failed: {}", e));
        }

        let new_exe = tmp_dir.path().join(&bin_name);
        let current_exe = match std::env::current_exe() {
            Ok(p) => p,
            Err(e) => return UpdateStatus::Error(format!("Cannot locate current executable: {}", e)),
        };

        let replacement_tmp = tmp_dir.path().join("laduino_old_bin");
        if let Err(e) = self_update::Move::from_source(&new_exe)
            .replace_using_temp(&replacement_tmp)
            .to_dest(&current_exe)
        {
            return UpdateStatus::Error(format!("Failed to replace executable: {}", e));
        }

        UpdateStatus::ReadyToRestart {
            new_version: target_version.to_string(),
        }
    }
}

/// Render the Software Update modal dialog in `egui`.
pub fn render_update_dialog(ctx: &egui::Context, app: &mut CanvasApp) {
    if !app.updater.show_window {
        return;
    }

    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.updater.show_window = false;
        return;
    }

    let palette = app.theme.palette();
    let mut open_win = app.updater.show_window;
    let mut close_win = false;
    let mut trigger_check = false;
    let mut trigger_install: Option<(String, Option<String>)> = None;
    let mut trigger_restart = false;
    let mut settings_changed = false;

    let sim_active = app.doc.simulator.is_active();
    let block_reason = AppUpdater::safety_block_reason(app.online_mode, sim_active, app.doc.is_dirty);

    Window::new("🔄 Software Update & Release Manager")
        .order(egui::Order::Foreground)
        .open(&mut open_win)
        .default_width(580.0)
        .min_width(520.0)
        .collapsible(false)
        .resizable(true)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            // 1. Current Version & Platform Header Card (No text overlapping)
            egui::Frame::new()
                .fill(if palette.is_light() {
                    Color32::from_rgb(241, 245, 249)
                } else {
                    palette.card_bg
                })
                .stroke(Stroke::new(1.0, palette.card_border))
                .corner_radius(CornerRadius::same(6))
                .inner_margin(egui::Margin::symmetric(14, 10))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("LaduinoPLC / uPLC Studio")
                                    .size(15.0)
                                    .strong()
                                    .color(palette.accent_heading),
                            );
                            ui.add_space(4.0);
                            egui::Frame::new()
                                .fill(palette.accent_ok.gamma_multiply(0.18))
                                .stroke(Stroke::new(1.0, palette.accent_ok.gamma_multiply(0.5)))
                                .corner_radius(CornerRadius::same(4))
                                .inner_margin(egui::Margin::symmetric(6, 2))
                                .show(ui, |ui| {
                                    ui.label(
                                        RichText::new(format!("v{}", AppUpdater::current_version()))
                                            .size(11.5)
                                            .strong()
                                            .color(palette.accent_ok),
                                    );
                                });
                        });
                        ui.add_space(3.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("Target Architecture:")
                                    .size(11.0)
                                    .color(palette.rung_text_dim),
                            );
                            ui.label(
                                RichText::new(AppUpdater::current_target())
                                    .monospace()
                                    .size(11.0)
                                    .color(palette.text_primary),
                            );
                        });
                    });
                });

            ui.add_space(6.0);

            // 2. Update Source Configuration (GitHub Releases vs Private Server)
            ui.group(|ui| {
                ui.set_width(ui.available_width());
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new("Update Source:")
                            .strong()
                            .size(11.5)
                            .color(palette.rung_text_dim),
                    );
                    if ui
                        .selectable_label(!app.updater.use_custom_server, "🐙 GitHub Releases")
                        .clicked()
                    {
                        app.updater.use_custom_server = false;
                        settings_changed = true;
                    }
                    if ui
                        .selectable_label(app.updater.use_custom_server, "🌐 Private Server (JSON)")
                        .clicked()
                    {
                        app.updater.use_custom_server = true;
                        settings_changed = true;
                    }
                });

                ui.separator();

                if !app.updater.use_custom_server {
                    egui::Grid::new("updater_github_cfg_grid")
                        .num_columns(2)
                        .spacing([10.0, 6.0])
                        .show(ui, |ui| {
                            ui.label(RichText::new("Repository Owner:").size(11.5));
                            if ui
                                .add(
                                    egui::TextEdit::singleline(&mut app.updater.repo_owner)
                                        .hint_text(DEFAULT_REPO_OWNER)
                                        .desired_width(240.0),
                                )
                                .changed()
                            {
                                settings_changed = true;
                            }
                            ui.end_row();

                            ui.label(RichText::new("Repository Name:").size(11.5));
                            if ui
                                .add(
                                    egui::TextEdit::singleline(&mut app.updater.repo_name)
                                        .hint_text(DEFAULT_REPO_NAME)
                                        .desired_width(240.0),
                                )
                                .changed()
                            {
                                settings_changed = true;
                            }
                            ui.end_row();
                        });
                } else {
                    ui.label(
                        RichText::new("Manifest URL (latest.json):")
                            .size(11.5)
                            .color(palette.rung_text_dim),
                    );
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut app.updater.custom_manifest_url)
                                .hint_text("https://updates.example.com/laduino-plc/latest.json")
                                .desired_width(ui.available_width()),
                        )
                        .changed()
                    {
                        settings_changed = true;
                    }
                }
            });

            ui.add_space(6.0);

            // 3. Industrial Safety Guard Banner (Structured 2-row layout to prevent overlapping)
            if let Some(reason) = block_reason {
                egui::Frame::new()
                    .fill(Color32::from_rgba_unmultiplied(230, 126, 34, 28))
                    .stroke(Stroke::new(1.0, palette.accent_warn))
                    .corner_radius(CornerRadius::same(6))
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("⚠ Safety Lock Active")
                                        .size(11.5)
                                        .strong()
                                        .color(palette.accent_warn),
                                );
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if app.online_mode {
                                        if ui.small_button("🛑 Stop Online").clicked() {
                                            app.stop_online_monitoring();
                                        }
                                    } else if sim_active {
                                        if ui.small_button("⏹ Stop Sim").clicked() {
                                            app.reset_run_mode();
                                        }
                                    } else if app.doc.is_dirty {
                                        if ui.small_button("💾 Save Project").clicked() {
                                            app.save_or_save_as();
                                        }
                                    }
                                });
                            });
                            ui.add_space(2.0);
                            ui.add(
                                egui::Label::new(
                                    RichText::new(reason)
                                        .size(11.0)
                                        .color(palette.text_primary),
                                )
                                .wrap(),
                            );
                        });
                    });
                ui.add_space(6.0);
            }

            // 4. Status & Release Notes Area
            let status_snapshot = app.updater.status.clone();
            egui::Frame::new()
                .fill(if palette.is_light() {
                    Color32::from_rgb(248, 250, 252)
                } else {
                    palette.pill_bg
                })
                .stroke(Stroke::new(1.0, palette.card_border.gamma_multiply(0.6)))
                .corner_radius(CornerRadius::same(6))
                .inner_margin(egui::Margin::symmetric(12, 10))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    match &status_snapshot {
                        UpdateStatus::Idle => {
                            ui.label(
                                RichText::new("Click 'Check for Updates' to query the release server.")
                                    .size(12.0)
                                    .color(palette.rung_text_dim),
                            );
                        }
                        UpdateStatus::Checking => {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label(
                                    RichText::new("Checking release server for updates...")
                                        .size(12.0)
                                        .strong()
                                        .color(palette.accent_heading),
                                );
                            });
                        }
                        UpdateStatus::UpToDate { current_version } => {
                            ui.label(
                                RichText::new(format!(
                                    "✔ You are running the latest version (v{}).",
                                    current_version
                                ))
                                .size(12.5)
                                .strong()
                                .color(palette.accent_ok),
                            );
                        }
                        UpdateStatus::UpdateAvailable {
                            current_version,
                            latest_version,
                            notes,
                            download_url,
                        } => {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(format!(
                                        "🎉 New Version Available: v{} (Current: v{})",
                                        latest_version, current_version
                                    ))
                                    .size(13.0)
                                    .strong()
                                    .color(palette.accent_ok),
                                )
                                .wrap(),
                            );
                            if let Some(url) = download_url {
                                ui.add_space(2.0);
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(format!("Package: {}", url))
                                            .monospace()
                                            .size(10.0)
                                            .color(palette.rung_text_dim),
                                    )
                                    .wrap(),
                                );
                            }
                            if !notes.trim().is_empty() {
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new("Release Notes:")
                                        .size(11.0)
                                        .strong()
                                        .color(palette.text_primary),
                                );
                                let mut notes_view = notes.clone();
                                egui::ScrollArea::vertical()
                                    .id_salt("update_release_notes_scroll")
                                    .max_height(120.0)
                                    .show(ui, |ui| {
                                        ui.add(
                                            egui::TextEdit::multiline(&mut notes_view)
                                                .desired_width(ui.available_width())
                                                .desired_rows(4)
                                                .interactive(false),
                                        );
                                    });
                            }
                        }
                        UpdateStatus::Downloading { target_version } => {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label(
                                    RichText::new(format!(
                                        "Downloading and installing v{}... Please wait.",
                                        target_version
                                    ))
                                    .size(12.0)
                                    .strong()
                                    .color(palette.accent_heading),
                                );
                            });
                        }
                        UpdateStatus::ReadyToRestart { new_version } => {
                            ui.label(
                                RichText::new(format!(
                                    "✅ Successfully updated to v{}!",
                                    new_version
                                ))
                                .size(13.0)
                                .strong()
                                .color(palette.accent_ok),
                            );
                            ui.label(
                                RichText::new("Please restart the application to launch the new version.")
                                    .size(11.5)
                                    .color(palette.text_primary),
                            );
                        }
                        UpdateStatus::Error(err_msg) => {
                            ui.label(
                                RichText::new("✖ Update Check / Install Error")
                                    .size(12.0)
                                    .strong()
                                    .color(palette.accent_danger),
                            );
                            ui.add_space(2.0);
                            ui.add(
                                egui::Label::new(
                                    RichText::new(err_msg)
                                        .size(11.0)
                                        .color(palette.text_primary),
                                )
                                .wrap(),
                            );
                        }
                    }
                });

            ui.add_space(8.0);

            // 5. Bottom Action Bar (Clean layout with safety margins)
            ui.horizontal(|ui| {
                let is_busy = matches!(
                    status_snapshot,
                    UpdateStatus::Checking | UpdateStatus::Downloading { .. }
                );

                let check_btn = egui::Button::new(
                    RichText::new("🔍 Check for Updates")
                        .size(12.0)
                        .strong(),
                )
                .min_size(egui::vec2(140.0, 28.0));

                if ui.add_enabled(!is_busy, check_btn).clicked() {
                    trigger_check = true;
                }

                if let UpdateStatus::UpdateAvailable {
                    latest_version,
                    download_url,
                    ..
                } = &status_snapshot
                {
                    let can_install = block_reason.is_none() && !is_busy;
                    let install_btn = egui::Button::new(
                        RichText::new(format!("⬇ Install v{}", latest_version))
                            .size(12.0)
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(Color32::from_rgb(24, 130, 90))
                    .min_size(egui::vec2(140.0, 28.0));

                    let resp = ui.add_enabled(can_install, install_btn);
                    if let Some(reason) = block_reason {
                        resp.on_disabled_hover_text(reason);
                    } else if resp.clicked() {
                        trigger_install = Some((latest_version.clone(), download_url.clone()));
                    }
                }

                if let UpdateStatus::ReadyToRestart { new_version } = &status_snapshot {
                    let can_restart = block_reason.is_none();
                    let restart_btn = egui::Button::new(
                        RichText::new(format!("🔄 Restart into v{}", new_version))
                            .size(12.0)
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(Color32::from_rgb(14, 116, 170))
                    .min_size(egui::vec2(150.0, 28.0));

                    if ui.add_enabled(can_restart, restart_btn).clicked() {
                        trigger_restart = true;
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let close_btn = egui::Button::new(RichText::new("Close").size(12.0))
                        .min_size(egui::vec2(72.0, 28.0));
                    if ui.add_enabled(!is_busy, close_btn).clicked() {
                        close_win = true;
                    }
                });
            });
        });

    if settings_changed {
        app.save_settings();
    }
    if trigger_check {
        app.updater.check_for_updates(ctx.clone());
    }
    if let Some((ver, url)) = trigger_install {
        app.updater.perform_update(ctx.clone(), ver, url);
    }
    if trigger_restart {
        if let Ok(exe) = std::env::current_exe() {
            let _ = std::process::Command::new(exe).spawn();
            app.allow_exit = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
    if !open_win || close_win {
        app.updater.show_window = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison_and_cleaning() {
        assert_eq!(AppUpdater::clean_version("v0.2.0"), "0.2.0");
        assert_eq!(AppUpdater::clean_version("V1.0.3 "), "1.0.3");
        assert!(AppUpdater::is_newer_version("0.1.0", "0.1.1"));
        assert!(AppUpdater::is_newer_version("0.1.0", "v0.2.0"));
        assert!(!AppUpdater::is_newer_version("0.2.0", "0.2.0"));
        assert!(!AppUpdater::is_newer_version("0.2.0", "0.1.9"));
    }

    #[test]
    fn test_custom_manifest_parsing_and_target_lookup() {
        let sample_json = r#"{
            "version": "v0.2.0",
            "notes": "- AutoCAD style toolbar\n- Self-update support",
            "platforms": {
                "x86_64-unknown-linux-gnu": {
                    "url": "https://updates.example.com/laduino-plc-v0.2.0-x86_64-unknown-linux-gnu.tar.gz"
                },
                "windows-x86_64": {
                    "url": "https://updates.example.com/laduino-plc-v0.2.0-x86_64-pc-windows-msvc.zip"
                }
            }
        }"#;
        let manifest: CustomReleaseManifest = serde_json::from_str(sample_json).unwrap();
        assert_eq!(AppUpdater::clean_version(&manifest.version), "0.2.0");
        assert!(manifest.asset_for_target("x86_64-unknown-linux-gnu").is_some());
        assert!(manifest.asset_for_target("x86_64-pc-windows-msvc").is_some());
        assert!(manifest.asset_for_target("unknown-target").is_none());
    }

    #[test]
    fn test_industrial_safety_block_reason() {
        assert!(AppUpdater::safety_block_reason(true, false, false).is_some());
        assert!(AppUpdater::safety_block_reason(false, true, false).is_some());
        assert!(AppUpdater::safety_block_reason(false, false, true).is_some());
        assert!(AppUpdater::safety_block_reason(false, false, false).is_none());
    }
}
