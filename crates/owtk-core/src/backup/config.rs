use super::layout::{
    F1_CONFIG_BACKUP_START, F1_CONFIG_PRIMARY_END, F1_CONFIG_PRIMARY_START, F4_CONFIG_A_END, F4_CONFIG_A_START,
    F4_CONFIG_B_END, F4_CONFIG_B_START, F4_OTP_SERIAL_HI, F4_OTP_SERIAL_LO,
};

/// Sentinel value for erased/unset 16-bit fields in flash.
const ERASED_U16: u16 = 0xFFFF;

/// Sentinel value for erased/unset 32-bit fields in flash.
const ERASED_U32: u32 = 0xFFFF_FFFF;

// ── Parsed config struct ─────────────────────────────────────────────

/// Parsed configuration from a flash backup.
///
/// Contains only the essential fields that need to be editable:
/// serial, odometer, amp hours, and BMS serial.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BackupConfig {
    pub serial_lo: Option<u16>,
    pub serial_hi: Option<u16>,

    /// OTP serial (F4 only) — must match config serial for firmware.
    pub otp_serial_lo: Option<u16>,
    pub otp_serial_hi: Option<u16>,

    pub tilt_pitch: Option<u16>,
    pub gyro_x_offset: Option<u16>,
    pub gyro_z_offset: Option<u16>,
    pub gyro_y_offset: Option<u16>,
    pub generation: Option<u16>,
    pub simplestop: Option<u16>,

    /// F4 Only
    pub haptic_enabled: Option<u16>,
    pub recurve_rails: Option<u16>,

    pub odometer_lo: Option<u16>,
    pub odometer_hi: Option<u16>,

    pub amp_hours_lo: Option<u16>,
    pub amp_hours_hi: Option<u16>,

    pub bms_serial_lo: Option<u16>,
    pub bms_serial_hi: Option<u16>,
}

// ── Shared helpers ───────────────────────────────────────────────────

/// Combines an optional `(lo, hi)` pair into a u32.
/// Returns `None` unless both halves are present.
fn combine_u16s(lo: Option<u16>, hi: Option<u16>) -> Option<u32> {
    match (lo, hi) {
        (Some(lo), Some(hi)) => Some((u32::from(hi) << 16) | u32::from(lo)),
        _ => None,
    }
}

/// Splits an optional u32 into `(lo, hi)` u16 halves.
fn split_u32(val: Option<u32>) -> (Option<u16>, Option<u16>) {
    match val {
        Some(v) => (Some(v as u16), Some((v >> 16) as u16)),
        None => (None, None),
    }
}

/// Reads a little-endian u16 from `data` at byte offset `offset`.
/// Returns `None` if the value equals the erased sentinel or if the
/// offset is out of bounds.
pub(super) fn read_u16_le(data: &[u8], offset: usize) -> Option<u16> {
    let bytes: [u8; 2] = data.get(offset..offset + 2)?.try_into().ok()?;
    let val = u16::from_le_bytes(bytes);
    if val == ERASED_U16 { None } else { Some(val) }
}

/// Reads a little-endian u32 from `data` at byte offset `offset`.
/// Returns `None` if the value equals the erased sentinel or if the
/// offset is out of bounds.
fn read_u32_le(data: &[u8], offset: usize) -> Option<u32> {
    let bytes: [u8; 4] = data.get(offset..offset + 4)?.try_into().ok()?;
    let val = u32::from_le_bytes(bytes);
    if val == ERASED_U32 { None } else { Some(val) }
}

/// Writes a little-endian u16 value to `data` at the given offset.
/// Writes the erased sentinel (`0xFFFF`) for `None`.
///
/// # Panics
///
/// Panics if `offset + 2` exceeds `data.len()`.
fn write_u16_le(data: &mut [u8], offset: usize, val: Option<u16>) {
    let bytes = val.unwrap_or(ERASED_U16).to_le_bytes();
    data.get_mut(offset..offset + 2).expect("write_u16_le: offset out of bounds").copy_from_slice(&bytes);
}

