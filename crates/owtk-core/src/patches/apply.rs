use std::collections::{HashMap, HashSet};

use anyhow::ensure;

use super::scripting;
use super::types::{
    PatchDefinition, PatchDiffEntry, PatchDiffReport, PatchEntry, PatchError, PatchSelection, PatchStatus,
    PatchWriteRecord, ScriptTarget,
};
use crate::board::BoardGeneration;
use crate::crypto::cipher::RSA_SIG_SIZE;

/// Context needed by the patch apply pipeline for SRAM allocation.
///
/// Abstracts over [`FirmwareDescriptor`](crate::firmware::types::FirmwareDescriptor)
/// and [`BootloaderDescriptor`](crate::bootloader::types::BootloaderDescriptor) so
/// the same apply logic works for both image types.
#[derive(Debug)]
pub struct PatchApplyContext {
    pub board: BoardGeneration,
    pub version: u16,
    pub sram_free_start: Option<u32>,
    /// Whether the decrypted firmware has a trailing RSA signature.
    ///
    /// `true` for `AesCTR128DynIv` firmware (256-byte RSA-2048 signature),
    /// `false` for all other firmware and bootloader images.
    ///
    /// When set, the apply pipeline strips the signature before allocating
    /// appended code, then appends an `0xFF`-filled placeholder of
    /// [`RSA_SIG_SIZE`] bytes so the bootloader finds the expected trailing bytes.
    pub has_rsa_sig: bool,
}

/// Reads `len` bytes from `firmware` at `offset`.
///
/// Returns `None` if the range extends past the end of the firmware.
fn read_bytes(firmware: &[u8], offset: usize, len: usize) -> Option<&[u8]> {
    let end = offset.checked_add(len)?;
    firmware.get(offset..end)
}

/// Writes arbitrary bytes into `firmware` at `offset`.
fn write_bytes(firmware: &mut [u8], offset: usize, bytes: &[u8]) -> anyhow::Result<()> {
    let end = offset.checked_add(bytes.len()).ok_or_else(|| anyhow::anyhow!("offset {offset:#X} overflows"))?;
    ensure!(end <= firmware.len(), "write at {offset:#X} extends past firmware (need {end}, have {})", firmware.len());
    firmware
        .get_mut(offset..end)
        .ok_or_else(|| anyhow::anyhow!("slice {offset:#X}..{end} out of range"))?
        .copy_from_slice(bytes);
    Ok(())
}

/// Helper to build a [`PatchError`] from a patch id and message.
fn patch_err(id: &str, err: impl std::fmt::Display) -> PatchError {
    PatchError { patch_id: id.to_owned(), message: format!("{err:#}") }
}

/// Detects the current state of a patch in the loaded firmware.
///
/// Pure Rust comparison — no script involvement:
/// - All targets match `original` → [`PatchStatus::Stock`]
/// - All targets are readable but at least one differs → [`PatchStatus::Applied`]
/// - Any target extends past the firmware buffer → [`PatchStatus::Unknown`]
/// - All targets are blind/append (none checkable) → [`PatchStatus::Blind`]
pub fn detect_status(firmware: &[u8], definition: &PatchDefinition) -> PatchStatus {
    let mut all_stock = true;
    let mut any_checked = false;
    for target in &definition.targets {
        if target.append || target.blind {
            continue;
        }
        any_checked = true;
        match read_bytes(firmware, target.offset, target.original.len()) {
            Some(current) if current == target.original.as_slice() => {}
            Some(_) => all_stock = false,
            None => return PatchStatus::Unknown,
        }
    }
    if !any_checked {
        return PatchStatus::Blind;
    }
    if all_stock { PatchStatus::Stock } else { PatchStatus::Applied }
}

/// Returns `true` when at least one entry's current selection differs
/// from its initial (as-loaded) selection, meaning the output will
/// differ from the original firmware.
pub fn has_pending_patch_changes(entries: &[PatchEntry]) -> bool {
    entries.iter().any(|e| e.selection != e.initial_selection)
}

