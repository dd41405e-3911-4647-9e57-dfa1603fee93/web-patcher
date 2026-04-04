use super::helpers::card_frame;
use crate::app::{PatcherApp, PendingFileKind};
use owtk_core::backup::{BackupConfig, ParsedBackup, write_f1_config, write_f4_config};
use owtk_core::board::McuFamily;

/// Amber highlight for modified values.
const MODIFIED_COLOR: egui::Color32 = egui::Color32::from_rgb(255, 210, 100);

struct SerialOtpRowParams<'a> {
    serial_lo: &'a mut Option<u16>,
    serial_hi: &'a mut Option<u16>,
    otp_lo: &'a mut Option<u16>,
    otp_hi: &'a mut Option<u16>,
    original_serial_lo: Option<u16>,
    original_serial_hi: Option<u16>,
    original_otp_lo: Option<u16>,
    original_otp_hi: Option<u16>,
}

/// Desktop: renders the backup window as a floating `egui::Window`.
pub(crate) fn show(app: &mut PatcherApp, ui: &egui::Ui) {
    let Some(backup) = app.parsed_backup.as_mut() else {
        return;
    };

    let mut open = true;
    let mut save_request: Option<(Vec<u8>, String, &str)> = None;
    let mut import_request: Option<PendingFileKind> = None;

    let max_width = (ui.ctx().viewport_rect().width() - 32.0).max(300.0);
    egui::Window::new("Backup").default_width(420.0_f32.min(max_width)).default_height(300.0).open(&mut open).show(
        ui.ctx(),
        |ui| {
            show_content(backup, ui, &mut save_request, &mut import_request);
        },
    );

    let ctx = ui.ctx().clone();
    if let Some((data, name, title)) = save_request {
        app.start_save_request(data, &name, title, &ctx);
    }

    if let Some(kind) = import_request {
        app.start_file_request(kind, &ctx);
    }

    if !open {
        app.parsed_backup = None;
    }
}

/// Mobile: renders the backup content directly into the provided `ui`.
pub(crate) fn show_inline(app: &mut PatcherApp, ui: &mut egui::Ui) {
    let Some(backup) = app.parsed_backup.as_mut() else {
        return;
    };

    let mut save_request: Option<(Vec<u8>, String, &str)> = None;
    let mut import_request: Option<PendingFileKind> = None;

    show_content(backup, ui, &mut save_request, &mut import_request);

    let ctx = ui.ctx().clone();
    if let Some((data, name, title)) = save_request {
        app.start_save_request(data, &name, title, &ctx);
    }

    if let Some(kind) = import_request {
        app.start_file_request(kind, &ctx);
    }
}