/// Writes a little-endian u32 value to `data` at the given offset.
/// Writes the erased sentinel (`0xFFFFFFFF`) for `None`.
///
/// # Panics
///
/// Panics if `offset + 4` exceeds `data.len()`.
fn write_u32_le(data: &mut [u8], offset: usize, val: Option<u32>) {
    let bytes = val.unwrap_or(ERASED_U32).to_le_bytes();
    data.get_mut(offset..offset + 4).expect("write_u32_le: offset out of bounds").copy_from_slice(&bytes);
}

// ── F1 config parsing ───────────────────────────────────────────────

const F1_TILT_PITCH: usize = 0x00;
const F1_GYRO_X: usize = 0x02;
const F1_GYRO_Z: usize = 0x04;
const F1_GYRO_Y: usize = 0x06;
const F1_SERIAL_LO: usize = 0x0A;
const F1_ODOMETER: usize = 0x0C;
const F1_AMP_HOURS: usize = 0x10;
const F1_SERIAL_HI: usize = 0x30;
const F1_GENERATION: usize = 0x28;
const F1_BMS_ID: usize = 0x38;
const F1_SIMPLESTOP: usize = 0x3C;

/// Parses an F1 config page (1024 bytes) into a [`BackupConfig`].
pub fn parse_f1_config(page: &[u8]) -> BackupConfig {
    let odometer = read_u32_le(page, F1_ODOMETER);
    let amp_hours = read_u32_le(page, F1_AMP_HOURS);
    let bms_id = read_u32_le(page, F1_BMS_ID);

    let (odometer_lo, odometer_hi) = split_u32(odometer);
    let (amp_hours_lo, amp_hours_hi) = split_u32(amp_hours);
    let (bms_serial_lo, bms_serial_hi) = split_u32(bms_id);

    BackupConfig {
        serial_lo: read_u16_le(page, F1_SERIAL_LO),
        serial_hi: read_u16_le(page, F1_SERIAL_HI),
        otp_serial_lo: None,
        otp_serial_hi: None,
        tilt_pitch: read_u16_le(page, F1_TILT_PITCH),
        gyro_x_offset: read_u16_le(page, F1_GYRO_X),
        gyro_z_offset: read_u16_le(page, F1_GYRO_Z),
        gyro_y_offset: read_u16_le(page, F1_GYRO_Y),
        generation: read_u16_le(page, F1_GENERATION),
        simplestop: read_u16_le(page, F1_SIMPLESTOP),
        recurve_rails: None,
        haptic_enabled: None,
        odometer_lo,
        odometer_hi,
        amp_hours_lo,
        amp_hours_hi,
        bms_serial_lo,
        bms_serial_hi,
    }
}

// ── F4 config parsing ───────────────────────────────────────────────

/// F4 sector header value indicating an active sector.
const F4_SECTOR_ACTIVE: u16 = 0x0000;

/// Size of the sector header slot (2-byte header + 2-byte padding,
/// records start at offset 4).
const F4_HEADER_SIZE: usize = 4;

// F4 config key tags (only the ones we care about).
const F4_TAG_TILT_PITCH: u16 = 0xA500;
const F4_TAG_GYRO_X: u16 = 0xA501;
const F4_TAG_GYRO_Z: u16 = 0xA502;
const F4_TAG_GYRO_Y: u16 = 0xA503;
const F4_TAG_SERIAL_LO: u16 = 0xA504;
const F4_TAG_SERIAL_HI: u16 = 0xA505;
const F4_TAG_GENERATION: u16 = 0xA506;
const F4_TAG_ODOMETER_LO: u16 = 0xA50C;
const F4_TAG_ODOMETER_HI: u16 = 0xA50D;
const F4_TAG_AMP_HOURS_LO: u16 = 0xA50E;
const F4_TAG_AMP_HOURS_HI: u16 = 0xA50F;
const F4_TAG_SIMPLESTOP: u16 = 0xA514;
const F4_TAG_BMS_SERIAL_LO: u16 = 0xA51E;
const F4_TAG_BMS_SERIAL_HI: u16 = 0xA51F;
const F4_TAG_HAPTIC_ENABLED: u16 = 0xA52E;
const F4_TAG_RECURVE_RAILS: u16 = 0xA535;