/// Clones `firmware`, applies every patch entry in `entries` to the
/// clone, and returns the patched copy.
///
/// The original `firmware` slice is never modified.  If any patch
/// fails the error is returned and the partially-modified clone is
/// discarded.
///
/// # Errors
///
/// Returns [`PatchError`] if allocation fails or a patch script produces
/// invalid write descriptors.
pub fn apply_patches_to_copy(
    image: &[u8],
    entries: &[PatchEntry],
    max_image_size: usize,
    ctx: &PatchApplyContext,
) -> Result<Vec<u8>, PatchError> {
    let (patched, _) = apply_patches_to_copy_with_report(image, entries, max_image_size, ctx)?;
    Ok(patched)
}

/// Like [`apply_patches_to_copy`], but also returns a [`PatchDiffReport`]
/// describing every byte that was changed and why.
///
/// The report maps each file offset to its virtual address on the MCU,
/// making it directly usable for verification in IDA Pro, Binary Ninja,
/// or similar tools.
///
/// # Errors
///
/// Returns [`PatchError`] if allocation fails or a patch script produces
/// invalid write descriptors.
pub fn apply_patches_to_copy_with_report(
    image: &[u8],
    entries: &[PatchEntry],
    max_image_size: usize,
    ctx: &PatchApplyContext,
) -> Result<(Vec<u8>, PatchDiffReport), PatchError> {
    // For firmware with RSA signatures (DynIV), strip the signature before
    // allocating.  Appended code must live *before* the signature so the
    // bootloader always finds it in the expected trailing bytes.
    // We can't re-sign, so the placeholder is just 0xFF fill.
    let work_image = if ctx.has_rsa_sig && image.len() > RSA_SIG_SIZE {
        image.get(..image.len() - RSA_SIG_SIZE).expect("len > RSA_SIG_SIZE")
    } else {
        image
    };

    // Reserve space for the signature placeholder that will be appended.
    let alloc_limit = if ctx.has_rsa_sig { max_image_size.saturating_sub(RSA_SIG_SIZE) } else { max_image_size };

    let alloc_result = allocate_appends(work_image, alloc_limit, entries).map_err(|e| patch_err("allocator", e))?;

    let sram_allocs = allocate_sram(ctx, entries).map_err(|e| patch_err("allocator", e))?;

    let mut patched = work_image.to_vec();
    if alloc_result.required_size > patched.len() {
        let aligned = (alloc_result.required_size + 15) & !15;
        patched.resize(aligned, 0xFF);
    }

    // Clear the old allocation region so previous shellcode doesn't linger.
    patched.get_mut(alloc_result.content_end..).expect("content_end within patched").fill(0xFF);

    // Write the allocation marker so future patch operations can reclaim
    // this space instead of stacking after old allocations.
    if let Some(offset) = alloc_result.marker_offset {
        write_bytes(&mut patched, offset, &ALLOC_MARKER).map_err(|e| patch_err("allocator", e))?;
    }

    let fw_base = ctx.board.mcu_family_from_board_gen().firmware_base_address();

    let mut diff_entries = Vec::new();
    for entry in entries {
        let writes = apply_single_patch(&mut patched, entry, &alloc_result.allocs, &sram_allocs, fw_base)?;
        if !writes.is_empty() {
            diff_entries.push(PatchDiffEntry {
                patch_id: entry.definition.id.clone(),
                patch_name: entry.definition.name.clone(),
                writes,
            });
        }
    }

    // Append an 0xFF signature placeholder so the bootloader finds the
    // expected number of trailing bytes.  We can't compute a valid RSA
    // signature, but the placeholder keeps the layout correct.
    if ctx.has_rsa_sig {
        patched.resize(patched.len() + RSA_SIG_SIZE, 0xFF);
    }

    let report = PatchDiffReport {
        board: ctx.board.to_string(),
        version: ctx.version,
        firmware_base: fw_base,
        patches: diff_entries,
    };

    Ok((patched, report))
}

/// Assigned SRAM address for a patch's named allocation, keyed by `(patch_id, label)`.
pub type SramAllocations = HashMap<(String, String), u32>;