/// Shared content renderer.
fn show_content(
    backup: &mut ParsedBackup,
    ui: &mut egui::Ui,
    save_request: &mut Option<(Vec<u8>, String, &'static str)>,
    import_request: &mut Option<PendingFileKind>,
) {
    let mcu = backup.mcu_family;
    let has_changes =
        backup.config != backup.original_config || backup.bootloader_version != backup.original_bootloader_version;

    egui::ScrollArea::vertical().auto_shrink([false, true]).show(ui, |ui| {
        show_bootloader_info(ui, backup, save_request, import_request);
        ui.add_space(2.0);
        show_firmware_info(ui, backup, save_request, import_request);
        ui.separator();
        ui.add_space(2.0);
        show_settings(ui, backup, has_changes, mcu, save_request);
    });
}

// ── Card sections extracted from `show` ──────────────────────────────

fn show_bootloader_info(
    ui: &mut egui::Ui,
    backup: &mut ParsedBackup,
    save_request: &mut Option<(Vec<u8>, String, &'static str)>,
    import_request: &mut Option<PendingFileKind>,
) {
    let mcu = backup.mcu_family;

    // ── Info grid ─────────────────────────────────────────────────
    egui::Grid::new("backup_info_grid").num_columns(3).spacing([8.0, 4.0]).show(ui, |ui| {
        // Bootloader row
        ui.strong("Bootloader:");
        if let Some(bl) = &backup.bootloader {
            ui.horizontal(|ui| {
                ui.label(format!("{} - v{}", bl.descriptor.board, bl.descriptor.version));
                if !bl.exact_match {
                    ui.label(egui::RichText::new("(modified)").color(MODIFIED_COLOR).size(12.0))
                        .on_hover_text("Bootloader only matches partial hash.");
                }
            });
        } else if backup.bootloader_present {
            ui.colored_label(egui::Color32::from_rgb(180, 180, 180), "Unknown");
        } else {
            ui.colored_label(egui::Color32::from_rgb(180, 180, 180), "Not present");
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .small_button(egui_phosphor::regular::FLOPPY_DISK)
                .on_hover_text("Extract bootloader to file")
                .clicked()
            {
                let range = mcu.bootloader_range();
                let name = if let Some(bl) = &backup.bootloader {
                    format!("bootloader_{}_{}.bin", bl.descriptor.board, bl.descriptor.version)
                } else {
                    format!("bootloader_{mcu}.bin")
                };
                let region = backup.data.get(range).expect("bootloader region within backup bounds");
                *save_request = Some((region.to_vec(), name, "Save Bootloader"));
            }
            if ui
                .small_button(egui_phosphor::regular::FOLDER_OPEN)
                .on_hover_text("Replace bootloader from file")
                .clicked()
            {
                *import_request = Some(PendingFileKind::BackupImportBootloader);
            }
        });
        ui.end_row();
    });

    show_bootloader_version_override(ui, backup);
}

fn show_bootloader_version_override(ui: &mut egui::Ui, backup: &mut ParsedBackup) {
    ui.add_space(2.0);
    let ver_modified = backup.bootloader_version != backup.original_bootloader_version;
    let detected_version = backup.bootloader.as_ref().map(|bl| bl.descriptor.version);
    let version_mismatch = match (backup.bootloader_version, detected_version) {
        (Some(stored), Some(detected)) => stored != detected,
        _ => false,
    };
    let header_text = if ver_modified {
        egui::RichText::new(format!("{} File Version", egui_phosphor::regular::WARNING)).color(MODIFIED_COLOR)
    } else if version_mismatch {
        egui::RichText::new(format!("{} File Version", egui_phosphor::regular::WARNING))
            .color(egui::Color32::from_rgb(255, 100, 100))
    } else {
        egui::RichText::new("File Version")
    };
    let mut header = egui::CollapsingHeader::new(header_text).id_salt("bl_file_version").default_open(false);
    if version_mismatch {
        header = header.open(Some(true));
    }
    header.show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label("Stored version:");
            editable_u16(ui, &mut backup.bootloader_version);
            if reset_button(ui, ver_modified) {
                backup.bootloader_version = backup.original_bootloader_version;
            }
        });
        ui.label(
            egui::RichText::new(
                "The bootloader version stored in the file. The Onewheel \
                     app reads this value to determine which firmware updates \
                     are available. The board and version shown above are \
                     identified by hash and may differ from this field. \
                     This should match the detected version and only be \
                     changed if you know what you are doing.",
            )
            .color(egui::Color32::from_rgb(255, 100, 100))
            .size(12.0),
        );
    });
}
fn show_firmware_info(
    ui: &mut egui::Ui,
    backup: &ParsedBackup,
    save_request: &mut Option<(Vec<u8>, String, &'static str)>,
    import_request: &mut Option<PendingFileKind>,
) {
    let mcu = backup.mcu_family;

    egui::Grid::new("backup_firmware_grid").num_columns(3).spacing([8.0, 4.0]).show(ui, |ui| {
        // Firmware row
        ui.strong("Firmware:");
        if let Some(fw) = &backup.firmware {
            ui.horizontal(|ui| {
                ui.label(format!("{} - {}", fw.descriptor.board, fw.descriptor.version));
                if !fw.exact_match {
                    ui.label(egui::RichText::new("(modified)").color(MODIFIED_COLOR).size(12.0))
                        .on_hover_text("Firmware only matches partial hash.");
                }
            });
        } else if backup.firmware_present {
            ui.colored_label(egui::Color32::from_rgb(180, 180, 180), "Unknown");
        } else {
            ui.colored_label(egui::Color32::from_rgb(180, 180, 180), "Not present");
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button(egui_phosphor::regular::FLOPPY_DISK).on_hover_text("Extract firmware to file").clicked()
            {
                let range = mcu.firmware_range();
                let name = if let Some(fw) = &backup.firmware {
                    format!("firmware_{}_{}.bin", fw.descriptor.board, fw.descriptor.version)
                } else {
                    format!("firmware_{mcu}.bin")
                };
                let region = backup.data.get(range).expect("firmware region within backup bounds");
                *save_request = Some((region.to_vec(), name, "Save Firmware"));
            }
            if ui
                .small_button(egui_phosphor::regular::FOLDER_OPEN)
                .on_hover_text("Replace firmware from file")
                .clicked()
            {
                *import_request = Some(PendingFileKind::BackupImportFirmware);
            }
        });
        ui.end_row();
    });
}