/// Parses a single F4 config sector by scanning its append-only log.
///
/// Returns `None` if the sector is not active.
fn parse_f4_sector(sector: &[u8]) -> Option<BackupConfig> {
    if sector.len() < F4_HEADER_SIZE {
        return None;
    }

    let header = u16::from_le_bytes(sector.get(..2)?.try_into().ok()?);
    if header != F4_SECTOR_ACTIVE {
        return None;
    }

    let mut config = BackupConfig::default();
    let mut offset = F4_HEADER_SIZE;

    while offset + 4 <= sector.len() {
        let value = u16::from_le_bytes(sector.get(offset..offset + 2)?.try_into().ok()?);
        let key_tag = u16::from_le_bytes(sector.get(offset + 2..offset + 4)?.try_into().ok()?);

        if key_tag == ERASED_U16 && value == ERASED_U16 {
            break;
        }

        match key_tag {
            F4_TAG_TILT_PITCH => config.tilt_pitch = Some(value),
            F4_TAG_GYRO_X => config.gyro_x_offset = Some(value),
            F4_TAG_GYRO_Z => config.gyro_z_offset = Some(value),
            F4_TAG_GYRO_Y => config.gyro_y_offset = Some(value),
            F4_TAG_SERIAL_LO => config.serial_lo = Some(value),
            F4_TAG_SERIAL_HI => config.serial_hi = Some(value),
            F4_TAG_ODOMETER_LO => config.odometer_lo = Some(value),
            F4_TAG_ODOMETER_HI => config.odometer_hi = Some(value),
            F4_TAG_GENERATION => config.generation = Some(value),
            F4_TAG_AMP_HOURS_LO => config.amp_hours_lo = Some(value),
            F4_TAG_AMP_HOURS_HI => config.amp_hours_hi = Some(value),
            F4_TAG_SIMPLESTOP => config.simplestop = Some(value),
            F4_TAG_BMS_SERIAL_LO => config.bms_serial_lo = Some(value),
            F4_TAG_BMS_SERIAL_HI => config.bms_serial_hi = Some(value),
            F4_TAG_HAPTIC_ENABLED => config.haptic_enabled = Some(value),
            F4_TAG_RECURVE_RAILS => config.haptic_enabled = Some(value),
            _ => {} // ignore all other tags
        }

        offset += 4;
    }

    Some(config)
}

/// Parses an F4 backup's config by trying Sector A, then Sector B.
pub fn parse_f4_config(sector_a: &[u8], sector_b: &[u8]) -> BackupConfig {
    parse_f4_sector(sector_a).or_else(|| parse_f4_sector(sector_b)).unwrap_or_default()
}

/// Reads the OTP serial from the F4 OTP sector.
pub fn read_f4_otp_serial(data: &[u8]) -> (Option<u16>, Option<u16>) {
    let lo = read_u16_le(data, F4_OTP_SERIAL_LO);
    let hi = read_u16_le(data, F4_OTP_SERIAL_HI);
    (lo, hi)
}

// ── Write-back functions ─────────────────────────────────────────────

/// Writes the essential config values back into the raw F1 backup data.
///
/// Only the primary config page is written; the backup page is wiped
/// to 0xFF. Writing both pages can cause crashes on some boards.
pub fn write_f1_config(data: &mut [u8], config: &BackupConfig) {
    let p = F1_CONFIG_PRIMARY_START;

    // Wipe the backup page and erase the primary page before writing.
    data.get_mut(F1_CONFIG_BACKUP_START..p).expect("backup too small for F1 config backup page").fill(0xFF);
    data.get_mut(p..F1_CONFIG_PRIMARY_END).expect("backup too small for F1 config primary page").fill(0xFF);

    // All fields written to primary page only.
    write_u16_le(data, p + F1_TILT_PITCH, config.tilt_pitch);
    write_u16_le(data, p + F1_GYRO_X, config.gyro_x_offset);
    write_u16_le(data, p + F1_GYRO_Z, config.gyro_z_offset);
    write_u16_le(data, p + F1_GYRO_Y, config.gyro_y_offset);
    write_u16_le(data, p + F1_GENERATION, config.generation);
    write_u16_le(data, p + F1_SIMPLESTOP, config.simplestop.or(Some(0)));
    write_u16_le(data, p + F1_SERIAL_LO, config.serial_lo);
    write_u16_le(data, p + F1_SERIAL_HI, config.serial_hi);

    let bms = combine_u16s(config.bms_serial_lo, config.bms_serial_hi);
    write_u32_le(data, p + F1_BMS_ID, bms);

    let odo = combine_u16s(config.odometer_lo, config.odometer_hi);
    write_u32_le(data, p + F1_ODOMETER, odo);

    let amp = combine_u16s(config.amp_hours_lo, config.amp_hours_hi);
    write_u32_le(data, p + F1_AMP_HOURS, amp);
}

