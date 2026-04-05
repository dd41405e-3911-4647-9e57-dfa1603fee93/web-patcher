use std::collections::{BTreeMap, BTreeSet};

use super::helpers::card_frame;
use crate::app::PatcherApp;
use owtk_core::board::BoardGeneration;
use owtk_core::patches::types::{PatchDefinition, PatchTarget, ScriptParamKind};
use owtk_core::patches::{all_patches_grouped, scripting};

/// A patch entry with board availability and per-board version info.
struct PatchInfo<'a> {
    name: &'a str,
    description: &'a str,
    id: &'a str,
    target: PatchTarget,
    experimental: bool,
    /// Board generation → set of versions.
    boards: BTreeMap<BoardGeneration, BTreeSet<u16>>,
}

/// Collects every patch definition into a flat map keyed by patch id,
/// merging board generations and firmware versions.
fn collect_patches<'a>(
    grouped: &'a BTreeMap<BoardGeneration, BTreeMap<u16, Vec<&'a PatchDefinition>>>,
) -> BTreeMap<&'a str, PatchInfo<'a>> {
    let mut patches: BTreeMap<&'a str, PatchInfo<'a>> = BTreeMap::new();

    for (board, versions) in grouped {
        for (version, defs) in versions {
            for def in defs {
                patches
                    .entry(def.id.as_str())
                    .and_modify(|info| {
                        info.boards.entry(*board).or_default().insert(*version);
                    })
                    .or_insert_with(|| {
                        let mut boards = BTreeMap::new();
                        boards.entry(*board).or_insert_with(BTreeSet::new).insert(*version);
                        PatchInfo {
                            name: def.name.as_str(),
                            description: def.description.as_str(),
                            id: def.id.as_str(),
                            target: def.target,
                            experimental: def.experimental,
                            boards,
                        }
                    });
            }
        }
    }

    patches
}

/// Desktop: renders the "Supported Patches" window when toggled from the top bar.
///
/// A row of board filter chips at the top lets the user narrow the list
/// to a specific board generation.  When a board is selected, parameter
/// info is shown for that board's version of each patch.
pub(crate) fn show(app: &mut PatcherApp, ui: &egui::Ui) {
    if !app.show_supported_patches {
        return;
    }

    let mut open = app.show_supported_patches;

    let max_width = (ui.ctx().viewport_rect().width() - 32.0).max(300.0);
    egui::Window::new("Supported Patches")
        .open(&mut open)
        .default_width(420.0_f32.min(max_width))
        .default_height(480.0)
        .resizable(true)
        .show(ui.ctx(), |ui| {
            show_content(app, ui);
        });

    app.show_supported_patches = open;
}

/// Mobile: renders the supported patches content directly into the provided `ui`.
pub(crate) fn show_inline(app: &mut PatcherApp, ui: &mut egui::Ui) {
    show_content(app, ui);
}

/// Shared content renderer used by both `show` (desktop) and `show_inline` (mobile).
fn show_content(app: &mut PatcherApp, ui: &mut egui::Ui) {
    let grouped = all_patches_grouped();
    let patches = collect_patches(&grouped);

    // Collect boards that actually have patches.
    let available_boards: BTreeSet<BoardGeneration> = grouped.keys().copied().collect();

    if patches.is_empty() {
        ui.label("No patch definitions found.");
        return;
    }

    ui.add_space(4.0);

    // ── Board filter chips ──────────────────────────────────
    ui.horizontal_wrapped(|ui| {
        let all_selected = app.supported_board_filter.is_none();
        if ui.selectable_label(all_selected, "All").clicked() {
            app.supported_board_filter = None;
        }

        ui.separator();

        for board in &available_boards {
            let selected = app.supported_board_filter == Some(*board);
            if ui.selectable_label(selected, board.to_string()).clicked() {
                app.supported_board_filter = if selected { None } else { Some(*board) };
            }
        }
    });

    ui.add_space(4.0);
    ui.separator();
    ui.add_space(4.0);

    // ── Patch list ──────────────────────────────────────────
    let filter = app.supported_board_filter;

    // Sort: firmware patches first, then bootloader, alphabetical within each.
    let mut sorted: Vec<&PatchInfo<'_>> =
        patches.values().filter(|info| filter.is_none_or(|board| info.boards.contains_key(&board))).collect();
    sorted.sort_by(|a, b| a.target.cmp(&b.target).then(a.name.cmp(b.name)));

    egui::ScrollArea::vertical().id_salt("supported_window_scroll").auto_shrink([false, true]).show(ui, |ui| {
        if sorted.is_empty() {
            ui.label("No patches match the selected board.");
        }

        for (i, info) in sorted.iter().enumerate() {
            if i > 0 {
                ui.add_space(4.0);
            }

            card_frame(ui).show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                show_patch_entry(ui, info, filter);
            });
        }
    });
}