fn show_settings(
    ui: &mut egui::Ui,
    backup: &mut ParsedBackup,
    has_changes: bool,
    mcu: McuFamily,
    save_request: &mut Option<(Vec<u8>, String, &'static str)>,
) {
    // ── Settings header with save button ─────────────────────────
    ui.horizontal(|ui| {
        ui.heading("Settings");

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Save Backup").clicked() {
                let mut data = backup.data.clone();
                if has_changes {
                    match mcu {
                        McuFamily::F1 => write_f1_config(&mut data, &backup.config),
                        McuFamily::F4 => write_f4_config(&mut data, &backup.config),
                    }
                    backup.write_bootloader_version(&mut data);
                }
                *save_request = Some((data, backup.default_filename(), "Save Backup"));
            }
        });
    });

    ui.add_space(2.0);

    // ── Config card ──────────────────────────────────────────────
    card_frame(ui).show(ui, |ui| {
        show_config_fields(ui, &mut backup.config, &backup.original_config, mcu);
    });
}

/// Renders the editable config fields.
fn show_config_fields(ui: &mut egui::Ui, config: &mut BackupConfig, original: &BackupConfig, mcu: McuFamily) {
    // Serial — on F4 also writes OTP serial.
    if mcu == McuFamily::F4 {
        config_row_serial_otp(
            ui,
            "Serial:",
            &mut SerialOtpRowParams {
                serial_lo: &mut config.serial_lo,
                serial_hi: &mut config.serial_hi,
                otp_lo: &mut config.otp_serial_lo,
                otp_hi: &mut config.otp_serial_hi,
                original_serial_lo: original.serial_lo,
                original_serial_hi: original.serial_hi,
                original_otp_lo: original.otp_serial_lo,
                original_otp_hi: original.otp_serial_hi,
            },
        );
    } else {
        config_row_u32(
            ui,
            "Serial:",
            &mut config.serial_lo,
            &mut config.serial_hi,
            original.serial_lo,
            original.serial_hi,
        );
    }

    config_row_u32(
        ui,
        "BMS Serial:",
        &mut config.bms_serial_lo,
        &mut config.bms_serial_hi,
        original.bms_serial_lo,
        original.bms_serial_hi,
    );

    config_row_i16(ui, "Tilt Pitch:", &mut config.tilt_pitch, original.tilt_pitch);
    config_row_i16(ui, "Gyro X Offset:", &mut config.gyro_x_offset, original.gyro_x_offset);
    config_row_i16(ui, "Gyro Z Offset:", &mut config.gyro_z_offset, original.gyro_z_offset);
    config_row_i16(ui, "Gyro Y Offset:", &mut config.gyro_y_offset, original.gyro_y_offset);
    config_row_enum::<HwGeneration>(ui, "Generation:", &mut config.generation, original.generation);
    config_row_bool(ui, "Simplestop", &mut config.simplestop, original.simplestop);

    if mcu == McuFamily::F4 {
        config_row_bool(ui, "Haptic Enabled", &mut config.haptic_enabled, original.haptic_enabled);
        config_row_bool(ui, "Recurve Rails", &mut config.recurve_rails, original.recurve_rails);
    }

    config_row_u32(
        ui,
        "Lifetime Odometer:",
        &mut config.odometer_lo,
        &mut config.odometer_hi,
        original.odometer_lo,
        original.odometer_hi,
    );
    config_row_u32(
        ui,
        "Lifetime Amp Hours:",
        &mut config.amp_hours_lo,
        &mut config.amp_hours_hi,
        original.amp_hours_lo,
        original.amp_hours_hi,
    );
}