/// Applies config edits to the raw F4 backup data.
///
/// Builds a fresh config sector from scratch containing only the
/// fields we track, so any untracked tags revert to defaults (`0xFF`).
/// Erases Sector B.
pub fn write_f4_config(data: &mut [u8], config: &BackupConfig) {
    let sector_size = F4_CONFIG_A_END - F4_CONFIG_A_START;
    let mut sector = vec![0xFF; sector_size];

    // Active header (0x0000) + 2 bytes padding.
    sector.get_mut(0..2).expect("sector too small for header").copy_from_slice(&F4_SECTOR_ACTIVE.to_le_bytes());

    // Append a record for each field that has a value.
    let fields: &[(u16, Option<u16>)] = &[
        (F4_TAG_TILT_PITCH, config.tilt_pitch),
        (F4_TAG_GYRO_X, config.gyro_x_offset),
        (F4_TAG_GYRO_Z, config.gyro_z_offset),
        (F4_TAG_GYRO_Y, config.gyro_y_offset),
        (F4_TAG_SERIAL_LO, config.serial_lo),
        (F4_TAG_SERIAL_HI, config.serial_hi),
        (F4_TAG_ODOMETER_LO, config.odometer_lo),
        (F4_TAG_ODOMETER_HI, config.odometer_hi),
        (F4_TAG_GENERATION, config.generation),
        (F4_TAG_AMP_HOURS_LO, config.amp_hours_lo),
        (F4_TAG_AMP_HOURS_HI, config.amp_hours_hi),
        (F4_TAG_BMS_SERIAL_LO, config.bms_serial_lo),
        (F4_TAG_BMS_SERIAL_HI, config.bms_serial_hi),
        (F4_TAG_SIMPLESTOP, config.simplestop.or(Some(0))),
        (F4_TAG_HAPTIC_ENABLED, config.haptic_enabled),
        (F4_TAG_RECURVE_RAILS, config.recurve_rails),
    ];

    let mut offset = F4_HEADER_SIZE;
    for &(tag, value) in fields {
        if let Some(val) = value {
            sector
                .get_mut(offset..offset + 2)
                .expect("sector overflow writing value")
                .copy_from_slice(&val.to_le_bytes());
            sector
                .get_mut(offset + 2..offset + 4)
                .expect("sector overflow writing tag")
                .copy_from_slice(&tag.to_le_bytes());
            offset += 4;
        }
    }

    data.get_mut(F4_CONFIG_A_START..F4_CONFIG_A_END)
        .expect("backup too small for F4 sector A")
        .copy_from_slice(&sector);
    data.get_mut(F4_CONFIG_B_START..F4_CONFIG_B_END).expect("backup too small for F4 sector B").fill(0xFF);

    // Write OTP serial — must match the config serial for firmware to accept it.
    write_u16_le(data, F4_OTP_SERIAL_LO, config.otp_serial_lo);
    write_u16_le(data, F4_OTP_SERIAL_HI, config.otp_serial_hi);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup::layout::{
        F1_CONFIG_BACKUP_START, F1_CONFIG_PRIMARY_START, F1_FLASH_SIZE, F4_CONFIG_A_END, F4_CONFIG_A_START,
        F4_CONFIG_B_END, F4_CONFIG_B_START, F4_FLASH_SIZE,
    };

    // ── F1 ───────────────────────────────────────────────────────

    /// Builds a minimal F1 config page (1024 bytes) with known test values.
    fn make_f1_config_page() -> Vec<u8> {
        let mut page = vec![0xFF; 1024];
        // tilt_pitch at offset 0x00
        page[0x00..0x02].copy_from_slice(&42_u16.to_le_bytes());
        // serial_lo at offset 0x0A
        page[0x0A..0x0C].copy_from_slice(&1234_u16.to_le_bytes());
        // odometer at offset 0x0C (u32 LE)
        page[0x0C..0x10].copy_from_slice(&50000_u32.to_le_bytes());
        // generation at offset 0x28
        page[0x28..0x2A].copy_from_slice(&3_u16.to_le_bytes());
        // serial_hi at offset 0x30
        page[0x30..0x32].copy_from_slice(&5678_u16.to_le_bytes());
        // simplestop at offset 0x3C
        page[0x3C..0x3E].copy_from_slice(&1_u16.to_le_bytes());
        page
    }

    #[test]
    fn f1_config_parse() {
        let page = make_f1_config_page();
        let config = parse_f1_config(&page);
        assert_eq!(config.tilt_pitch, Some(42));
        assert_eq!(config.serial_lo, Some(1234));
        assert_eq!(config.serial_hi, Some(5678));
        assert_eq!(config.odometer_lo, Some(50000_u32 as u16));
        assert_eq!(config.odometer_hi, Some((50000_u32 >> 16) as u16));
        assert_eq!(config.generation, Some(3));
        assert_eq!(config.simplestop, Some(1));
        assert_eq!(config.haptic_enabled, None);
        assert_eq!(config.recurve_rails, None);
    }

    #[test]
    fn f1_config_erased_fields_are_none() {
        let page = vec![0xFF; 1024]; // fully erased
        let config = parse_f1_config(&page);
        assert_eq!(config.serial_lo, None);
        assert_eq!(config.odometer_lo, None);
        assert_eq!(config.tilt_pitch, None);
        assert_eq!(config.generation, None);
        assert_eq!(config.simplestop, None);
    }

    #[test]
    fn f1_config_write_round_trip() {
        let page = make_f1_config_page();
        let mut config = parse_f1_config(&page);

        // Modify a value.
        config.serial_lo = Some(9999);
        config.serial_hi = Some(1111);

        // Write into an F1-sized backup.
        let mut data = vec![0xFF; F1_FLASH_SIZE];
        data[F1_CONFIG_PRIMARY_START..F1_CONFIG_PRIMARY_START + 1024].copy_from_slice(&page);
        data[F1_CONFIG_BACKUP_START..F1_CONFIG_BACKUP_START + 1024].copy_from_slice(&page);
        write_f1_config(&mut data, &config);

        // Re-parse and verify.
        let primary = &data[F1_CONFIG_PRIMARY_START..F1_CONFIG_PRIMARY_START + 1024];
        let reparsed = parse_f1_config(primary);
        assert_eq!(reparsed.serial_lo, Some(9999));
        assert_eq!(reparsed.serial_hi, Some(1111));
        // Tilt pitch should be preserved.
        assert_eq!(reparsed.tilt_pitch, Some(42));
        // New fields should be preserved.
        assert_eq!(reparsed.generation, Some(3));
        assert_eq!(reparsed.simplestop, Some(1));

        // Backup page should be wiped (all 0xFF).
        let backup = &data[F1_CONFIG_BACKUP_START..F1_CONFIG_BACKUP_START + 1024];
        assert!(backup.iter().all(|&b| b == 0xFF));
    }

    // ── F4 ───────────────────────────────────────────────────────

    /// Builds a minimal F4 config sector with a few known tag-value records.
    fn make_f4_sector() -> Vec<u8> {
        let sector_size = F4_CONFIG_A_END - F4_CONFIG_A_START;
        let mut sector = vec![0xFF; sector_size];
        // Active header (0x0000).
        sector[0..2].copy_from_slice(&F4_SECTOR_ACTIVE.to_le_bytes());
        sector[2..4].copy_from_slice(&[0xFF, 0xFF]); // padding

        // Record at offset 4: serial_lo = 1234, tag = 0xA504
        let mut offset = F4_HEADER_SIZE;
        sector[offset..offset + 2].copy_from_slice(&1234_u16.to_le_bytes());
        sector[offset + 2..offset + 4].copy_from_slice(&F4_TAG_SERIAL_LO.to_le_bytes());
        offset += 4;

        // Record: serial_hi = 5678, tag = 0xA505
        sector[offset..offset + 2].copy_from_slice(&5678_u16.to_le_bytes());
        sector[offset + 2..offset + 4].copy_from_slice(&F4_TAG_SERIAL_HI.to_le_bytes());
        offset += 4;

        // Record: generation = 3, tag = 0xA506
        sector[offset..offset + 2].copy_from_slice(&3_u16.to_le_bytes());
        sector[offset + 2..offset + 4].copy_from_slice(&F4_TAG_GENERATION.to_le_bytes());
        offset += 4;

        // Record: simplestop = 1, tag = 0xA514
        sector[offset..offset + 2].copy_from_slice(&1_u16.to_le_bytes());
        sector[offset + 2..offset + 4].copy_from_slice(&F4_TAG_SIMPLESTOP.to_le_bytes());
        offset += 4;

        // Record: haptic_enabled = 1, tag = 0xA52E
        sector[offset..offset + 2].copy_from_slice(&1_u16.to_le_bytes());
        sector[offset + 2..offset + 4].copy_from_slice(&F4_TAG_HAPTIC_ENABLED.to_le_bytes());
        offset += 4;

        // Record: recurve_rails = 1, tag = 0xA535
        sector[offset..offset + 2].copy_from_slice(&1_u16.to_le_bytes());
        sector[offset + 2..offset + 4].copy_from_slice(&F4_TAG_RECURVE_RAILS.to_le_bytes());

        sector
    }

    #[test]
    fn f4_sector_parse() {
        let sector = make_f4_sector();
        let config = parse_f4_sector(&sector).expect("active sector");
        assert_eq!(config.serial_lo, Some(1234));
        assert_eq!(config.serial_hi, Some(5678));
        assert_eq!(config.generation, Some(3));
        assert_eq!(config.simplestop, Some(1));
        assert_eq!(config.haptic_enabled, Some(1));
        assert_eq!(config.recurve_rails, Some(1));
        // Fields not in the sector should be None.
        assert_eq!(config.tilt_pitch, None);
    }

    #[test]
    fn f4_inactive_sector_returns_none() {
        let mut sector = vec![0xFF; F4_CONFIG_A_END - F4_CONFIG_A_START];
        // Non-zero header means inactive.
        sector[0..2].copy_from_slice(&0x1234_u16.to_le_bytes());
        assert!(parse_f4_sector(&sector).is_none());
    }

    #[test]
    fn f4_config_falls_back_to_sector_b() {
        let sector_size = F4_CONFIG_A_END - F4_CONFIG_A_START;
        let inactive = vec![0xFF; sector_size]; // inactive sector A
        let active = make_f4_sector(); // active sector B
        let config = parse_f4_config(&inactive, &active);
        assert_eq!(config.serial_lo, Some(1234));
    }

    #[test]
    fn f4_config_write_round_trip() {
        let sector = make_f4_sector();
        let mut config = parse_f4_sector(&sector).expect("active sector");
        config.serial_lo = Some(9999);

        // Build a minimal F4 backup with the sector placed correctly.
        let mut data = vec![0xFF; F4_FLASH_SIZE];
        data[F4_CONFIG_A_START..F4_CONFIG_A_END].copy_from_slice(&sector);
        write_f4_config(&mut data, &config);

        // Re-parse sector A.
        let reparsed_sector = &data[F4_CONFIG_A_START..F4_CONFIG_A_END];
        let reparsed = parse_f4_sector(reparsed_sector).expect("written sector should be active");
        assert_eq!(reparsed.serial_lo, Some(9999));
        assert_eq!(reparsed.serial_hi, Some(5678)); // preserved
        // New fields should be preserved.
        assert_eq!(reparsed.generation, Some(3));
        assert_eq!(reparsed.simplestop, Some(1));
        assert_eq!(reparsed.haptic_enabled, Some(1));
        assert_eq!(reparsed.recurve_rails, Some(1));

        // Sector B should be erased.
        assert!(data[F4_CONFIG_B_START..F4_CONFIG_B_END].iter().all(|&b| b == 0xFF));
    }
}
