mod file_loading;

use egui::epaint::Shadow;
use egui::vec2;
use egui_notify::Toasts;
pub(crate) use file_loading::PendingFileKind;

use crate::ui::{backup_window, bootloader_window, firmware_window, keys_window, supported_window, top_bar};
use owtk_core::backup::ParsedBackup;
use owtk_core::bootloader::IdentifiedBootloader;
use owtk_core::crypto::CryptoKey;
use owtk_core::firmware::{FirmwareState, IdentifiedFirmware};
use owtk_core::patches::PatchEntry;

/// Tracks which single view is displayed on mobile.
#[derive(Default, Clone, Copy, PartialEq)]
pub(crate) enum MobileView {
    #[default]
    None,
    Firmware,
    Bootloader,
    Backup,
    Keys,
    SupportedPatches,
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct PatcherApp {
    pub(crate) crypto_keys: Option<Vec<CryptoKey>>,

    #[serde(skip)]
    pub(crate) pending_file_req: Option<(PendingFileKind, poll_promise::Promise<Option<Vec<u8>>>)>,

    #[serde(skip)]
    pub(crate) pending_save_req: Option<poll_promise::Promise<Option<()>>>,

    #[serde(skip)]
    pub(crate) firmware: Option<Vec<u8>>,

    #[serde(skip)]
    pub(crate) firmware_identity: Option<IdentifiedFirmware>,

    #[serde(skip)]
    pub(crate) patch_entries: Option<Vec<PatchEntry>>,

    /// Holds `(data, has_patches)` while the save-format prompt is visible.
    #[serde(skip)]
    pub(crate) save_format_prompt: Option<(Vec<u8>, bool)>,

    #[serde(skip)]
    pub(crate) show_supported_patches: bool,

    /// Board filter for the supported patches window.
    #[serde(skip)]
    pub(crate) supported_board_filter: Option<owtk_core::board::BoardGeneration>,

    #[serde(skip)]
    pub(crate) parsed_backup: Option<ParsedBackup>,

    #[serde(skip)]
    pub(crate) bootloader: Option<Vec<u8>>,

    #[serde(skip)]
    pub(crate) bootloader_identity: Option<IdentifiedBootloader>,

    #[serde(skip)]
    pub(crate) bootloader_patch_entries: Option<Vec<PatchEntry>>,

    #[serde(skip)]
    pub(crate) show_keys_window: bool,

    #[serde(skip)]
    pub(crate) show_mobile_menu: bool,

    #[serde(skip)]
    pub(crate) mobile_view: MobileView,

    #[serde(skip)]
    pub(crate) toasts: Toasts,
}

impl Default for PatcherApp {
    fn default() -> Self {
        Self {
            crypto_keys: None,
            pending_file_req: None,
            pending_save_req: None,
            firmware: None,
            firmware_identity: None,
            patch_entries: None,
            save_format_prompt: None,
            parsed_backup: None,
            bootloader: None,
            bootloader_identity: None,
            bootloader_patch_entries: None,
            show_supported_patches: false,
            supported_board_filter: None,
            show_keys_window: false,
            show_mobile_menu: false,
            mobile_view: MobileView::default(),
            toasts: Toasts::new()
                .with_shadow(Shadow { offset: [1, 2], blur: 8, spread: 2, color: egui::Color32::from_black_alpha(60) })
                .with_padding(vec2(12.0, 10.0)),
        }
    }
}

impl PatcherApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // ── Load JetBrains Mono as the UI font ──────────────────
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "JetBrainsMono".to_owned(),
            std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
                "../../assets/fonts/JetBrainsMono-Regular.ttf"
            ))),
        );
        // Use it as the highest-priority font for both families.
        fonts.families.entry(egui::FontFamily::Proportional).or_default().insert(0, "JetBrainsMono".to_owned());
        fonts.families.entry(egui::FontFamily::Monospace).or_default().insert(0, "JetBrainsMono".to_owned());

        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);

        cc.egui_ctx.set_fonts(fonts);

        if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    /// Builds a default file name for saving based on the firmware identity, target and patch state.
    pub(crate) fn default_save_name(&self, target_state: FirmwareState, is_patched: bool) -> String {
        if let Some(id) = &self.firmware_identity {
            let suffix = match target_state {
                FirmwareState::Encrypted => {
                    if is_patched {
                        "patched_enc"
                    } else {
                        "enc"
                    }
                }
                FirmwareState::Decrypted => {
                    if is_patched {
                        "patched_dec"
                    } else {
                        "dec"
                    }
                }
            };
            format!("{}_{}_{}.bin", id.descriptor.board.to_string().to_lowercase(), id.descriptor.version, suffix)
        } else {
            "firmware.bin".to_owned()
        }
    }

    /// Starts an async save-file dialog that writes `data` to the user-chosen path.
    pub(crate) fn start_save_request(&mut self, data: Vec<u8>, default_name: &str, title: &str, ctx: &egui::Context) {
        let name = default_name.to_owned();
        let title = title.to_owned();
        let repaint = ctx.clone();
        self.pending_save_req = Some(poll_promise::Promise::spawn_local(async move {
            let handle = rfd::AsyncFileDialog::new().set_title(&title).set_file_name(&name).save_file().await?;
            handle.write(&data).await.ok()?;
            repaint.request_repaint();
            Some(())
        }));
    }

    /// Polls the pending save request and cleans up when it completes.
    ///
    /// We intentionally don't show a toast here because `rfd` doesn't
    /// let us reliably distinguish a successful write from the user
    /// cancelling or dismissing the dialog.
    pub(crate) fn poll_save_request(&mut self) {
        let Some(promise) = &self.pending_save_req else {
            return;
        };

        if promise.ready().is_none() {
            return;
        }

        self.pending_save_req = None;
    }

    /// Mobile: render only the active view in a scroll area.
    fn show_mobile_central_panel(&mut self, ui: &mut egui::Ui) {
        // If the current view's data was unloaded, fall back to None.
        match self.mobile_view {
            MobileView::Firmware if self.firmware.is_none() => self.mobile_view = MobileView::None,
            MobileView::Bootloader if self.bootloader.is_none() => self.mobile_view = MobileView::None,
            MobileView::Backup if self.parsed_backup.is_none() => self.mobile_view = MobileView::None,
            MobileView::Keys if !self.show_keys_window => self.mobile_view = MobileView::None,
            MobileView::SupportedPatches if !self.show_supported_patches => self.mobile_view = MobileView::None,
            _ => {}
        }

        // Keep toggle flags in sync — turn them off when not the active view.
        if self.mobile_view != MobileView::Keys {
            self.show_keys_window = false;
        }
        if self.mobile_view != MobileView::SupportedPatches {
            self.show_supported_patches = false;
        }

        egui::ScrollArea::vertical().id_salt("mobile_scroll").auto_shrink([false, false]).show(ui, |ui| {
            match self.mobile_view {
                MobileView::None => {}
                MobileView::Firmware => firmware_window::show_inline(self, ui),
                MobileView::Bootloader => bootloader_window::show_inline(self, ui),
                MobileView::Backup => backup_window::show_inline(self, ui),
                MobileView::Keys => keys_window::show_inline(self, ui),
                MobileView::SupportedPatches => supported_window::show_inline(self, ui),
            }
        });
    }
}

