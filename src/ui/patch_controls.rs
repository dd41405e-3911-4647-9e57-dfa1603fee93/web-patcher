use owtk_core::patches::types::{ScriptParam, ScriptParamKind, ScriptValue};
use owtk_core::patches::{PatchEntry, PatchSelection, PatchStatus, scripting};

/// Renders the UI for a single patch entry as a collapsing header.
///
/// - **Collapsed**: patch name + status badge on one line (compact).
/// - **Expanded**: description text + script parameter controls.
pub(crate) fn show_patch_entry(ui: &mut egui::Ui, entry: &mut PatchEntry) {
    let def = entry.definition;
    let status_text = status_label(entry).to_owned();
    let status_color = status_color(entry);

    // Whether this patch has been changed from its initial state.
    let cache_key = scripting::cache_key(&def.id, def.board, def.version);
    let is_modified = entry.selection != entry.initial_selection;

    let id = ui.make_persistent_id(format!("patch_entry_{}", def.id));

    // Resolve compiled params once so we can toggle selection in the header.
    let compiled = scripting::get_compiled(&cache_key);
    let params = compiled.as_ref().map(|c| &c.params[..]);

    egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false)
        .show_header(ui, |ui| {
            ui.horizontal(|ui| {
                // Enable / disable checkbox (no label).
                let mut enabled = !matches!(entry.selection, PatchSelection::Disabled);
                if ui.checkbox(&mut enabled, "").changed() {
                    entry.selection = if enabled {
                        // Prefer read-back values (actual firmware state)
                        // over parameter defaults.
                        if let Some(values) = &entry.read_values {
                            PatchSelection::Values(values.clone())
                        } else {
                            match params {
                                Some(p) if !p.is_empty() => {
                                    PatchSelection::Values(p.iter().map(|p| p.default.clone()).collect())
                                }
                                _ => PatchSelection::Values(Vec::new()),
                            }
                        }
                    } else {
                        PatchSelection::Disabled
                    };
                }

                let name_text = egui::RichText::new(&def.name).strong();
                // Highlight the name if the patch has been modified from
                // its initial loaded state.
                let name_text =
                    if is_modified { name_text.color(egui::Color32::from_rgb(255, 210, 100)) } else { name_text };
                ui.label(name_text);

                if def.experimental {
                    let warn = egui::RichText::new(egui_phosphor::regular::WARNING)
                        .color(egui::Color32::from_rgb(255, 200, 60));
                    ui.label(warn).on_hover_text("Experimental — this patch is not fully tested");
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.colored_label(status_color, &status_text).on_hover_text(status_tooltip(entry));
                });
            });
        })
        .body(|ui| {
            // Description
            ui.label(egui::RichText::new(&def.description).weak());

            // Script parameter controls (only shown when enabled).
            if !matches!(entry.selection, PatchSelection::Disabled)
                && let Some(params) = params
                && !params.is_empty()
            {
                ui.separator();
                show_script_params(ui, &mut entry.selection, params, entry.read_values.as_deref(), &cache_key);
            }
        });
}

/// Returns a short status label for the patch.
fn status_label(entry: &PatchEntry) -> &str {
    match &entry.status {
        PatchStatus::Stock => "Stock",
        PatchStatus::Applied => "Applied",
        PatchStatus::Unknown => "Unknown state",
    }
}

/// Returns the color to use for the status badge.
fn status_color(entry: &PatchEntry) -> egui::Color32 {
    match &entry.status {
        PatchStatus::Stock => egui::Color32::from_rgb(120, 180, 120),
        PatchStatus::Applied => egui::Color32::from_rgb(100, 160, 220),
        PatchStatus::Unknown => egui::Color32::YELLOW,
    }
}

/// Returns a tooltip description for the patch status.
fn status_tooltip(entry: &PatchEntry) -> &str {
    match &entry.status {
        PatchStatus::Stock => {
            "The loaded firmware has stock bytes at this patch's offsets — it was not applied when the firmware was loaded."
        }
        PatchStatus::Applied => {
            "The loaded firmware has modified bytes at this patch's offsets — it was already applied when the firmware was loaded."
        }
        PatchStatus::Unknown => {
            "The patch's target offsets extend past the loaded firmware buffer — status could not be determined."
        }
    }
}

// ── Script ──────────────────────────────────────────────────────────────