/// Renders a single patch entry as a collapsible row.
fn show_patch_entry(ui: &mut egui::Ui, info: &PatchInfo<'_>, filter: Option<BoardGeneration>) {
    let id = ui.make_persistent_id(format!("sup_entry_{}", info.id));

    egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false)
        .show_header(ui, |ui| {
            let (tag, tag_color) = match info.target {
                PatchTarget::Firmware => ("[FW]", egui::Color32::from_rgb(100, 180, 230)),
                PatchTarget::Bootloader => ("[BL]", egui::Color32::from_rgb(180, 140, 255)),
            };
            ui.label(egui::RichText::new(tag).strong().color(tag_color).size(12.0));
            ui.label(egui::RichText::new(info.name).strong());
            if info.experimental {
                let warn =
                    egui::RichText::new(egui_phosphor::regular::WARNING).color(egui::Color32::from_rgb(255, 200, 60));
                ui.label(warn).on_hover_text("Experimental — this patch is not fully tested");
            }
        })
        .body(|ui| {
            // ── Description ─────────────────────────────────────────
            ui.label(egui::RichText::new(info.description).weak());
            ui.add_space(4.0);

            // ── Parameters ──────────────────────────────────────────
            if let Some(board) = filter {
                let version = info.boards.get(&board).and_then(|vs| vs.iter().next().copied());
                if let Some(version) = version {
                    show_params_detail(ui, info.id, board, version);
                }
            } else {
                let per_board: Vec<(BoardGeneration, u16)> =
                    info.boards.iter().filter_map(|(b, vs)| vs.iter().next().map(|&v| (*b, v))).collect();

                let param_names: Vec<Vec<&str>> = per_board
                    .iter()
                    .filter_map(|(b, v)| {
                        let key = scripting::cache_key(info.id, *b, *v);
                        scripting::get_compiled(&key).map(|c| c.params.iter().map(|p| p.name.as_str()).collect())
                    })
                    .collect();

                #[expect(clippy::indexing_slicing, reason = "windows(2) guarantees exactly 2 elements per window")]
                let all_same = param_names.windows(2).all(|w| w[0] == w[1]);

                if all_same {
                    if let Some(&(board, version)) = per_board.first() {
                        show_params_detail(ui, info.id, board, version);
                    }
                } else {
                    let has_params = param_names.iter().any(|p| !p.is_empty());
                    if has_params {
                        ui.add_space(2.0);
                        ui.label("Parameters vary by board — select a board to see details.");
                    }
                }
            }

            // ── Availability ────────────────────────────────────────
            ui.separator();
            if let Some(board) = filter {
                if let Some(versions) = info.boards.get(&board) {
                    let version_list: Vec<String> = versions.iter().map(|v| v.to_string()).collect();
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Versions:").strong());
                        ui.label(egui::RichText::new(version_list.join(", ")).weak());
                    });
                }
            } else {
                egui::Grid::new(format!("avail_{}", info.id)).num_columns(2).spacing([8.0, 2.0]).show(ui, |ui| {
                    for (board, versions) in &info.boards {
                        ui.label(egui::RichText::new(board.to_string()).strong());
                        let version_list: Vec<String> = versions.iter().map(|v| v.to_string()).collect();
                        ui.label(egui::RichText::new(version_list.join(", ")).weak());
                        ui.end_row();
                    }
                });
            }
        });
}

/// Shows the parameter detail collapsible for a specific patch version.
fn show_params_detail(ui: &mut egui::Ui, patch_id: &str, board: BoardGeneration, version: u16) {
    let key = scripting::cache_key(patch_id, board, version);
    let Some(compiled) = scripting::get_compiled(&key) else { return };
    if compiled.params.is_empty() {
        return;
    }

    let params_id = ui.make_persistent_id(format!("sup_params_{patch_id}"));
    egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), params_id, false)
        .show_header(ui, |ui| {
            ui.label(format!("Parameters ({})", compiled.params.len()));
        })
        .body(|ui| {
            egui::Grid::new(format!("sup_param_grid_{patch_id}")).num_columns(2).spacing([16.0, 3.0]).show(ui, |ui| {
                for param in &compiled.params {
                    // Column 1: parameter name.
                    let name_resp = ui.label(egui::RichText::new(&param.label).strong());
                    if let Some(desc) = &param.description {
                        name_resp.on_hover_text(desc);
                    }

                    // Column 2: type hint.
                    let hint_resp = ui.label(egui::RichText::new(param_kind_hint(&param.kind)).weak());
                    if let ScriptParamKind::Enum { options } = &param.kind {
                        let max_val_len = options.iter().map(|(v, _)| v.len()).max().unwrap_or(0);
                        let tooltip: String = options
                            .iter()
                            .map(|(val, label)| format!("{val:>max_val_len$} - {label}"))
                            .collect::<Vec<_>>()
                            .join("\n");
                        hint_resp.on_hover_text(egui::RichText::new(tooltip).family(egui::FontFamily::Monospace));
                    }

                    ui.end_row();
                }
            });
        });
}

/// Returns a compact type hint string for a parameter kind.
fn param_kind_hint(kind: &ScriptParamKind) -> String {
    match kind {
        ScriptParamKind::Toggle => "toggle".into(),
        ScriptParamKind::Integer { min, max } => format!("int({min}..{max})"),
        ScriptParamKind::Float { min, max } => format!("float({min}..{max})"),
        ScriptParamKind::Enum { options } => format!("{} options", options.len()),
        ScriptParamKind::Hex { len } => format!("hex({len} bytes)"),
    }
}
