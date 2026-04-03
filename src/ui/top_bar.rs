use crate::app::{MobileView, PatcherApp, PendingFileKind};

use super::helpers::is_mobile;

/// Shows the top navigation bar.
///
/// Layout (wide, >=600px):
///   [OWTK]  |  [Load File] [Encryption Keys]  ...  [Supported Patches]  [Theme]
///
/// Layout (narrow, <600px):
///   [OWTK]  ...  [Hamburger]
///   (tapping hamburger opens a vertical dropdown with all actions)
pub(crate) fn show(app: &mut PatcherApp, ctx: &egui::Context) {
    let mobile = is_mobile(ctx);

    egui::TopBottomPanel::top("top_bar")
        .frame(
            egui::Frame::new()
                .fill(ctx.style().visuals.panel_fill)
                .inner_margin(egui::Margin::symmetric(8, if mobile { 8 } else { 4 })),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.visuals_mut().button_frame = false;
                ui.spacing_mut().item_spacing.x = 8.0;

                if mobile {
                    // ── Mobile: title + hamburger ────────────────────
                    ui.label(egui::RichText::new("OWTK").strong().size(18.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(egui::RichText::new(egui_phosphor::regular::LIST).size(18.0)).clicked() {
                            app.show_mobile_menu = !app.show_mobile_menu;
                        }
                    });
                } else {
                    // ── Desktop: inline buttons ────────────────────
                    let load_btn = egui::Button::new(
                        egui::RichText::new("Load File").strong().color(egui::Color32::from_rgb(70, 140, 220)),
                    );
                    let load_response = ui.add(load_btn);
                    egui::Popup::menu(&load_response).show(|ui| {
                        if ui.button("Firmware").clicked() {
                            app.start_file_request(PendingFileKind::Firmware, ctx);
                        }

                        if ui.button("Bootloader").clicked() {
                            app.start_file_request(PendingFileKind::Bootloader, ctx);
                        }

                        ui.separator();

                        if ui.button("Backup").clicked() {
                            app.start_file_request(PendingFileKind::Backup, ctx);
                        }
                    });

                    ui.separator();

                    if ui.selectable_label(app.show_keys_window, "Encryption Keys").clicked() {
                        app.show_keys_window = !app.show_keys_window;
                    }

                    ui.separator();

                    if ui.selectable_label(app.show_supported_patches, "Supported Patches").clicked() {
                        app.show_supported_patches = !app.show_supported_patches;
                    }

                }
            });
        });

    // ── Mobile dropdown menu ───────────────────────────────────────
    if mobile && app.show_mobile_menu {
        egui::TopBottomPanel::top("mobile_menu")
            .frame(
                egui::Frame::new()
                    .fill(ctx.style().visuals.panel_fill)
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .stroke(ctx.style().visuals.widgets.noninteractive.bg_stroke),
            )
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    let btn_size = egui::vec2(ui.available_width(), 36.0);

                    // ── Load File items ──────────────────────────
                    let mut load_button = |ui: &mut egui::Ui, label: &str, kind: PendingFileKind| {
                        let button = egui::Button::new(
                            egui::RichText::new(label).strong().color(egui::Color32::from_rgb(70, 140, 220)),
                        );

                        if ui.add_sized(btn_size, button).clicked() {
                            app.start_file_request(kind, ctx);
                            app.show_mobile_menu = false;
                        }
                    };

                    load_button(ui, "Load Firmware", PendingFileKind::Firmware);
                    load_button(ui, "Load Bootloader", PendingFileKind::Bootloader);
                    load_button(ui, "Load Backup", PendingFileKind::Backup);

                    // ── View items ───────────────────────────────
                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);

                    if ui
                        .add_sized(btn_size, egui::Button::new("Encryption Keys").selected(app.show_keys_window))
                        .clicked()
                    {
                        app.show_keys_window = !app.show_keys_window;
                        app.mobile_view = if app.show_keys_window { MobileView::Keys } else { MobileView::None };
                        app.show_mobile_menu = false;
                    }

                    if ui
                        .add_sized(
                            btn_size,
                            egui::Button::new("Supported Patches").selected(app.show_supported_patches),
                        )
                        .clicked()
                    {
                        app.show_supported_patches = !app.show_supported_patches;
                        app.mobile_view =
                            if app.show_supported_patches { MobileView::SupportedPatches } else { MobileView::None };
                        app.show_mobile_menu = false;
                    }
                });
            });
    }
}