/// Renders script parameter controls (enum, int, float, toggle).
/// Called only when the patch is enabled and has parameters.
fn show_script_params(
    ui: &mut egui::Ui,
    selection: &mut PatchSelection,
    params: &[ScriptParam],
    read_values: Option<&[ScriptValue]>,
    cache_key: &str,
) {
    // Ensure we have a Values selection with the right count.
    let values = match selection {
        PatchSelection::Values(vals) if vals.len() == params.len() => vals,
        _ => {
            *selection = PatchSelection::Values(params.iter().map(|p| p.default.clone()).collect());
            match selection {
                PatchSelection::Values(vals) => vals,
                PatchSelection::Disabled => unreachable!(),
            }
        }
    };

    egui::Grid::new(format!("script_params_{cache_key}")).num_columns(3).spacing([8.0, 4.0]).show(ui, |ui| {
        for (idx, (param, value)) in params.iter().zip(values.iter_mut()).enumerate() {
            // The initial value to revert to: prefer read-back, fall back to default.
            let initial = read_values.and_then(|rv| rv.get(idx)).unwrap_or(&param.default);

            // Show the label with an optional tooltip for the description.
            let add_label = |ui: &mut egui::Ui| {
                let resp = ui.label(&param.label);
                if let Some(desc) = &param.description {
                    resp.on_hover_text(desc);
                }
            };

            // Revert button — shown for int/float when value differs from initial.
            let add_revert = |ui: &mut egui::Ui, value: &mut ScriptValue, initial: &ScriptValue| {
                let enabled = value != initial;
                let btn =
                    egui::Button::new(egui::RichText::new(egui_phosphor::regular::ARROW_COUNTER_CLOCKWISE).size(13.0))
                        .small();
                if ui.add_enabled(enabled, btn).on_hover_text("Revert to initial value").clicked() {
                    *value = initial.clone();
                }
            };

            match &param.kind {
                ScriptParamKind::Toggle => {
                    let mut enabled = matches!(value, ScriptValue::Bool(true));
                    if ui.checkbox(&mut enabled, &param.label).changed() {
                        *value = ScriptValue::Bool(enabled);
                    }
                    ui.end_row();
                }
                ScriptParamKind::Integer { min, max } => {
                    add_label(ui);
                    show_integer_input(ui, *min, *max, value);
                    add_revert(ui, value, initial);
                    ui.end_row();
                }
                ScriptParamKind::Float { min, max } => {
                    add_label(ui);
                    show_float_input(ui, *min, *max, value);
                    add_revert(ui, value, initial);
                    ui.end_row();
                }
                ScriptParamKind::Enum { options } => {
                    add_label(ui);
                    let id_salt = format!("script_param_{cache_key}_{}", param.name);
                    show_enum_input(ui, &id_salt, options, value);
                    ui.end_row();
                }
                ScriptParamKind::Hex { len } => {
                    add_label(ui);
                    let id = egui::Id::new(format!("hex_buf_{cache_key}_{}", param.name));
                    show_hex_input(ui, id, *len, value);
                    let before = value.clone();
                    add_revert(ui, value, initial);
                    if *value != before {
                        ui.data_mut(|d| d.remove::<String>(id));
                    }
                    ui.end_row();
                }
            }
        }
    });
}

/// Renders an integer drag-value control.
fn show_integer_input(ui: &mut egui::Ui, min: i64, max: i64, value: &mut ScriptValue) {
    let mut v = match value {
        ScriptValue::Int(i) => *i,
        _ => min,
    };
    if ui.add(egui::DragValue::new(&mut v).range(min..=max).speed(1.0)).changed() {
        *value = ScriptValue::Int(v);
    }
}

/// Renders a float drag-value control.
fn show_float_input(ui: &mut egui::Ui, min: f64, max: f64, value: &mut ScriptValue) {
    let mut v = match value {
        ScriptValue::Float(f) => *f,
        _ => min,
    };
    if ui.add(egui::DragValue::new(&mut v).range(min..=max).speed(0.1).max_decimals(4)).changed() {
        *value = ScriptValue::Float(v);
    }
}

/// Renders an enum combo-box control.
fn show_enum_input(ui: &mut egui::Ui, id_salt: &str, options: &[(String, String)], value: &mut ScriptValue) {
    let current_label = match value {
        ScriptValue::String(s) => options.iter().find(|(v, _)| v == s).map_or(s.as_str(), |(_, label)| label.as_str()),
        _ => "(none)",
    };
    egui::ComboBox::from_id_salt(id_salt).selected_text(current_label).show_ui(ui, |ui| {
        for (opt_value, opt_label) in options {
            let is_selected = matches!(value, ScriptValue::String(s) if s == opt_value);
            if ui.selectable_label(is_selected, opt_label).clicked() && !is_selected {
                *value = ScriptValue::String(opt_value.clone());
            }
        }
    });
}

/// Renders a hex byte string text input, updating `value` when the input is valid.
fn show_hex_input(ui: &mut egui::Ui, id: egui::Id, len: usize, value: &mut ScriptValue) {
    let mut buf = ui
        .data_mut(|d| d.get_temp::<String>(id))
        .unwrap_or_else(|| match value {
            ScriptValue::Bytes(b) => hex::encode(b),
            _ => "00".repeat(len),
        });

    let expected_chars = len * 2;
    let is_valid = buf.len() == expected_chars && buf.chars().all(|c| c.is_ascii_hexdigit());

    let text_color = if is_valid { ui.visuals().text_color() } else { egui::Color32::from_rgb(255, 100, 100) };

    let response = ui.add(
        egui::TextEdit::singleline(&mut buf)
            .font(egui::TextStyle::Monospace)
            .desired_width((len as f32 * 16.0).min(400.0))
            .char_limit(expected_chars)
            .text_color(text_color)
            .hint_text(format!("{expected_chars} hex chars")),
    );

    if response.changed() {
        buf = buf.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_ascii_lowercase();
    }

    if buf.len() == expected_chars && buf.chars().all(|c| c.is_ascii_hexdigit()) {
        if let Ok(bytes) = hex::decode(&buf) {
            *value = ScriptValue::Bytes(bytes);
        }
    }

    ui.data_mut(|d| d.insert_temp(id, buf));
}