/// Allocates SRAM addresses bottom-up from the start of free SRAM for all
/// active patches.
///
/// Uses the verified `sram_free_start` from the firmware descriptor (typically
/// the initial SP) as the base.  Allocations grow upward from that address,
/// with an overflow check against the MCU's physical SRAM end to prevent
/// allocating past the chip's actual memory.
///
/// Returns a map of `(patch_id, label) -> address`.
pub fn allocate_sram(ctx: &PatchApplyContext, entries: &[PatchEntry]) -> anyhow::Result<SramAllocations> {
    // If no patches need SRAM, skip the check entirely.
    let needs_sram =
        entries.iter().any(|e| !matches!(e.selection, PatchSelection::Disabled) && !e.definition.sram.is_empty());

    if !needs_sram {
        return Ok(SramAllocations::new());
    }

    let sram_free_start = ctx.sram_free_start.ok_or_else(|| {
        anyhow::anyhow!(
            "{} v{} has no verified SRAM free-start — cannot safely allocate patch variables",
            ctx.board,
            ctx.version,
        )
    })?;

    let sram_end = ctx.board.mcu_family_from_board_gen().sram_end();

    let mut requests: Vec<(String, String, usize)> = Vec::new();
    for entry in entries {
        if matches!(entry.selection, PatchSelection::Disabled) {
            continue;
        }
        for (label, size) in &entry.definition.sram {
            requests.push((entry.definition.id.clone(), label.clone(), *size));
        }
    }
    requests.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

    let mut allocs = SramAllocations::new();
    let mut cursor = sram_free_start;

    for (patch_id, label, size) in requests {
        let size_aligned = ((size + 3) & !3) as u32;
        let alloc_addr = cursor;
        cursor = cursor
            .checked_add(size_aligned)
            .ok_or_else(|| anyhow::anyhow!("SRAM allocation overflow for '{patch_id}':'{label}'"))?;

        ensure!(
            cursor <= sram_end,
            "SRAM allocation for '{patch_id}':'{label}' at {alloc_addr:#010X} \
             (end {cursor:#010X}) would exceed SRAM limit ({sram_end:#010X})",
        );

        allocs.insert((patch_id, label), alloc_addr);
    }

    Ok(allocs)
}

/// Assigned offset for an append target, keyed by `(patch_id, target_index)`.
pub type AppendAllocations = HashMap<(String, usize), usize>;

/// Thumb-2 `b.w .` (branch-to-self / infinite loop), used as flash
/// padding by some firmware images instead of plain `0xFF` fill.
const THUMB2_LOOP_PAD: [u8; 4] = [0xFF, 0xF7, 0xFE, 0xBF];

/// Magic marker written before the first flash allocation.  On
/// subsequent patch operations this lets [`find_content_end`] detect
/// where previous allocations began so the space can be reclaimed
/// instead of stacking new allocations after old ones.
const ALLOC_MARKER: [u8; 8] = *b"OWTK_PAT";

/// Strips trailing padding bytes from `data`, returning the index
/// one past the last non-padding byte.
///
/// Recognises two padding patterns:
/// - `0xFF` fill (standard flash erase value)
/// - `FF F7 FE BF` (Thumb-2 `b.w .` infinite-loop instruction)
fn strip_trailing_padding(data: &[u8]) -> usize {
    let mut end = data.len();
    loop {
        let prev = end;
        while end > 0 && data.get(end - 1).copied() == Some(0xFF) {
            end -= 1;
        }
        while end >= 4 && data.get(end - 4..end) == Some(&THUMB2_LOOP_PAD) {
            end -= 4;
        }
        if end == prev {
            break;
        }
    }
    end
}

/// Finds the end of actual firmware content by stripping trailing
/// padding **and** any previous allocation region marked by
/// [`ALLOC_MARKER`].
///
/// If a marker is found the content end is moved back to just before
/// it (minus any padding between real content and the marker), so
/// the allocation region is fully reclaimed.
fn find_content_end(data: &[u8]) -> usize {
    let end = strip_trailing_padding(data);

    // Scan backwards for the allocation marker.  Because the marker
    // sits right before the first allocation and allocations are near
    // the end of the image, `rposition` finds it quickly.
    if let Some(pos) = data.get(..end).unwrap_or_default().windows(ALLOC_MARKER.len()).rposition(|w| w == ALLOC_MARKER)
    {
        // Strip padding between the real firmware content and the marker.
        strip_trailing_padding(data.get(..pos).unwrap_or_default())
    } else {
        end
    }
}