// ── ConfigEnum trait + implementations ───────────────────────────────

/// A type that can be selected from a combo box and stored as `Option<u16>`.
trait ConfigEnum: Copy + PartialEq + 'static {
    fn from_u16(val: u16) -> Option<Self>;
    fn to_u16(self) -> u16;
    fn variants() -> &'static [Self];
    fn label(self) -> &'static str;
}

/// Hardware generation values stored in the config.
#[derive(Debug, Copy, Clone, PartialEq)]
enum HwGeneration {
    V1,
    V1_2,
    Plus,
    XR,
    Pint,
    GT,
    PintXS,
    GTS,
    XRC,
}

impl ConfigEnum for HwGeneration {
    fn from_u16(val: u16) -> Option<Self> {
        match val {
            1 => Some(Self::V1),
            2 => Some(Self::V1_2),
            3 => Some(Self::Plus),
            4 => Some(Self::XR),
            5 => Some(Self::Pint),
            6 => Some(Self::GT),
            7 => Some(Self::PintXS),
            8 => Some(Self::GTS),
            9 => Some(Self::XRC),
            _ => None,
        }
    }

    fn to_u16(self) -> u16 {
        match self {
            Self::V1 => 1,
            Self::V1_2 => 2,
            Self::Plus => 3,
            Self::XR => 4,
            Self::Pint => 5,
            Self::GT => 6,
            Self::PintXS => 7,
            Self::GTS => 8,
            Self::XRC => 9,
        }
    }

    fn variants() -> &'static [Self] {
        &[Self::V1, Self::V1_2, Self::Plus, Self::XR, Self::Pint, Self::GT, Self::PintXS, Self::GTS, Self::XRC]
    }

    fn label(self) -> &'static str {
        match self {
            Self::V1 => "V1",
            Self::V1_2 => "V1.2",
            Self::Plus => "Plus",
            Self::XR => "XR",
            Self::Pint => "Pint",
            Self::GT => "GT",
            Self::PintXS => "Pint X/S",
            Self::GTS => "GTS",
            Self::XRC => "XRC",
        }
    }
}

// ── Row helpers (label left, value + reset right-aligned) ────────────

fn config_row_u32(
    ui: &mut egui::Ui,
    label: &str,
    lo: &mut Option<u16>,
    hi: &mut Option<u16>,
    original_lo: Option<u16>,
    original_hi: Option<u16>,
) {
    let is_modified = *lo != original_lo || *hi != original_hi;
    ui.horizontal(|ui| {
        modified_label(ui, label, is_modified);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if reset_button(ui, is_modified) {
                *lo = original_lo;
                *hi = original_hi;
            }
            editable_u32(ui, lo, hi);
        });
    });
}

fn config_row_i16(ui: &mut egui::Ui, label: &str, val: &mut Option<u16>, original: Option<u16>) {
    let is_modified = *val != original;
    ui.horizontal(|ui| {
        modified_label(ui, label, is_modified);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if reset_button(ui, is_modified) {
                *val = original;
            }
            editable_i16(ui, val);
        });
    });
}

fn config_row_enum<T: ConfigEnum>(ui: &mut egui::Ui, label: &str, val: &mut Option<u16>, original: Option<u16>) {
    let is_modified = *val != original;
    ui.horizontal(|ui| {
        modified_label(ui, label, is_modified);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if reset_button(ui, is_modified) {
                *val = original;
            }
            let current = val.and_then(T::from_u16);
            let display = current.map_or("Unknown", |v| v.label());
            egui::ComboBox::from_id_salt(label).selected_text(display).show_ui(ui, |ui| {
                for &variant in T::variants() {
                    let selected = current == Some(variant);
                    if ui.selectable_label(selected, variant.label()).clicked() {
                        *val = Some(variant.to_u16());
                    }
                }
            });
        });
    });
}