impl eframe::App for PatcherApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_file_request();
        self.poll_save_request();

        let is_mobile = crate::ui::helpers::is_mobile(ctx);

        top_bar::show(self, ctx);

        egui::CentralPanel::default().show(ctx, |ui| {
            let painter = ui.painter();
            let center = ui.available_rect_before_wrap().center();
            let watermark_color = if ui.visuals().dark_mode {
                egui::Color32::from_white_alpha(10)
            } else {
                egui::Color32::from_black_alpha(10)
            };
            let screen_w = ui.ctx().viewport_rect().width();
            let watermark_size = (screen_w * 0.35).clamp(48.0, 160.0);
            painter.text(
                center,
                egui::Align2::CENTER_CENTER,
                "OWTK",
                egui::FontId::proportional(watermark_size),
                watermark_color,
            );
            painter.text(
                center + egui::vec2(0.0, watermark_size * 0.6),
                egui::Align2::CENTER_CENTER,
                "OneWheel Toolkit",
                egui::FontId::proportional(watermark_size * 0.15),
                watermark_color,
            );

            if is_mobile {
                self.show_mobile_central_panel(ui);
            } else {
                firmware_window::show(self, ui);
                bootloader_window::show(self, ui);
                backup_window::show(self, ui);
                keys_window::show(self, ui);
                supported_window::show(self, ui);
            }

            ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                ui.hyperlink_to(
                    format!("v{} · Source Code", env!("CARGO_PKG_VERSION")),
                    "https://github.com/dd41405e-3911-4647-9e57-dfa1603fee93/web-patcher",
                );

                egui::warn_if_debug_build(ui);
            });
        });

        // Must be last — renders toasts on top of everything.
        // Temporarily override the noninteractive bg_fill so toasts
        // get a more visible background (egui-notify uses this color).
        // NOTE: egui-notify's `Toasts::show()` takes `&Context`, not
        // `&mut Ui`, so we cannot use a scoped style override and must
        // swap the context-level style instead.
        let original_style = ctx.style();
        let mut toast_style = (*original_style).clone();
        toast_style.visuals.widgets.noninteractive.bg_fill =
            if toast_style.visuals.dark_mode { egui::Color32::from_gray(50) } else { egui::Color32::from_gray(230) };
        ctx.set_style(toast_style);

        self.toasts.show(ctx);

        // Restore the original style (one clone instead of re-reading
        // the modified style from ctx and patching the field back).
        ctx.set_style((*original_style).clone());
    }
}