/// Result of [`allocate_appends`]: the allocation map, total required
/// buffer size, and the offset where [`ALLOC_MARKER`] should be written
/// (if any allocations were made).
pub struct AppendAllocResult {
    pub allocs: AppendAllocations,
    pub required_size: usize,
    pub marker_offset: Option<usize>,
    /// End of real firmware content (before any allocations or padding).
    /// Everything from this offset onward should be cleared before writing
    /// new allocations.
    pub content_end: usize,
}

/// Collects all append targets from active patch entries and assigns
/// sequential offsets starting from the current firmware end.
///
/// A magic [`ALLOC_MARKER`] is reserved before the first allocation so
/// future patch operations can detect and reclaim the space.
pub fn allocate_appends(
    firmware: &[u8],
    max_firmware_size: usize,
    entries: &[PatchEntry],
) -> anyhow::Result<AppendAllocResult> {
    // Collect (patch_id, target_index, size) for all active append targets,
    // sorted by (patch_id, target_index) for determinism.
    let mut requests: Vec<(String, usize, usize)> = Vec::new();
    for entry in entries {
        if matches!(entry.selection, PatchSelection::Disabled) {
            continue;
        }
        for (i, target) in entry.definition.targets.iter().enumerate() {
            if target.append {
                let size = target.original.len().max(1);
                requests.push((entry.definition.id.clone(), i, size));
            }
        }
    }
    requests.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

    // Strip trailing padding (and any previous allocation region) to find
    // the actual firmware content end.  This ensures appended code is placed
    // inside the existing flash image (in the padding region) rather than
    // beyond it, which would collide with config pages on F1 boards.
    let content_len = find_content_end(firmware);

    let mut allocs = AppendAllocations::new();
    let mut cursor = (content_len + 3) & !3; // align to 4 bytes

    // Reserve space for the allocation marker before the first target.
    let marker_offset = if requests.is_empty() {
        None
    } else {
        let offset = cursor;
        cursor += ALLOC_MARKER.len();
        cursor = (cursor + 3) & !3;
        Some(offset)
    };

    for (patch_id, target_idx, size) in requests {
        ensure!(
            cursor.checked_add(size).is_some_and(|end| end <= max_firmware_size),
            "append allocation for patch '{patch_id}' target {target_idx} would exceed flash limit \
             (need {:#X}, limit {max_firmware_size:#X})",
            cursor + size,
        );
        allocs.insert((patch_id, target_idx), cursor);
        cursor += size;
        // Align next allocation to 4 bytes.
        cursor = (cursor + 3) & !3;
    }

    Ok(AppendAllocResult { allocs, required_size: cursor, marker_offset, content_end: content_len })
}

/// Creates a copy of targets with append offsets resolved from the allocation map.
///
/// Returns an error if an append target has no corresponding allocation entry,
/// which would mean writing to an unresolved offset.
fn resolve_targets(
    targets: &[ScriptTarget],
    allocs: &AppendAllocations,
    patch_id: &str,
) -> anyhow::Result<Vec<ScriptTarget>> {
    targets
        .iter()
        .enumerate()
        .map(|(i, t)| {
            if t.append {
                let &offset = allocs
                    .get(&(patch_id.to_owned(), i))
                    .ok_or_else(|| anyhow::anyhow!("missing append allocation for patch '{patch_id}' target {i}"))?;
                Ok(ScriptTarget { offset, original: t.original.clone(), meta: t.meta.clone(), append: true, blind: t.blind })
            } else {
                Ok(t.clone())
            }
        })
        .collect()
}