fn config_row_bool(ui: &mut egui::Ui, label: &str, val: &mut Option<u16>, original: Option<u16>) {
    let is_modified = *val != original;
    ui.horizontal(|ui| {
        modified_label(ui, label, is_modified);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if reset_button(ui, is_modified) {
                *val = original;
            }
            let mut checked = val.map_or(false, |v| v != 0);
            if ui.checkbox(&mut checked, "").changed() {
                *val = Some(if checked { 1 } else { 0 });
            }
        });
    });
}

fn config_row_serial_otp(ui: &mut egui::Ui, label: &str, params: &mut SerialOtpRowParams<'_>) {
    let is_modified = *params.serial_lo != params.original_serial_lo || *params.serial_hi != params.original_serial_hi;
    ui.horizontal(|ui| {
        modified_label(ui, label, is_modified);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if reset_button(ui, is_modified) {
                *params.serial_lo = params.original_serial_lo;
                *params.serial_hi = params.original_serial_hi;
                *params.otp_lo = params.original_otp_lo;
                *params.otp_hi = params.original_otp_hi;
            }
            editable_serial_with_otp(ui, params.serial_lo, params.serial_hi, params.otp_lo, params.otp_hi);
        });
    });
}

// ── Field helpers ────────────────────────────────────────────────────

fn modified_label(ui: &mut egui::Ui, label: &str, is_modified: bool) {
    let text = egui::RichText::new(label).strong();
    let text = if is_modified { text.color(MODIFIED_COLOR) } else { text };
    ui.label(text);
}

/// Shows a reset button. Disabled when the field is unmodified.
/// Returns `true` if clicked.
fn reset_button(ui: &mut egui::Ui, is_modified: bool) -> bool {
    ui.add_enabled(is_modified, egui::Button::new(egui_phosphor::regular::ARROW_COUNTER_CLOCKWISE).small()).clicked()
}

fn editable_u32(ui: &mut egui::Ui, lo: &mut Option<u16>, hi: &mut Option<u16>) {
    let current = (u32::from(hi.unwrap_or(0)) << 16) | u32::from(lo.unwrap_or(0));
    let mut v = i64::from(current);
    let drag = egui::DragValue::new(&mut v).range(0..=0xFFFF_FFFE_i64).speed(1.0);
    if ui.add(drag).changed() {
        v = v.clamp(0, 0xFFFF_FFFE);
        *lo = Some((v & 0xFFFF) as u16);
        *hi = Some(((v >> 16) & 0xFFFF) as u16);
    }
}

fn editable_u16(ui: &mut egui::Ui, val: &mut Option<u16>) {
    let mut v = val.unwrap_or(0) as i32;
    let drag = egui::DragValue::new(&mut v).range(0..=0xFFFE_i32).speed(1.0);
    if ui.add(drag).changed() {
        v = v.clamp(0, 0xFFFE);
        *val = Some(v as u16);
    }
}

fn editable_i16(ui: &mut egui::Ui, val: &mut Option<u16>) {
    let mut signed = val.unwrap_or(0) as i16 as i32;
    let drag = egui::DragValue::new(&mut signed).range(i16::MIN as i32..=i16::MAX as i32).speed(1.0);
    if ui.add(drag).changed() {
        signed = signed.clamp(i16::MIN as i32, i16::MAX as i32);
        *val = Some(signed as i16 as u16);
    }
}

fn editable_serial_with_otp(
    ui: &mut egui::Ui,
    serial_lo: &mut Option<u16>,
    serial_hi: &mut Option<u16>,
    otp_lo: &mut Option<u16>,
    otp_hi: &mut Option<u16>,
) {
    let current = (u32::from(serial_hi.unwrap_or(0)) << 16) | u32::from(serial_lo.unwrap_or(0));
    let mut v = i64::from(current);
    let drag = egui::DragValue::new(&mut v).range(0..=0xFFFF_FFFE_i64).speed(1.0);
    if ui.add(drag).changed() {
        v = v.clamp(0, 0xFFFF_FFFE);
        let lo_val = (v & 0xFFFF) as u16;
        let hi_val = ((v >> 16) & 0xFFFF) as u16;
        *serial_lo = Some(lo_val);
        *serial_hi = Some(hi_val);
        *otp_lo = Some(lo_val);
        *otp_hi = Some(hi_val);
    }
}
