use std::collections::BTreeMap;

use super::helpers::card_frame;
use crate::app::{PatcherApp, PendingFileKind};
use owtk_core::board::BoardGeneration;
use owtk_core::crypto::CryptoIdentifier;
use owtk_core::firmware::known_firmwares;

/// For a given `CryptoIdentifier`, collects all compatible firmware descriptors
/// from the known firmware database, grouped by board generation and sorted by
/// version number.
fn compatible_firmwares(ident: &CryptoIdentifier) -> BTreeMap<BoardGeneration, Vec<u16>> {
    let mut map: BTreeMap<BoardGeneration, Vec<u16>> = BTreeMap::new();

    for fw in known_firmwares() {
        if fw.crypto_identifier == *ident {
            map.entry(fw.board).or_default().push(fw.version);
        }
    }

    // Sort versions within each board
    for versions in map.values_mut() {
        versions.sort_unstable();
        versions.dedup();
    }

    map
}

/// Desktop: renders the keys window as a floating `egui::Window`.
pub(crate) fn show(app: &mut PatcherApp, ui: &egui::Ui) {
    if !app.show_keys_window {
        return;
    }

    let mut open = app.show_keys_window;

    let max_width = (ui.ctx().viewport_rect().width() - 32.0).max(300.0);
    egui::Window::new("Encryption Keys").open(&mut open).default_width(520.0_f32.min(max_width)).show(ui.ctx(), |ui| {
        show_content(app, ui);
    });

    app.show_keys_window = open;
}

/// Mobile: renders the keys content directly into the provided `ui`.
pub(crate) fn show_inline(app: &mut PatcherApp, ui: &mut egui::Ui) {
    show_content(app, ui);
}

/// Shared content renderer used by both `show` (desktop) and `show_inline` (mobile).
#[expect(clippy::too_many_lines, reason = "UI rendering function — length is driven by layout code")]
fn show_content(app: &mut PatcherApp, ui: &mut egui::Ui) {
    // ── Load button + key count ──────────────────────────────────
    let key_count = app.crypto_keys.as_ref().map_or(0, std::vec::Vec::len);

    ui.add_space(4.0);

    ui.horizontal(|ui| {
        if ui.button("Load from Dump").clicked() {
            app.start_file_request(PendingFileKind::Dump, ui.ctx());
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let text = if key_count == 0 {
                "No keys loaded".to_owned()
            } else {
                format!("{key_count} key{} loaded", if key_count == 1 { "" } else { "s" })
            };
            ui.label(egui::RichText::new(text).weak());
        });
    });

    ui.add_space(4.0);
    ui.separator();
    ui.add_space(4.0);

    // ── Key list ────────────────────────────────────────────────
    //
    // Track a key index to remove so we can mutate after the
    // immutable iteration.
    let mut remove_index: Option<usize> = None;

    if let Some(keys) = &app.crypto_keys {
        if keys.is_empty() {
            ui.label("No keys found.");
        } else {
            egui::ScrollArea::vertical().id_salt("keys_window_scroll").auto_shrink([false, true]).show(ui, |ui| {
                for (i, key) in keys.iter().enumerate() {
                    if i > 0 {
                        ui.add_space(4.0);
                    }

                    let label = compatible_firmwares(&key.identifier)
                        .keys()
                        .map(|b| b.to_string())
                        .collect::<Vec<_>>()
                        .join(", ");

                    card_frame(ui).show(ui, |ui| {
                        let id = ui.make_persistent_id(format!("crypto_key_{i}"));
                        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, true)
                            .show_header(ui, |ui| {
                                ui.strong(&label);
                                ui.label(egui::RichText::new(format!("({})", key.identifier.method)).weak());

                                // Right-aligned remove button
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui
                                        .small_button(egui_phosphor::regular::TRASH)
                                        .on_hover_text("Remove key")
                                        .clicked()
                                    {
                                        remove_index = Some(i);
                                    }
                                });
                            })
                            .body(|ui| {
                                egui::Grid::new(format!("key_details_{i}")).num_columns(2).spacing([8.0, 4.0]).show(
                                    ui,
                                    |ui| {
                                        ui.label("Key:");
                                        ui.monospace(hex::encode(key.key));
                                        ui.end_row();

                                        if let Some(iv) = &key.iv {
                                            ui.label("IV:");
                                            ui.monospace(hex::encode(iv));
                                            ui.end_row();
                                        }
                                    },
                                );

                                let compat = compatible_firmwares(&key.identifier);
                                if !compat.is_empty() {
                                    ui.add_space(6.0);

                                    let compat_id = ui.make_persistent_id(format!("compat_fw_{i}"));
                                    egui::collapsing_header::CollapsingState::load_with_default_open(
                                        ui.ctx(),
                                        compat_id,
                                        false,
                                    )
                                    .show_header(ui, |ui| {
                                        ui.strong("Compatible Firmware");
                                    })
                                    .body(|ui| {
                                        egui::Grid::new(format!("compat_grid_{i}"))
                                            .num_columns(2)
                                            .spacing([12.0, 3.0])
                                            .show(ui, |ui| {
                                                for (board, versions) in &compat {
                                                    ui.label(board.to_string());
                                                    let ver_str = versions
                                                        .iter()
                                                        .map(|v| v.to_string())
                                                        .collect::<Vec<_>>()
                                                        .join(", ");
                                                    ui.add(egui::Label::new(ver_str).wrap());
                                                    ui.end_row();
                                                }
                                            });
                                    });
                                }
                            });
                    });
                }
            }); // ScrollArea
        }
    }

    // ── Apply deferred removal ──────────────────────────────────
    if let Some(idx) = remove_index
        && let Some(keys) = &mut app.crypto_keys
    {
        keys.remove(idx);
        if keys.is_empty() {
            app.crypto_keys = None;
        }
    }
}
