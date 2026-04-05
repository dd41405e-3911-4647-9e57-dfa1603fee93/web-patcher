use crate::board::BoardGeneration;

// ── Script patch types ─────────────────────────────────────────────────

/// A firmware location that a script patch may read or write.
///
/// Used for detection (comparing against stock bytes) and for
/// verification (ensuring offsets are within bounds before writing).
#[derive(Debug, Clone)]
pub struct ScriptTarget {
    /// Byte offset into the **decrypted** firmware image.
    pub offset: usize,
    /// Expected stock firmware bytes at this offset.
    pub original: Vec<u8>,
    /// Optional arbitrary metadata accessible to scripts as
    /// `TARGETS[i].meta["key"]`.  Stored as a Rhai map directly
    /// since it originates from and is consumed by Rhai scripts.
    pub meta: Option<rhai::Map>,
    /// When `true`, this target's space is dynamically allocated from the
    /// end of the firmware image rather than at a fixed offset.  The
    /// `offset` field is assigned by the allocator before `apply()` runs;
    /// `original` is empty.
    pub append: bool,
    /// When `true`, the original bytes at this offset are unknown (e.g.
    /// encryption keys that cannot be distributed for licensing reasons).
    /// The target uses `size` instead of `original` — `original` is a
    /// zero-filled placeholder whose only purpose is carrying the byte
    /// length.  Detection and revert both skip blind targets.
    pub blind: bool,
}

/// A parameter declared by a script's `parameters()` function for UI
/// rendering.
#[derive(Debug, Clone)]
pub struct ScriptParam {
    /// Machine-readable variable name (e.g. `"speed_mph"`).
    pub name: String,
    /// Human-readable label shown in the UI.
    pub label: String,
    /// Optional longer description / tooltip.
    pub description: Option<String>,
    /// The type and constraints of this parameter.
    pub kind: ScriptParamKind,
    /// Default value shown when the firmware is stock.
    pub default: ScriptValue,
}

/// The type of a script-declared parameter, used to select the
/// appropriate UI control.
#[derive(Debug, Clone)]
pub enum ScriptParamKind {
    /// A boolean toggle (checkbox).
    Toggle,
    /// An integer parameter with min/max bounds (drag slider).
    Integer { min: i64, max: i64 },
    /// A floating-point parameter with min/max bounds (drag slider).
    Float { min: f64, max: f64 },
    /// A selection from named options (combo box).
    /// Each tuple is `(value_name, display_label)`.
    Enum { options: Vec<(String, String)> },
    /// A fixed-length hex byte string input (e.g. for encryption keys).
    /// `len` is the expected number of **bytes** (not hex characters).
    Hex { len: usize },
}

/// A dynamically-typed value for script parameters.
#[derive(Debug, Clone, PartialEq)]
pub enum ScriptValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    /// Raw byte vector, typically entered as a hex string in the UI.
    Bytes(Vec<u8>),
}

// ── Patch definition ────────────────────────────────────────────────────

/// Whether a patch targets a firmware image or a bootloader image.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PatchTarget {
    Firmware,
    Bootloader,
}

impl std::fmt::Display for PatchTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Firmware => "Firmware",
            Self::Bootloader => "Bootloader",
        })
    }
}

/// Full definition of a patch targeting a specific firmware or bootloader
/// version.
///
/// Every patch is a Rhai script.  The script's `patch()` function
/// declares metadata and targets, `parameters()` declares UI parameters,
/// `apply(params)` produces write descriptors, and an optional `read(fw)`
/// reads back current values from the image.
///
/// Because byte offsets differ between versions, each
/// `(board, version)` pair that a conceptual patch supports gets its
/// own `PatchDefinition` entry.
#[derive(Debug, Clone)]
pub struct PatchDefinition {
    /// Machine-readable identifier shared across versions
    /// (e.g. `"battery_type"`).
    pub id: String,
    /// Human-readable name shown in the UI.
    pub name: String,
    /// Longer description / tooltip.
    pub description: String,
    /// Whether this patch targets a firmware or bootloader image.
    pub target: PatchTarget,
    /// Board generation this entry targets.
    pub board: BoardGeneration,
    /// Version number this entry targets (firmware or bootloader version).
    pub version: u16,
    /// Write targets with stock bytes for verification.  Scripts can
    /// only write to offsets declared here.
    pub targets: Vec<ScriptTarget>,
    /// SRAM allocation requests: `(label, size_bytes)` pairs.
    /// Addresses are assigned top-down from SRAM end at apply time.
    pub sram: Vec<(String, usize)>,
    /// Whether this patch is experimental / not fully tested.
    /// When `true` the UI shows a warning indicator.
    pub experimental: bool,
}