/// Applies a single patch entry using pre-computed allocations and
/// returns a [`PatchWriteRecord`] for every byte range that was written.
fn apply_single_patch(
    firmware: &mut [u8],
    entry: &PatchEntry,
    allocs: &AppendAllocations,
    sram_allocs: &SramAllocations,
    fw_base: u32,
) -> Result<Vec<PatchWriteRecord>, PatchError> {
    let def = entry.definition;
    let mut records = Vec::new();

    match &entry.selection {
        PatchSelection::Disabled => {
            // Revert to stock — only fixed targets with known original bytes.
            for target in &def.targets {
                if target.append || target.blind {
                    continue;
                }
                let old = read_bytes(firmware, target.offset, target.original.len()).unwrap_or(&[]).to_vec();
                if old != target.original {
                    records.push(PatchWriteRecord {
                        offset: target.offset,
                        address: fw_base + target.offset as u32,
                        old_bytes: old,
                        new_bytes: target.original.clone(),
                        is_append: false,
                    });
                }
                write_bytes(firmware, target.offset, &target.original).map_err(|e| patch_err(&def.id, e))?;
            }
        }
        PatchSelection::Values(values) => {
            let resolved = resolve_targets(&def.targets, allocs, &def.id).map_err(|e| patch_err(&def.id, e))?;

            // Verify fixed targets are in bounds.
            for target in &resolved {
                if target.append {
                    continue;
                }
                let len = target.original.len();
                if read_bytes(firmware, target.offset, len).is_none() {
                    return Err(patch_err(
                        &def.id,
                        format_args!("target at {:#X} extends past firmware", target.offset),
                    ));
                }
            }

            let key = scripting::cache_key(&def.id, def.board, def.version);
            let compiled = scripting::get_compiled(&key).ok_or_else(|| patch_err(&def.id, "script not compiled"))?;

            let descriptors = scripting::run_apply(compiled, &resolved, values, def.board, sram_allocs, &def.id)
                .map_err(|e| patch_err(&def.id, e))?;

            // Build a lookup of which targets are append targets.
            let append_offsets: HashSet<usize> = resolved.iter().filter(|t| t.append).map(|t| t.offset).collect();

            for desc in &descriptors {
                let old = read_bytes(firmware, desc.offset, desc.bytes.len()).unwrap_or(&[]).to_vec();
                records.push(PatchWriteRecord {
                    offset: desc.offset,
                    address: fw_base + desc.offset as u32,
                    old_bytes: old,
                    new_bytes: desc.bytes.clone(),
                    is_append: append_offsets.contains(&desc.offset),
                });
                write_bytes(firmware, desc.offset, &desc.bytes).map_err(|e| patch_err(&def.id, e))?;
            }
        }
    }

    Ok(records)
}

/// Builds a list of [`PatchEntry`]s for the given firmware, detecting the
/// current status of each patch and initialising the UI selection to
/// match.
pub fn build_patch_entries(firmware: &[u8], definitions: &[&'static PatchDefinition]) -> Vec<PatchEntry> {
    definitions
        .iter()
        .map(|def| {
            let status = detect_status(firmware, def);
            let key = scripting::cache_key(&def.id, def.board, def.version);

            // Always try to read back values from firmware via read().
            let read_values = scripting::get_compiled(&key)
                .and_then(|compiled| scripting::run_read(compiled, firmware, &def.targets));

            let selection = if status == PatchStatus::Applied {
                match &read_values {
                    Some(values) => PatchSelection::Values(values.clone()),
                    None => {
                        // read() not defined — use defaults from describe().
                        if let Some(compiled) = scripting::get_compiled(&key) {
                            PatchSelection::Values(compiled.params.iter().map(|p| p.default.clone()).collect())
                        } else {
                            PatchSelection::Disabled
                        }
                    }
                }
            } else {
                PatchSelection::Disabled
            };

            PatchEntry { definition: def, status, initial_selection: selection.clone(), selection, read_values }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::BoardGeneration;
    use crate::patches::types::{PatchTarget, ScriptValue};

    /// Helper to create a synthetic PatchDefinition for testing.
    fn make_def(targets: Vec<ScriptTarget>, sram: Vec<(String, usize)>) -> PatchDefinition {
        PatchDefinition {
            id: "test_patch".into(),
            name: "Test Patch".into(),
            description: "A test patch".into(),
            target: PatchTarget::Firmware,
            board: BoardGeneration::XR,
            version: 4142,
            targets,
            sram,
            experimental: false,
        }
    }

    /// Helper to leak a PatchDefinition so we get a &'static reference for PatchEntry.
    fn leak_def(def: PatchDefinition) -> &'static PatchDefinition {
        Box::leak(Box::new(def))
    }

    fn fixed_target(offset: usize, original: &[u8]) -> ScriptTarget {
        ScriptTarget { offset, original: original.to_vec(), meta: None, append: false, blind: false }
    }

    fn append_target(size: usize) -> ScriptTarget {
        ScriptTarget { offset: 0, original: vec![0u8; size], meta: None, append: true, blind: false }
    }

    // ── detect_status ────────────────────────────────────────────

    #[test]
    fn detect_status_stock() {
        let firmware = vec![0xAA, 0xBB, 0xCC, 0xDD, 0x00, 0x00];
        let def = make_def(vec![fixed_target(0, &[0xAA, 0xBB]), fixed_target(2, &[0xCC, 0xDD])], vec![]);
        assert_eq!(detect_status(&firmware, &def), PatchStatus::Stock);
    }

    #[test]
    fn detect_status_applied() {
        let firmware = vec![0xFF, 0xFF, 0xCC, 0xDD]; // first target differs
        let def = make_def(vec![fixed_target(0, &[0xAA, 0xBB]), fixed_target(2, &[0xCC, 0xDD])], vec![]);
        assert_eq!(detect_status(&firmware, &def), PatchStatus::Applied);
    }

    #[test]
    fn detect_status_unknown_out_of_bounds() {
        let firmware = vec![0xAA, 0xBB]; // only 2 bytes
        let def = make_def(vec![fixed_target(10, &[0xCC, 0xDD])], vec![]);
        assert_eq!(detect_status(&firmware, &def), PatchStatus::Unknown);
    }

    #[test]
    fn detect_status_skips_append_targets() {
        let firmware = vec![0xAA, 0xBB];
        let def = make_def(vec![fixed_target(0, &[0xAA, 0xBB]), append_target(64)], vec![]);
        assert_eq!(detect_status(&firmware, &def), PatchStatus::Stock);
    }

    // ── has_pending_patch_changes ────────────────────────────────

    #[test]
    fn no_changes_when_selections_match() {
        let def = leak_def(make_def(vec![], vec![]));
        let entry = PatchEntry {
            definition: def,
            status: PatchStatus::Stock,
            selection: PatchSelection::Disabled,
            initial_selection: PatchSelection::Disabled,
            read_values: None,
        };
        assert!(!has_pending_patch_changes(&[entry]));
    }

    #[test]
    fn changes_detected_when_selection_differs() {
        let def = leak_def(make_def(vec![], vec![]));
        let entry = PatchEntry {
            definition: def,
            status: PatchStatus::Stock,
            selection: PatchSelection::Values(vec![ScriptValue::Bool(true)]),
            initial_selection: PatchSelection::Disabled,
            read_values: None,
        };
        assert!(has_pending_patch_changes(&[entry]));
    }

    // ── allocate_sram ────────────────────────────────────────────

    #[test]
    fn sram_allocation_succeeds() {
        let def = leak_def(make_def(vec![], vec![("counter".into(), 4), ("buffer".into(), 16)]));
        let entry = PatchEntry {
            definition: def,
            status: PatchStatus::Stock,
            selection: PatchSelection::Values(vec![]),
            initial_selection: PatchSelection::Disabled,
            read_values: None,
        };
        let ctx = PatchApplyContext {
            board: BoardGeneration::XR,
            version: 4142,
            sram_free_start: Some(0x2000_4000),
            has_rsa_sig: false,
        };
        let allocs = allocate_sram(&ctx, &[entry]).expect("should succeed");
        assert_eq!(allocs.len(), 2, "both allocations should be present");
        // Allocations are sorted by (patch_id, label), so "buffer" < "counter".
        let buffer_addr = allocs[&("test_patch".into(), "buffer".into())];
        let counter_addr = allocs[&("test_patch".into(), "counter".into())];
        assert!(buffer_addr >= 0x2000_4000);
        assert!(counter_addr > buffer_addr, "counter should be after buffer (alphabetical order)");
        assert!(counter_addr + 4 <= BoardGeneration::XR.mcu_family_from_board_gen().sram_end());
    }

    #[test]
    fn sram_allocation_skips_disabled_patches() {
        let def = leak_def(make_def(vec![], vec![("counter".into(), 4)]));
        let entry = PatchEntry {
            definition: def,
            status: PatchStatus::Stock,
            selection: PatchSelection::Disabled,
            initial_selection: PatchSelection::Disabled,
            read_values: None,
        };
        let ctx = PatchApplyContext {
            board: BoardGeneration::XR,
            version: 4142,
            sram_free_start: Some(0x2000_4000),
            has_rsa_sig: false,
        };
        let allocs = allocate_sram(&ctx, &[entry]).expect("should succeed");
        assert!(allocs.is_empty());
    }

    #[test]
    fn sram_allocation_fails_without_free_start() {
        let def = leak_def(make_def(vec![], vec![("counter".into(), 4)]));
        let entry = PatchEntry {
            definition: def,
            status: PatchStatus::Stock,
            selection: PatchSelection::Values(vec![]),
            initial_selection: PatchSelection::Disabled,
            read_values: None,
        };
        let ctx =
            PatchApplyContext { board: BoardGeneration::XR, version: 4142, sram_free_start: None, has_rsa_sig: false };
        assert!(allocate_sram(&ctx, &[entry]).is_err());
    }

    #[test]
    fn sram_allocation_overflow() {
        let def = leak_def(make_def(vec![], vec![("huge".into(), 0x1_0000)]));
        let entry = PatchEntry {
            definition: def,
            status: PatchStatus::Stock,
            selection: PatchSelection::Values(vec![]),
            initial_selection: PatchSelection::Disabled,
            read_values: None,
        };
        // Start near the SRAM end so the allocation overflows.
        let ctx = PatchApplyContext {
            board: BoardGeneration::XR,
            version: 4142,
            sram_free_start: Some(0x2000_4FF0),
            has_rsa_sig: false,
        };
        assert!(allocate_sram(&ctx, &[entry]).is_err());
    }

    // ── allocate_appends ─────────────────────────────────────────

    #[test]
    fn append_allocation_succeeds() {
        let firmware = vec![0xAA; 1024]; // 1 KB firmware, no trailing 0xFF
        let def = leak_def(make_def(vec![append_target(64)], vec![]));
        let entry = PatchEntry {
            definition: def,
            status: PatchStatus::Stock,
            selection: PatchSelection::Values(vec![]),
            initial_selection: PatchSelection::Disabled,
            read_values: None,
        };
        let max_size = 0xC800;
        let result = allocate_appends(&firmware, max_size, &[entry]).expect("should succeed");
        assert_eq!(result.allocs.len(), 1);
        let offset = result.allocs[&("test_patch".into(), 0)];
        assert!(offset >= 1024, "append must be after firmware content");
        assert!(result.required_size <= max_size);
        assert!(result.marker_offset.is_some(), "marker should be present when there are allocations");
    }

    #[test]
    fn append_allocation_strips_trailing_padding() {
        // Firmware with content followed by 0xFF padding.
        let mut firmware = vec![0xAA; 512];
        firmware.extend(vec![0xFF; 512]); // 512 bytes of padding
        let def = leak_def(make_def(vec![append_target(32)], vec![]));
        let entry = PatchEntry {
            definition: def,
            status: PatchStatus::Stock,
            selection: PatchSelection::Values(vec![]),
            initial_selection: PatchSelection::Disabled,
            read_values: None,
        };
        let result = allocate_appends(&firmware, 0xC800, &[entry]).expect("should succeed");
        let offset = result.allocs[&("test_patch".into(), 0)];
        // Marker (8 bytes, 4-byte aligned) sits at 512, first alloc at 520.
        assert_eq!(result.marker_offset, Some(512));
        assert_eq!(offset, 520);
    }

    #[test]
    fn append_allocation_overflow() {
        let firmware = vec![0xAA; 0xC700]; // nearly full
        let def = leak_def(make_def(vec![append_target(0x200)], vec![]));
        let entry = PatchEntry {
            definition: def,
            status: PatchStatus::Stock,
            selection: PatchSelection::Values(vec![]),
            initial_selection: PatchSelection::Disabled,
            read_values: None,
        };
        assert!(allocate_appends(&firmware, 0xC800, &[entry]).is_err());
    }

    #[test]
    fn append_allocation_strips_thumb2_loop_padding() {
        // Firmware with content followed by Thumb-2 b.w . (infinite loop) padding.
        let mut firmware = vec![0xAA; 512];
        for _ in 0..64 {
            firmware.extend_from_slice(&[0xFF, 0xF7, 0xFE, 0xBF]); // 256 bytes of loop padding
        }
        let def = leak_def(make_def(vec![append_target(32)], vec![]));
        let entry = PatchEntry {
            definition: def,
            status: PatchStatus::Stock,
            selection: PatchSelection::Values(vec![]),
            initial_selection: PatchSelection::Disabled,
            read_values: None,
        };
        let result = allocate_appends(&firmware, 0xC800, &[entry]).expect("should succeed");
        let offset = result.allocs[&("test_patch".into(), 0)];
        // Marker at 512, first alloc at 520.
        assert_eq!(result.marker_offset, Some(512));
        assert_eq!(offset, 520);
    }

    #[test]
    fn find_content_end_reclaims_marked_allocations() {
        // Simulate a previously-patched firmware:
        // [content 512B][MARKER][old shellcode 64B][0xFF padding]
        let mut data = vec![0xAA; 512];
        data.extend_from_slice(&ALLOC_MARKER);
        data.extend(vec![0xBB; 64]); // old shellcode
        data.extend(vec![0xFF; 128]); // trailing padding
        // find_content_end should reclaim the old allocation region.
        assert_eq!(find_content_end(&data), 512);
    }

    #[test]
    fn alloc_marker_allows_repatch_at_same_offset() {
        // Simulate a previously-patched firmware image.
        let mut firmware = vec![0xAA; 512];
        firmware.extend(vec![0xFF; 16]); // padding
        firmware.extend_from_slice(&ALLOC_MARKER);
        firmware.extend(vec![0xBB; 64]); // old shellcode
        firmware.extend(vec![0xFF; 128]); // trailing padding

        let def = leak_def(make_def(vec![append_target(64)], vec![]));
        let entry = PatchEntry {
            definition: def,
            status: PatchStatus::Stock,
            selection: PatchSelection::Values(vec![]),
            initial_selection: PatchSelection::Disabled,
            read_values: None,
        };
        let result = allocate_appends(&firmware, 0xC800, &[entry]).expect("should succeed");
        // Marker should be placed right after content (512), reclaiming the old region.
        assert_eq!(result.marker_offset, Some(512));
    }

    #[test]
    fn find_content_end_mixed_padding() {
        // Content followed by 0xFF then Thumb-2 loop padding.
        let mut data = vec![0xAA; 100];
        data.extend(vec![0xFF; 8]);
        data.extend_from_slice(&[0xFF, 0xF7, 0xFE, 0xBF]);
        data.extend(vec![0xFF; 4]);
        assert_eq!(find_content_end(&data), 100);
    }

    #[test]
    fn find_content_end_no_padding() {
        let data = vec![0xAA; 100];
        assert_eq!(find_content_end(&data), 100);
    }

    #[test]
    fn find_content_end_empty() {
        assert_eq!(find_content_end(&[]), 0);
    }

    #[test]
    fn find_content_end_all_ff() {
        assert_eq!(find_content_end(&[0xFF; 64]), 0);
    }

    #[test]
    fn append_skips_disabled_patches() {
        let firmware = vec![0xAA; 1024];
        let def = leak_def(make_def(vec![append_target(64)], vec![]));
        let entry = PatchEntry {
            definition: def,
            status: PatchStatus::Stock,
            selection: PatchSelection::Disabled,
            initial_selection: PatchSelection::Disabled,
            read_values: None,
        };
        let result = allocate_appends(&firmware, 0xC800, &[entry]).expect("should succeed");
        assert!(result.allocs.is_empty());
        assert!(result.marker_offset.is_none(), "no marker when no allocations");
        // Required size is just the content length (stripped of trailing 0xFF).
        assert_eq!(result.required_size, 1024);
    }
}