// ── Runtime state ───────────────────────────────────────────────────────

/// The user's current selection for a single patch.
#[derive(Debug, Clone, PartialEq)]
pub enum PatchSelection {
    /// Patch is not active — firmware stays at (or reverts to) stock
    /// bytes.
    Disabled,
    /// Patch is active with these parameter values (one per declared
    /// parameter from `parameters()`).
    Values(Vec<ScriptValue>),
}

/// We can't derive `Eq` because of [`ScriptValue::Float`], but for
/// all practical purposes (no NaN values from user input) `PartialEq`
/// is sufficient.
impl Eq for PatchSelection {}

/// Detected state of a patch in the loaded firmware.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchStatus {
    /// All targets' `original` bytes match — the firmware is stock for
    /// this patch.
    Stock,
    /// At least one target's bytes differ from stock.
    Applied,
    /// One or more target offsets extend past the firmware buffer.
    Unknown,
}

/// An individual patch entry tracked at runtime — pairs a static
/// definition with mutable UI state.
#[derive(Debug)]
pub struct PatchEntry {
    /// Points into the static patch registry (lives in a `LazyLock`).
    pub definition: &'static PatchDefinition,
    /// Detected state when the firmware was first loaded / last
    /// scanned.
    pub status: PatchStatus,
    /// The user's current selection in the UI.
    pub selection: PatchSelection,
    /// The selection that was set when the entry was built (i.e. what
    /// the firmware currently contains).  Used to determine whether
    /// the user has made any changes that need applying.
    pub initial_selection: PatchSelection,
    /// Values read back from firmware via `read()`, regardless of
    /// stock/applied status.  Used to populate controls with actual
    /// firmware values when the user enables a stock patch.
    pub read_values: Option<Vec<ScriptValue>>,
}

/// Errors that can occur when applying or reverting patches.
#[derive(Debug, thiserror::Error)]
#[error("patch '{patch_id}': {message}")]
pub struct PatchError {
    /// The `id` of the patch that failed.
    pub patch_id: String,
    /// Human-readable explanation of what went wrong.
    pub message: String,
}

// ── Diff report types ───────────────────────────────────────────────────

/// A single write operation performed during patch application.
///
/// Captures the exact bytes replaced so the patch can be verified in a
/// disassembler (IDA Pro, Binary Ninja, etc.) without re-running the
/// patcher.
#[derive(Debug, Clone)]
pub struct PatchWriteRecord {
    /// Byte offset into the decrypted firmware image.
    pub offset: usize,
    /// Virtual address on the target MCU (`firmware_base + offset`).
    pub address: u32,
    /// Bytes that were present before the write (from the stock or
    /// previous image).
    pub old_bytes: Vec<u8>,
    /// Bytes that were written.
    pub new_bytes: Vec<u8>,
    /// Whether this write targets dynamically-appended space (as
    /// opposed to overwriting an existing region).
    pub is_append: bool,
}

/// All writes performed by a single patch.
#[derive(Debug, Clone)]
pub struct PatchDiffEntry {
    /// Machine-readable patch identifier (e.g. `"modify_top_speed"`).
    pub patch_id: String,
    /// Human-readable patch name.
    pub patch_name: String,
    /// The individual writes this patch performed.
    pub writes: Vec<PatchWriteRecord>,
}

/// Complete diff report produced by [`apply_patches_to_copy_with_report`].
///
/// Contains everything needed to verify patches in a binary analysis tool:
/// the board, version, base address, and every byte that changed.
#[derive(Debug, Clone)]
pub struct PatchDiffReport {
    /// Board generation (e.g. `"GT"`).
    pub board: String,
    /// Firmware version number.
    pub version: u16,
    /// Flash base address for the firmware region on this MCU
    /// (e.g. `0x0802_0000` for F4, `0x0800_3000` for F1).
    pub firmware_base: u32,
    /// Per-patch diff entries.
    pub patches: Vec<PatchDiffEntry>,
}
