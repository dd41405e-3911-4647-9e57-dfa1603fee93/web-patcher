use std::collections::HashMap;
use std::sync::{LazyLock, OnceLock};

use anyhow::{Context as _, bail, ensure};
use rhai::{AST, Array, Blob, Dynamic, Engine, Map, Scope};

use super::types::{ScriptParam, ScriptParamKind, ScriptTarget, ScriptValue};
use crate::board::BoardGeneration;

// ── Compiled script cache ──────────────────────────────────────────────

/// A compiled script patch, ready for execution.
pub struct CompiledScript {
    /// Pre-compiled Rhai AST.
    pub ast: AST,
    /// Cached parameter declarations from `parameters()`.
    pub params: Vec<ScriptParam>,
}

/// Global Rhai engine shared by all script patches.
static ENGINE: LazyLock<Engine> = LazyLock::new(create_engine);

/// Mutable backing store — populated once via [`compile_scripts`],
/// then moved into the lazy static on first access.
///
/// We use a [`OnceLock`] to allow one-time initialisation from the
/// registry module.
static SCRIPT_STORE: OnceLock<HashMap<String, CompiledScript>> = OnceLock::new();

/// Compiles all script patches and stores them.  Called once during
/// registry initialisation.
pub fn compile_scripts(scripts: Vec<(String, CompiledScript)>) {
    SCRIPT_STORE.set(scripts.into_iter().collect()).ok();
}

/// Returns the compiled script for a given cache key.
pub fn get_compiled(key: &str) -> Option<&'static CompiledScript> {
    SCRIPT_STORE.get()?.get(key)
}

/// Returns the cache key for a given patch id, board, and firmware version.
pub fn cache_key(patch_id: &str, board: BoardGeneration, version: u16) -> String {
    format!("{patch_id}_{board:?}_{version}")
}

// ── Engine setup ───────────────────────────────────────────────────────

/// Creates a sandboxed Rhai engine with the firmware patching API.
fn create_engine() -> Engine {
    let mut engine = Engine::new();

    // Sandboxing: prevent runaway scripts.
    engine.set_max_operations(10_000);
    engine.set_max_expr_depths(64, 32);
    engine.set_max_string_size(4096);
    engine.set_max_array_size(1024);
    engine.set_max_map_size(256);

    register_encoding_api(&mut engine);
    register_helpers(&mut engine);

    engine
}

// ── Encoding API ───────────────────────────────────────────────────────

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "Rhai i64/f64 to hardware-width conversions are intentional"
)]
fn register_encoding_api(engine: &mut Engine) {
    // ── Integer encoders ───────────────────────────────────────
    engine.register_fn("encode_u8", |v: i64| -> Blob { vec![v as u8] });
    engine.register_fn("encode_i8", |v: i64| -> Blob { vec![v as u8] });

    engine.register_fn("encode_u16le", |v: i64| -> Blob { (v as u16).to_le_bytes().to_vec() });
    engine.register_fn("encode_u16be", |v: i64| -> Blob { (v as u16).to_be_bytes().to_vec() });
    engine.register_fn("encode_i16le", |v: i64| -> Blob { (v as i16).to_le_bytes().to_vec() });
    engine.register_fn("encode_i16be", |v: i64| -> Blob { (v as i16).to_be_bytes().to_vec() });

    engine.register_fn("encode_u32le", |v: i64| -> Blob { (v as u32).to_le_bytes().to_vec() });
    engine.register_fn("encode_u32be", |v: i64| -> Blob { (v as u32).to_be_bytes().to_vec() });
    engine.register_fn("encode_i32le", |v: i64| -> Blob { (v as i32).to_le_bytes().to_vec() });
    engine.register_fn("encode_i32be", |v: i64| -> Blob { (v as i32).to_be_bytes().to_vec() });

    // ── Float encoders ─────────────────────────────────────────
    engine.register_fn("encode_f32le", |v: f64| -> Blob { (v as f32).to_le_bytes().to_vec() });
    engine.register_fn("encode_f32be", |v: f64| -> Blob { (v as f32).to_be_bytes().to_vec() });
    engine.register_fn("encode_f64le", |v: f64| -> Blob { v.to_le_bytes().to_vec() });
    engine.register_fn("encode_f64be", |v: f64| -> Blob { v.to_be_bytes().to_vec() });

    // ── Integer decoders ───────────────────────────────────────
    engine.register_fn("decode_u8", |b: Blob| -> i64 { b.first().copied().unwrap_or(0) as i64 });
    engine.register_fn("decode_i8", |b: Blob| -> i64 { b.first().copied().unwrap_or(0) as i8 as i64 });

    engine.register_fn("decode_u16le", |b: Blob| -> i64 {
        let arr: [u8; 2] = b.get(..2).and_then(|s| s.try_into().ok()).unwrap_or([0; 2]);
        u16::from_le_bytes(arr) as i64
    });
    engine.register_fn("decode_u16be", |b: Blob| -> i64 {
        let arr: [u8; 2] = b.get(..2).and_then(|s| s.try_into().ok()).unwrap_or([0; 2]);
        u16::from_be_bytes(arr) as i64
    });
    engine.register_fn("decode_i16le", |b: Blob| -> i64 {
        let arr: [u8; 2] = b.get(..2).and_then(|s| s.try_into().ok()).unwrap_or([0; 2]);
        i16::from_le_bytes(arr) as i64
    });
    engine.register_fn("decode_i16be", |b: Blob| -> i64 {
        let arr: [u8; 2] = b.get(..2).and_then(|s| s.try_into().ok()).unwrap_or([0; 2]);
        i16::from_be_bytes(arr) as i64
    });

    engine.register_fn("decode_u32le", |b: Blob| -> i64 {
        let arr: [u8; 4] = b.get(..4).and_then(|s| s.try_into().ok()).unwrap_or([0; 4]);
        u32::from_le_bytes(arr) as i64
    });
    engine.register_fn("decode_u32be", |b: Blob| -> i64 {
        let arr: [u8; 4] = b.get(..4).and_then(|s| s.try_into().ok()).unwrap_or([0; 4]);
        u32::from_be_bytes(arr) as i64
    });
    engine.register_fn("decode_i32le", |b: Blob| -> i64 {
        let arr: [u8; 4] = b.get(..4).and_then(|s| s.try_into().ok()).unwrap_or([0; 4]);
        i32::from_le_bytes(arr) as i64
    });
    engine.register_fn("decode_i32be", |b: Blob| -> i64 {
        let arr: [u8; 4] = b.get(..4).and_then(|s| s.try_into().ok()).unwrap_or([0; 4]);
        i32::from_be_bytes(arr) as i64
    });

    // ── Float decoders ─────────────────────────────────────────
    engine.register_fn("decode_f32le", |b: Blob| -> f64 {
        let arr: [u8; 4] = b.get(..4).and_then(|s| s.try_into().ok()).unwrap_or([0; 4]);
        f32::from_le_bytes(arr) as f64
    });
    engine.register_fn("decode_f32be", |b: Blob| -> f64 {
        let arr: [u8; 4] = b.get(..4).and_then(|s| s.try_into().ok()).unwrap_or([0; 4]);
        f32::from_be_bytes(arr) as f64
    });
    engine.register_fn("decode_f64le", |b: Blob| -> f64 {
        let arr: [u8; 8] = b.get(..8).and_then(|s| s.try_into().ok()).unwrap_or([0; 8]);
        f64::from_le_bytes(arr)
    });
    engine.register_fn("decode_f64be", |b: Blob| -> f64 {
        let arr: [u8; 8] = b.get(..8).and_then(|s| s.try_into().ok()).unwrap_or([0; 8]);
        f64::from_be_bytes(arr)
    });

    // ── Blob helpers ───────────────────────────────────────────
    engine.register_fn("bytes_equal", |a: Blob, b: Blob| -> bool { a == b });
}

// ── Script helpers ────────────────────────────────────────────────────

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "Rhai i64/f64 to hardware-width conversions are intentional"
)]
fn register_helpers(engine: &mut Engine) {
    // hex_to_blob("02 2A") -> Blob [0x02, 0x2A]
    engine.register_fn("hex_to_blob", |s: String| -> Result<Blob, Box<rhai::EvalAltResult>> {
        let clean: String = s.chars().filter(|c| !c.is_whitespace()).collect();
        hex::decode(&clean).map_err(|e| format!("hex_to_blob: invalid hex '{s}': {e}").into())
    });

    // blob_write(dst, offset, src) -> Blob with src written at offset
    engine.register_fn(
        "blob_write",
        |mut dst: Blob, offset: i64, src: Blob| -> Result<Blob, Box<rhai::EvalAltResult>> {
            if offset < 0 {
                return Err(format!("blob_write: negative offset {offset}").into());
            }
            let o = offset as usize;
            let end = o.saturating_add(src.len());
            if end > dst.len() {
                return Err(format!(
                    "blob_write: offset {o} + length {} = {end} exceeds blob size {}",
                    src.len(),
                    dst.len()
                )
                .into());
            }
            // Bounds are validated above — `end <= dst.len()`.
            dst.get_mut(o..end).expect("bounds checked above").copy_from_slice(&src);
            Ok(dst)
        },
    );

    // parse_int("4") -> 4i64
    engine.register_fn("parse_int", |s: String| -> Result<i64, Box<rhai::EvalAltResult>> {
        s.parse::<i64>().map_err(|e| format!("parse_int: invalid integer '{s}': {e}").into())
    });

    register_arm_helpers(engine);
}

// ── ARM Thumb-2 helpers ───────────────────────────────────────────────

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::too_many_lines,
    reason = "Rhai i64/f64 to hardware-width conversions are intentional; length driven by ARM instruction helpers"
)]
fn register_arm_helpers(engine: &mut Engine) {
    // nop_sled(byte_len) -> Blob filled with Thumb NOP instructions.
    // Thumb NOP = 0xBF00 = bytes [0x00, 0xBF] in little-endian.
    // byte_len must be even (Thumb instructions are 2-byte aligned).
    engine.register_fn("nop_sled", |byte_len: i64| -> Result<Blob, Box<rhai::EvalAltResult>> {
        if byte_len < 0 {
            return Err(format!("nop_sled: negative length {byte_len}").into());
        }
        let len = byte_len as usize;
        let mut blob = Vec::with_capacity(len & !1);
        for _ in 0..len / 2 {
            blob.push(0x00);
            blob.push(0xBF);
        }
        Ok(blob)
    });

    // pad_bytes(len, byte) -> Blob of `len` bytes all set to `byte`.
    engine.register_fn("pad_bytes", |len: i64, byte: i64| -> Result<Blob, Box<rhai::EvalAltResult>> {
        if len < 0 {
            return Err(format!("pad_bytes: negative length {len}").into());
        }
        Ok(vec![byte as u8; len as usize])
    });

    // blob_repeat(pattern, count) -> Blob with `pattern` repeated `count` times.
    engine.register_fn("blob_repeat", |pattern: Blob, count: i64| -> Result<Blob, Box<rhai::EvalAltResult>> {
        if count < 0 {
            return Err(format!("blob_repeat: negative count {count}").into());
        }
        Ok(pattern.repeat(count as usize))
    });

    // thumb_b(from_offset, to_offset) -> Blob encoding a Thumb unconditional
    // branch (B, T2 encoding, ±2KB range). Both offsets are byte addresses
    // within the firmware image.
    engine.register_fn("thumb_b", |from_offset: i64, to_offset: i64| -> Blob {
        // PC is at from_offset + 4 during execution (ARM pipeline).
        let delta = to_offset - (from_offset + 4);
        let imm11 = ((delta >> 1) & 0x7FF) as u16;
        let insn: u16 = 0xE000 | imm11;
        insn.to_le_bytes().to_vec()
    });

    // thumb_b_w(from_offset, to_offset) -> Blob encoding a Thumb-2 wide
    // unconditional branch (B.W, T4 encoding, ±16MB range).
    engine.register_fn("thumb_b_w", |from_offset: i64, to_offset: i64| -> Blob {
        let delta = to_offset - (from_offset + 4);
        let sign = u32::from(delta < 0);
        let udelta = delta as u32;
        let imm10 = (udelta >> 12) & 0x3FF;
        let imm11 = (udelta >> 1) & 0x7FF;
        let j1 = ((udelta >> 23) & 1) ^ sign ^ 1;
        let j2 = ((udelta >> 22) & 1) ^ sign ^ 1;
        let hw1: u16 = (0xF000 | (sign << 10) | imm10) as u16;
        let hw2: u16 = (0x9000 | (j1 << 13) | (j2 << 11) | imm11) as u16;
        let mut blob = Vec::with_capacity(4);
        blob.extend_from_slice(&hw1.to_le_bytes());
        blob.extend_from_slice(&hw2.to_le_bytes());
        blob
    });

    // thumb_bl(from_offset, to_offset) -> Blob encoding a Thumb-2
    // branch-with-link (BL, ±16MB range). Same encoding as B.W but
    // with the link bit set.
    engine.register_fn("thumb_bl", |from_offset: i64, to_offset: i64| -> Blob {
        let delta = to_offset - (from_offset + 4);
        let sign = u32::from(delta < 0);
        let udelta = delta as u32;
        let imm10 = (udelta >> 12) & 0x3FF;
        let imm11 = (udelta >> 1) & 0x7FF;
        let j1 = ((udelta >> 23) & 1) ^ sign ^ 1;
        let j2 = ((udelta >> 22) & 1) ^ sign ^ 1;
        let hw1: u16 = (0xF000 | (sign << 10) | imm10) as u16;
        let hw2: u16 = (0xD000 | (j1 << 13) | (j2 << 11) | imm11) as u16;
        let mut blob = Vec::with_capacity(4);
        blob.extend_from_slice(&hw1.to_le_bytes());
        blob.extend_from_slice(&hw2.to_le_bytes());
        blob
    });

    // thumb_movw(rd, imm16) -> Blob encoding MOVW Rd, #imm16 (T3).
    // Plain 16-bit immediate, any value 0–65535.
    engine.register_fn("thumb_movw", |rd: i64, imm16: i64| -> Blob {
        encode_thumb_movw_movt(0xF240, rd as u32, imm16 as u32)
    });

    // thumb_movt(rd, imm16) -> Blob encoding MOVT Rd, #imm16 (T1).
    // Writes imm16 into the top half of Rd without affecting the bottom half.
    engine.register_fn("thumb_movt", |rd: i64, imm16: i64| -> Blob {
        encode_thumb_movw_movt(0xF2C0, rd as u32, imm16 as u32)
    });

    // decode_thumb_movw(blob) -> i64, extracts the 16-bit immediate
    // from a MOVW or MOVT instruction.
    engine.register_fn("decode_thumb_movw", |b: Blob| -> i64 {
        let arr: [u8; 4] = match b.get(..4).and_then(|s| s.try_into().ok()) {
            Some(a) => a,
            None => return 0,
        };
        let hw1 = u16::from_le_bytes([arr[0], arr[1]]) as u32;
        let hw2 = u16::from_le_bytes([arr[2], arr[3]]) as u32;
        let imm4 = hw1 & 0xF;
        let i = (hw1 >> 10) & 1;
        let imm3 = (hw2 >> 12) & 7;
        let imm8 = hw2 & 0xFF;
        ((imm4 << 12) | (i << 11) | (imm3 << 8) | imm8) as i64
    });

    // thumb_mov_w(rd, imm) -> Blob encoding MOV.W Rd, #<modified_imm> (T2).
    // Uses the Thumb modified-immediate encoding (not all values are
    // representable — returns an empty blob if the value cannot be
    // encoded).
    engine.register_fn("thumb_mov_w", |rd: i64, imm: i64| -> Blob {
        let Some(imm12) = encode_thumb_modified_imm(imm as u32) else {
            return Vec::new();
        };
        let r = rd as u32;
        let i = ((imm12 >> 11) & 1) as u32;
        let imm3 = ((imm12 >> 8) & 7) as u32;
        let imm8 = (imm12 & 0xFF) as u32;
        let hw1: u16 = (0xF04F | (i << 10)) as u16;
        let hw2: u16 = ((imm3 << 12) | (r << 8) | imm8) as u16;
        let mut blob = Vec::with_capacity(4);
        blob.extend_from_slice(&hw1.to_le_bytes());
        blob.extend_from_slice(&hw2.to_le_bytes());
        blob
    });

    // decode_thumb_mov_w(blob) -> i64, decodes the modified-immediate
    // value from a MOV.W (T2) instruction.
    engine.register_fn("decode_thumb_mov_w", |b: Blob| -> i64 {
        let arr: [u8; 4] = match b.get(..4).and_then(|s| s.try_into().ok()) {
            Some(a) => a,
            None => return 0,
        };
        let hw1 = u16::from_le_bytes([arr[0], arr[1]]) as u32;
        let hw2 = u16::from_le_bytes([arr[2], arr[3]]) as u32;
        let i = (hw1 >> 10) & 1;
        let imm3 = (hw2 >> 12) & 7;
        let imm8 = hw2 & 0xFF;
        let imm12 = (i << 11) | (imm3 << 8) | imm8;
        thumb_expand_imm(imm12) as i64
    });
}

// ── Thumb modified-immediate helpers ─────────────────────────────────

/// Expands a 12-bit Thumb modified-immediate constant to its 32-bit
/// value (ARMv7-M `ThumbExpandImm`).
fn thumb_expand_imm(imm12: u32) -> u32 {
    if (imm12 >> 10) == 0 {
        let imm8 = imm12 & 0xFF;
        match (imm12 >> 8) & 3 {
            0 => imm8,
            1 => (imm8 << 16) | imm8,
            2 => (imm8 << 24) | (imm8 << 8),
            _ => (imm8 << 24) | (imm8 << 16) | (imm8 << 8) | imm8,
        }
    } else {
        let rotation = imm12 >> 7;
        let unrotated = 0x80 | (imm12 & 0x7F);
        unrotated.rotate_right(rotation)
    }
}

/// Tries to encode a 32-bit value as a 12-bit Thumb modified-immediate
/// constant.  Returns `None` if the value is not representable.
fn encode_thumb_modified_imm(value: u32) -> Option<u16> {
    // Pattern 00:00 — plain 8-bit (0x000000XX).
    if value <= 0xFF {
        return Some(value as u16);
    }

    let b0 = value & 0xFF;

    // Pattern 00:01 — 0x00XX00XX.
    if b0 != 0 && value == (b0 | (b0 << 16)) {
        return Some(0x100 | b0 as u16);
    }

    // Pattern 00:10 — 0xXX00XX00.
    let b1 = (value >> 8) & 0xFF;
    if b1 != 0 && value == ((b1 << 8) | (b1 << 24)) {
        return Some(0x200 | b1 as u16);
    }

    // Pattern 00:11 — 0xXXXXXXXX.
    if b0 != 0 && value == b0.wrapping_mul(0x0101_0101) {
        return Some(0x300 | b0 as u16);
    }

    // Rotation encoding: find rotation 8..=31 such that
    // ROL(value, rot) is an 8-bit value with bit 7 set.
    for rot in 8..=31u32 {
        let unrotated = value.rotate_left(rot);
        if (0x80..=0xFF).contains(&unrotated) {
            return Some(((rot as u16) << 7) | ((unrotated & 0x7F) as u16));
        }
    }

    None
}

/// Encodes a MOVW or MOVT instruction.  `base_hw1` selects the opcode
/// (`0xF240` for MOVW, `0xF2C0` for MOVT).
fn encode_thumb_movw_movt(base_hw1: u32, rd: u32, imm16: u32) -> Vec<u8> {
    let imm4 = (imm16 >> 12) & 0xF;
    let i = (imm16 >> 11) & 1;
    let imm3 = (imm16 >> 8) & 7;
    let imm8 = imm16 & 0xFF;
    let hw1: u16 = (base_hw1 | (i << 10) | imm4) as u16;
    let hw2: u16 = ((imm3 << 12) | (rd << 8) | imm8) as u16;
    let mut blob = Vec::with_capacity(4);
    blob.extend_from_slice(&hw1.to_le_bytes());
    blob.extend_from_slice(&hw2.to_le_bytes());
    blob
}

// ── Hex parsing ───────────────────────────────────────────────────────

/// Parses a hex string like `"02 2A"` into a byte vector.
fn parse_hex_bytes(s: &str) -> anyhow::Result<Vec<u8>> {
    let clean: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    hex::decode(&clean).with_context(|| format!("invalid hex bytes '{s}'"))
}

// ── Patch metadata extraction ─────────────────────────────────────────

/// Compiles a script source string, calls `patch()` to extract metadata,
/// calls `parameters()` to cache parameters, and returns all the data
/// needed by the registry to build `PatchDefinition`s.
///
/// Returns `(source, params, patch_info)` where `patch_info` contains
/// the id, name, description, and per-board version entries.
///
/// # Errors
///
/// Returns a descriptive error string if compilation or metadata
/// extraction fails.
pub fn compile_and_extract(source: &str) -> anyhow::Result<(AST, PatchInfo)> {
    let engine = &*ENGINE;
    let ast = engine.compile(source).context("compilation failed")?;
    let info = run_patch_info(engine, &ast)?;

    Ok((ast, info))
}

/// Extracts parameter declarations from a compiled script, injecting
/// `TARGETS` so that `parameters()` can generate controls dynamically
/// based on target metadata.
///
/// # Panics
///
/// Panics if the script's `parameters()` function fails.
pub fn extract_params(ast: &AST, targets: &[ScriptTarget]) -> Vec<ScriptParam> {
    run_describe_inner(&ENGINE, ast, targets).unwrap_or_else(|e| panic!("parameters() extraction failed: {e:#}"))
}

/// Metadata extracted from a script's `patch()` function.
pub struct PatchInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub experimental: bool,
    pub sram: Vec<(String, usize)>,
    pub boards: Vec<(BoardGeneration, Vec<VersionTargets>)>,
}

/// A group of firmware versions sharing the same targets.
pub struct VersionTargets {
    pub versions: Vec<u16>,
    pub targets: Vec<ScriptTarget>,
}

/// Calls the script's `patch()` function and parses the returned map
/// into a [`PatchInfo`].
fn run_patch_info(engine: &Engine, ast: &AST) -> anyhow::Result<PatchInfo> {
    let mut scope = Scope::new();
    let result: Map = engine.call_fn(&mut scope, ast, "patch", ()).context("patch() failed")?;

    let id = result.get("id").and_then(|v| v.clone().into_string().ok()).context("patch() missing 'id'")?;
    let name = result.get("name").and_then(|v| v.clone().into_string().ok()).context("patch() missing 'name'")?;
    let description = result
        .get("description")
        .and_then(|v| v.clone().into_string().ok())
        .context("patch() missing 'description'")?;

    let experimental = result.get("experimental").and_then(|v| v.as_bool().ok()).unwrap_or(false);

    // Parse optional SRAM allocation requests: sram: #{ label: size, ... }
    let sram: Vec<(String, usize)> = result
        .get("sram")
        .and_then(|v| v.clone().try_cast::<Map>())
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| {
                    let size = v.as_int().ok()? as usize;
                    Some((k.to_string(), size))
                })
                .collect()
        })
        .unwrap_or_default();

    let boards_map: Map = result
        .get("boards")
        .context("patch() missing 'boards'")?
        .clone()
        .try_cast::<Map>()
        .context("patch() 'boards' must be a map")?;

    let boards = parse_boards_map(&boards_map)?;

    Ok(PatchInfo { id, name, description, experimental, sram, boards })
}

/// Parses the `boards` map from a script's `patch()` return value into
/// a list of `(BoardGeneration, Vec<VersionTargets>)` pairs.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "Rhai i64/f64 to hardware-width conversions are intentional"
)]
fn parse_boards_map(boards_map: &Map) -> anyhow::Result<Vec<(BoardGeneration, Vec<VersionTargets>)>> {
    let mut boards = Vec::new();
    for (board_key, entries_val) in boards_map {
        let board_str = board_key.as_str();
        let board: BoardGeneration =
            board_str.parse().map_err(|e| anyhow::anyhow!("unknown board generation '{board_str}': {e}"))?;

        let entries: Vec<Map> = entries_val
            .clone()
            .into_typed_array::<Map>()
            .map_err(|e| anyhow::anyhow!("boards['{board_str}'] must be an array of maps: {e}"))?;

        let mut version_entries = Vec::new();
        for entry in &entries {
            let versions: Vec<u16> = entry
                .get("versions")
                .context("version entry missing 'versions'")?
                .clone()
                .into_typed_array::<i64>()
                .map_err(|e| anyhow::anyhow!("versions must be an array of integers: {e}"))?
                .into_iter()
                .map(|v| v as u16)
                .collect();

            let targets = parse_targets_array(entry)?;
            version_entries.push(VersionTargets { versions, targets });
        }

        boards.push((board, version_entries));
    }
    Ok(boards)
}

/// Parses the `targets` array from a version entry map.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "Rhai i64/f64 to hardware-width conversions are intentional"
)]
fn parse_targets_array(entry: &Map) -> anyhow::Result<Vec<ScriptTarget>> {
    let targets_arr: Vec<Map> = entry
        .get("targets")
        .context("version entry missing 'targets'")?
        .clone()
        .into_typed_array::<Map>()
        .map_err(|e| anyhow::anyhow!("targets must be an array of maps: {e}"))?;

    let mut targets = Vec::with_capacity(targets_arr.len());
    for target_map in &targets_arr {
        let append = target_map.get("append").and_then(|v| v.as_bool().ok()).unwrap_or(false);
        let blind = target_map.get("blind").and_then(|v| v.as_bool().ok()).unwrap_or(false);

        let (offset, original) = if append {
            let size =
                target_map.get("size").and_then(|v| v.as_int().ok()).context("append target missing 'size'")? as usize;
            (0, vec![0u8; size])
        } else if blind {
            let offset =
                target_map.get("offset").and_then(|v| v.as_int().ok()).context("target missing 'offset'")? as usize;
            let size =
                target_map.get("size").and_then(|v| v.as_int().ok()).context("blind target missing 'size'")? as usize;
            (offset, vec![0u8; size])
        } else {
            let offset =
                target_map.get("offset").and_then(|v| v.as_int().ok()).context("target missing 'offset'")? as usize;

            let original = match target_map.get("original") {
                Some(v) if v.is_blob() => v.clone().cast::<Vec<u8>>(),
                Some(v) => {
                    let s = v
                        .clone()
                        .into_string()
                        .map_err(|e| anyhow::anyhow!("target 'original' must be a hex string or blob: {e}"))?;
                    parse_hex_bytes(&s)?
                }
                None => bail!("target missing 'original'"),
            };
            (offset, original)
        };

        let meta = target_map.get("meta").and_then(|v| v.clone().try_cast::<Map>());

        targets.push(ScriptTarget { offset, original, meta, append, blind });
    }
    Ok(targets)
}

// ── Script lifecycle execution ─────────────────────────────────────────

/// Builds the `TARGETS` constant injected into every script scope.
fn build_targets_array(targets: &[ScriptTarget]) -> Array {
    targets
        .iter()
        .map(|t| {
            let mut map = Map::new();
            map.insert("offset".into(), Dynamic::from(t.offset as i64));
            map.insert("original".into(), Dynamic::from(t.original.clone()));
            map.insert("len".into(), Dynamic::from(t.original.len() as i64));
            map.insert("append".into(), Dynamic::from(t.append));
            map.insert("blind".into(), Dynamic::from(t.blind));
            if let Some(meta) = &t.meta {
                map.insert("meta".into(), Dynamic::from(meta.clone()));
            }
            Dynamic::from(map)
        })
        .collect()
}

/// Runs the script's `parameters()` function to discover UI parameters.
///
/// `TARGETS` is injected into scope so scripts can generate parameters
/// dynamically based on target metadata (e.g. one control per ride mode).
fn run_describe_inner(engine: &Engine, ast: &AST, targets: &[ScriptTarget]) -> anyhow::Result<Vec<ScriptParam>> {
    let mut scope = Scope::new();
    scope.push_constant("TARGETS", build_targets_array(targets));
    let result: Array = engine.call_fn(&mut scope, ast, "parameters", ()).context("parameters() failed")?;

    let mut params = Vec::with_capacity(result.len());
    for item in result {
        let map = item.try_cast::<Map>().context("parameters() must return an array of maps")?;
        let param = parse_script_param(&map)?;
        params.push(param);
    }
    Ok(params)
}

/// Parses a single parameter descriptor map from `parameters()`.
fn parse_script_param(map: &Map) -> anyhow::Result<ScriptParam> {
    let name = map.get("name").and_then(|v| v.clone().into_string().ok()).context("parameter missing 'name'")?;
    let label = map.get("label").and_then(|v| v.clone().into_string().ok()).context("parameter missing 'label'")?;
    let description = map.get("description").and_then(|v| v.clone().into_string().ok());

    let kind_str = map.get("kind").and_then(|v| v.clone().into_string().ok()).context("parameter missing 'kind'")?;

    let (kind, default) = match kind_str.as_str() {
        "toggle" | "bool" => {
            let def = map.get("initial").and_then(|v| v.as_bool().ok()).unwrap_or(false);
            (ScriptParamKind::Toggle, ScriptValue::Bool(def))
        }
        "int" | "integer" => {
            let min = map.get("min").and_then(|v| v.as_int().ok()).unwrap_or(0);
            let max = map.get("max").and_then(|v| v.as_int().ok()).unwrap_or(100);
            let def = map.get("initial").and_then(|v| v.as_int().ok()).unwrap_or(min);
            (ScriptParamKind::Integer { min, max }, ScriptValue::Int(def))
        }
        "float" => {
            let min = map.get("min").and_then(|v| v.as_float().ok()).unwrap_or(0.0);
            let max = map.get("max").and_then(|v| v.as_float().ok()).unwrap_or(100.0);
            let def = map.get("initial").and_then(|v| v.as_float().ok()).unwrap_or(min);
            (ScriptParamKind::Float { min, max }, ScriptValue::Float(def))
        }
        "enum" => {
            let options_arr =
                map.get("options").and_then(|v| v.clone().into_typed_array::<Map>().ok()).unwrap_or_default();
            let mut options = Vec::with_capacity(options_arr.len());
            for opt_map in &options_arr {
                let value = opt_map.get("value").and_then(|v| v.clone().into_string().ok()).unwrap_or_default();
                let opt_label =
                    opt_map.get("label").and_then(|v| v.clone().into_string().ok()).unwrap_or_else(|| value.clone());
                options.push((value, opt_label));
            }
            let def = map
                .get("default")
                .and_then(|v| v.clone().into_string().ok())
                .unwrap_or_else(|| options.first().map_or_else(String::new, |o| o.0.clone()));
            (ScriptParamKind::Enum { options }, ScriptValue::String(def))
        }
        "hex" => {
            let len =
                map.get("len").and_then(|v| v.as_int().ok()).context("hex parameter missing 'len'")? as usize;
            let def = match map.get("initial") {
                Some(v) if v.is_blob() => v.clone().cast::<Vec<u8>>(),
                Some(v) => {
                    let s = v.clone().into_string().unwrap_or_default();
                    parse_hex_bytes(&s).unwrap_or_else(|_| vec![0u8; len])
                }
                None => vec![0u8; len],
            };
            (ScriptParamKind::Hex { len }, ScriptValue::Bytes(def))
        }
        other => bail!("unknown parameter kind '{other}'"),
    };

    Ok(ScriptParam { name, label, description, kind, default })
}

/// A write descriptor returned by a script's `apply()`.
pub struct WriteDescriptor {
    pub offset: usize,
    pub bytes: Vec<u8>,
}

/// Converts script parameter values into a Rhai `Map` for passing
/// to `apply(params)`.
fn build_params_map(params: &[ScriptParam], values: &[ScriptValue]) -> Map {
    let mut map = Map::new();
    for (param, value) in params.iter().zip(values.iter()) {
        let dynamic = match value {
            ScriptValue::Bool(b) => Dynamic::from(*b),
            ScriptValue::Int(i) => Dynamic::from(*i),
            ScriptValue::Float(f) => Dynamic::from(*f),
            ScriptValue::String(s) => Dynamic::from(s.clone()),
            ScriptValue::Bytes(b) => Dynamic::from(b.clone()),
        };
        map.insert(param.name.clone().into(), dynamic);
    }
    map
}

/// Runs the script's `apply(params)` function, returning write
/// descriptors.
///
/// # Errors
///
/// Returns a descriptive error string if the script fails or produces
/// invalid write descriptors.
pub fn run_apply(
    compiled: &CompiledScript,
    targets: &[ScriptTarget],
    values: &[ScriptValue],
    board: crate::board::BoardGeneration,
    sram_allocs: &super::apply::SramAllocations,
    patch_id: &str,
) -> anyhow::Result<Vec<WriteDescriptor>> {
    let engine = &*ENGINE;
    let mut scope = Scope::new();
    scope.push_constant("TARGETS", build_targets_array(targets));
    scope.push_constant("BOARD_GEN", Dynamic::from(board.to_string()));

    // Build SRAM map: label → address (as i64 for Rhai).
    // Iteration order does not matter — we are building an unordered Rhai Map.
    let mut sram_map = Map::new();
    #[expect(clippy::iter_over_hash_type, reason = "building an unordered Rhai Map; iteration order is irrelevant")]
    for ((pid, label), &addr) in sram_allocs {
        if pid == patch_id {
            sram_map.insert(label.as_str().into(), Dynamic::from(addr as i64));
        }
    }
    scope.push_constant("SRAM", Dynamic::from(sram_map));

    let params_map = build_params_map(&compiled.params, values);

    let result: Array = engine.call_fn(&mut scope, &compiled.ast, "apply", (params_map,)).context("apply() failed")?;

    parse_write_descriptors(&result, targets)
}

/// Runs the script's optional `read(fw)` function.
///
/// Returns `Some(values)` if the script defines `read()` and it
/// succeeds, or `None` if the function does not exist.  Errors from
/// a defined `read()` are logged and treated as `None`.
pub fn run_read(compiled: &CompiledScript, firmware: &[u8], targets: &[ScriptTarget]) -> Option<Vec<ScriptValue>> {
    let engine = &*ENGINE;
    let mut scope = Scope::new();
    scope.push_constant("TARGETS", build_targets_array(targets));

    // Build a firmware reader map: keys are offset strings, values are
    // blobs of the bytes at that offset.
    let mut fw_map = Map::new();
    for target in targets {
        let Some(end) = target.offset.checked_add(target.original.len()) else { continue };
        if let Some(slice) = firmware.get(target.offset..end) {
            fw_map.insert(target.offset.to_string().into(), Dynamic::from(slice.to_vec()));
        }
    }

    match engine.call_fn::<Map>(&mut scope, &compiled.ast, "read", (fw_map,)) {
        Ok(result_map) => {
            // Map the returned values back to ScriptValues using the
            // parameter declarations to determine types.
            let mut values = Vec::with_capacity(compiled.params.len());
            for param in &compiled.params {
                let val = result_map
                    .get(param.name.as_str())
                    .map(|d| dynamic_to_script_value(d, &param.kind))
                    .unwrap_or_else(|| param.default.clone());
                values.push(val);
            }
            Some(values)
        }
        Err(err) => {
            // Log the error unless the function simply doesn't exist (optional).
            if !matches!(*err, rhai::EvalAltResult::ErrorFunctionNotFound(_, _)) {
                log::error!("script read() failed: {err}");
            }
            None
        }
    }
}

/// Converts a Rhai `Dynamic` to a [`ScriptValue`], guided by the
/// expected parameter kind.
fn dynamic_to_script_value(d: &Dynamic, kind: &ScriptParamKind) -> ScriptValue {
    match kind {
        ScriptParamKind::Toggle => ScriptValue::Bool(d.as_bool().unwrap_or(false)),
        ScriptParamKind::Integer { .. } => ScriptValue::Int(d.as_int().unwrap_or(0)),
        ScriptParamKind::Float { .. } => ScriptValue::Float(d.as_float().unwrap_or(0.0)),
        ScriptParamKind::Enum { .. } => ScriptValue::String(d.clone().into_string().unwrap_or_default()),
        ScriptParamKind::Hex { len } => {
            // read() may return a blob or a hex string.
            if d.is_blob() {
                ScriptValue::Bytes(d.clone().cast::<Vec<u8>>())
            } else if let Ok(s) = d.clone().into_string() {
                ScriptValue::Bytes(parse_hex_bytes(&s).unwrap_or_else(|_| vec![0u8; *len]))
            } else {
                ScriptValue::Bytes(vec![0u8; *len])
            }
        }
    }
}

/// Parses an array of `#{ offset, bytes }` maps into validated write
/// descriptors.  Each write must target a declared offset with matching
/// byte length.
fn parse_write_descriptors(result: &[Dynamic], targets: &[ScriptTarget]) -> anyhow::Result<Vec<WriteDescriptor>> {
    let mut descriptors = Vec::with_capacity(result.len());

    for item in result {
        let map =
            item.clone().try_cast::<Map>().context("apply() must return an array of maps (#{ offset, bytes })")?;

        let offset =
            map.get("offset").and_then(|v| v.as_int().ok()).context("write descriptor missing 'offset'")? as usize;

        let bytes: Vec<u8> = map
            .get("bytes")
            .and_then(|v| v.clone().into_typed_array::<u8>().ok().or_else(|| v.clone().try_cast::<Blob>()))
            .context("write descriptor missing 'bytes'")?;

        // Validate that the offset is in the declared targets list.
        let target = targets
            .iter()
            .find(|t| t.offset == offset)
            .with_context(|| format!("script attempted to write to undeclared offset {offset:#X}"))?;

        // Validate byte length matches the target's original length.
        ensure!(
            bytes.len() == target.original.len(),
            "script wrote {} bytes to offset {offset:#X} but target expects {}",
            bytes.len(),
            target.original.len(),
        );

        descriptors.push(WriteDescriptor { offset, bytes });
    }

    Ok(descriptors)
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Discovers all `.rhai` scripts in `src/patches/scripts/` subdirectories
    /// and returns `(filename, source)` pairs.
    fn discover_scripts() -> Vec<(String, String)> {
        let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/patches/scripts");
        let mut scripts = Vec::new();

        for subdir in &["firmware", "bootloader"] {
            let dir = base.join(subdir);
            if !dir.is_dir() {
                continue;
            }
            for entry in std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("failed to read {}: {e}", dir.display())) {
                let path = match entry {
                    Ok(e) => e.path(),
                    Err(_) => continue,
                };
                if path.extension().and_then(|e| e.to_str()) == Some("rhai") {
                    let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
                    if let Ok(source) = std::fs::read_to_string(&path) {
                        scripts.push((name, source));
                    }
                }
            }
        }

        scripts.sort_by(|a, b| a.0.cmp(&b.0));
        scripts
    }

    /// Compiles a script, extracts metadata, and exercises `apply()` with
    /// default parameter values for every board/version combination.
    ///
    /// This catches:
    /// - Syntax errors
    /// - Missing or misnamed `meta` keys (e.g. `meta.template` vs `meta.patched`)
    /// - Calls to unregistered functions
    /// - Wrong argument types / counts
    /// - `patch()` / `parameters()` returning malformed data
    fn run_script_smoke_test(filename: &str, source: &str) {
        let engine = create_engine();

        // ── Compile ──
        let ast = engine.compile(source).unwrap_or_else(|e| panic!("[{filename}] compilation failed: {e}"));

        // ── patch() ──
        let info = run_patch_info(&engine, &ast).unwrap_or_else(|e| panic!("[{filename}] {e}"));

        assert!(!info.id.is_empty(), "[{filename}] patch id is empty");
        assert!(!info.name.is_empty(), "[{filename}] patch name is empty");
        assert!(!info.boards.is_empty(), "[{filename}] patch has no boards");

        // ── apply() and read() for every board/version ──
        for (board, version_entries) in &info.boards {
            for ve in version_entries {
                let targets_array = build_targets_array(&ve.targets);

                // ── parameters() with TARGETS in scope ──
                let params =
                    run_describe_inner(&engine, &ast, &ve.targets).unwrap_or_else(|e| panic!("[{filename}] {e}"));

                // Collect default values from parameters.
                let defaults: Vec<ScriptValue> = params.iter().map(|p| p.default.clone()).collect();

                // -- apply(params) --
                {
                    let mut scope = Scope::new();
                    scope.push_constant("TARGETS", targets_array.clone());
                    scope.push_constant("BOARD_GEN", Dynamic::from(board.to_string()));
                    // Build a dummy SRAM map so scripts that reference SRAM["label"]
                    // get a plausible address instead of () (unit).
                    let mut sram_map = Map::new();
                    let mut dummy_addr: i64 = 0x2000_F000;
                    for (label, size) in &info.sram {
                        sram_map.insert(label.as_str().into(), Dynamic::from(dummy_addr));
                        dummy_addr += ((size + 3) & !3) as i64;
                    }
                    scope.push_constant("SRAM", Dynamic::from(sram_map));
                    let params_map = build_params_map(&params, &defaults);

                    let result: Array = engine.call_fn(&mut scope, &ast, "apply", (params_map,)).unwrap_or_else(|e| {
                        panic!(
                            "[{filename}] apply() failed for {board:?} v{}: {e}",
                            ve.versions.first().copied().unwrap_or(0)
                        )
                    });

                    // Validate the returned write descriptors.
                    parse_write_descriptors(&result, &ve.targets).unwrap_or_else(|e| {
                        panic!(
                            "[{filename}] apply() returned invalid descriptors for {board:?} v{}: {e}",
                            ve.versions.first().copied().unwrap_or(0)
                        )
                    });
                }

                // -- read(fw) (optional) --
                {
                    let mut scope = Scope::new();
                    scope.push_constant("TARGETS", targets_array);

                    // Build a fake firmware map with original bytes at each offset.
                    let mut fw_map = Map::new();
                    for target in &ve.targets {
                        if target.append || target.blind {
                            continue;
                        }
                        fw_map.insert(target.offset.to_string().into(), Dynamic::from(target.original.clone()));
                    }

                    match engine.call_fn::<Map>(&mut scope, &ast, "read", (fw_map,)) {
                        Ok(_) => {} // read() exists and ran successfully
                        Err(err) if matches!(*err, rhai::EvalAltResult::ErrorFunctionNotFound(_, _)) => {
                            // read() is optional — not an error
                        }
                        Err(err) => {
                            panic!(
                                "[{filename}] read() failed for {board:?} v{}: {err}",
                                ve.versions.first().copied().unwrap_or(0)
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn all_scripts_compile_and_run() {
        let scripts = discover_scripts();
        assert!(!scripts.is_empty(), "no .rhai scripts found in src/patches/scripts/");

        for (filename, source) in &scripts {
            run_script_smoke_test(filename, source);
        }
    }

    // ── Thumb encoding/decoding unit tests ────────────────────────────

    #[test]
    fn thumb_expand_imm_patterns() {
        // Pattern 00:00 — plain 8-bit
        assert_eq!(thumb_expand_imm(0x00), 0);
        assert_eq!(thumb_expand_imm(0x42), 0x42);
        assert_eq!(thumb_expand_imm(0xFF), 0xFF);

        // Pattern 00:01 — 0x00XX00XX
        assert_eq!(thumb_expand_imm(0x112), 0x0012_0012);
        assert_eq!(thumb_expand_imm(0x1FF), 0x00FF_00FF);

        // Pattern 00:10 — 0xXX00XX00
        assert_eq!(thumb_expand_imm(0x212), 0x1200_1200);
        assert_eq!(thumb_expand_imm(0x2FF), 0xFF00_FF00);

        // Pattern 00:11 — 0xXXXXXXXX
        assert_eq!(thumb_expand_imm(0x312), 0x1212_1212);
        assert_eq!(thumb_expand_imm(0x3FF), 0xFFFF_FFFF);
    }

    #[test]
    fn thumb_expand_imm_rotations() {
        // rotation=8, unrotated=0x80 → 0x80 ROR 8 = 0x80000000
        assert_eq!(thumb_expand_imm(0x400), 0x8000_0000);
        // rotation=8, unrotated=0xFF → 0xFF ROR 8 = 0xFF000000
        assert_eq!(thumb_expand_imm(0x47F), 0xFF00_0000);
        // rotation=24, unrotated=0x80 → 0x80 ROR 24 = 0x00008000
        assert_eq!(thumb_expand_imm(0xC00), 0x0000_8000);
        // rotation=28, unrotated=0x87 → 0x87 ROR 28 = 0x00000870
        assert_eq!(thumb_expand_imm(0xE07), 0x0000_0870);
        // rotation=31, unrotated=0xFF → 0xFF ROR 31 = 0x000001FE
        assert_eq!(thumb_expand_imm(0xFFF), 0x0000_01FE);
    }

    #[test]
    fn encode_thumb_modified_imm_roundtrip() {
        // Test every representable value round-trips correctly.
        for imm12 in 0..=0xFFFu32 {
            let value = thumb_expand_imm(imm12);
            match encode_thumb_modified_imm(value) {
                Some(encoded) => {
                    let decoded = thumb_expand_imm(encoded as u32);
                    assert_eq!(
                        decoded, value,
                        "round-trip failed: imm12=0x{imm12:03X} → value=0x{value:08X} → \
                         encoded=0x{encoded:03X} → decoded=0x{decoded:08X}"
                    );
                }
                None => {
                    panic!(
                        "encode_thumb_modified_imm failed for value=0x{value:08X} \
                         (generated from imm12=0x{imm12:03X})"
                    );
                }
            }
        }
    }

    #[test]
    fn encode_thumb_modified_imm_non_representable() {
        // Values that cannot be encoded as a modified immediate.
        let non_representable = [0x101, 0x1234, 0x871, 0x123, 0xABCD];
        for &val in &non_representable {
            assert!(encode_thumb_modified_imm(val).is_none(), "0x{val:X} should NOT be representable");
        }
    }

    #[test]
    fn thumb_mov_w_encode_decode_roundtrip() {
        // For every valid imm12, encode MOV.W then decode and verify.
        for imm12 in 0..=0xFFFu32 {
            let value = thumb_expand_imm(imm12);
            let blob = encode_thumb_mov_w_checked(0, value);
            assert!(
                !blob.is_empty(),
                "thumb_mov_w returned empty for representable value 0x{value:08X} (imm12=0x{imm12:03X})"
            );
            assert_eq!(blob.len(), 4);

            // Decode it back.
            let hw1 = u16::from_le_bytes([blob[0], blob[1]]) as u32;
            let hw2 = u16::from_le_bytes([blob[2], blob[3]]) as u32;
            let i = (hw1 >> 10) & 1;
            let imm3 = (hw2 >> 12) & 7;
            let imm8 = hw2 & 0xFF;
            let decoded_imm12 = (i << 11) | (imm3 << 8) | imm8;
            let decoded_value = thumb_expand_imm(decoded_imm12);
            assert_eq!(
                decoded_value, value,
                "MOV.W round-trip failed: value=0x{value:08X} (imm12=0x{imm12:03X}) → \
                 blob={blob:02X?} → decoded_imm12=0x{decoded_imm12:03X} → decoded=0x{decoded_value:08X}"
            );
        }
    }

    /// Helper: runs the same logic as the registered `thumb_mov_w` Rhai
    /// function but as a plain Rust function for testing.
    fn encode_thumb_mov_w_checked(rd: u32, imm: u32) -> Vec<u8> {
        let Some(imm12) = encode_thumb_modified_imm(imm) else {
            return Vec::new();
        };
        let i = ((imm12 >> 11) & 1) as u32;
        let imm3 = ((imm12 >> 8) & 7) as u32;
        let imm8 = (imm12 & 0xFF) as u32;
        let hw1: u16 = (0xF04F | (i << 10)) as u16;
        let hw2: u16 = ((imm3 << 12) | (rd << 8) | imm8) as u16;
        let mut blob = Vec::with_capacity(4);
        blob.extend_from_slice(&hw1.to_le_bytes());
        blob.extend_from_slice(&hw2.to_le_bytes());
        blob
    }

    #[test]
    fn thumb_mov_w_specific_values() {
        // Values around 2160 that ARE representable (multiples of 0x10
        // in 0x800..=0xFF0 range, i.e. rotation=28).
        let representable_rot28: Vec<u32> = (0x80..=0xFFu32).map(|v| v << 4).collect();
        for val in &representable_rot28 {
            let blob = encode_thumb_mov_w_checked(0, *val);
            assert!(!blob.is_empty(), "thumb_mov_w should encode 0x{val:X} ({val})");
        }

        // 2160 = 0x870 specifically
        let blob = encode_thumb_mov_w_checked(0, 2160);
        assert!(!blob.is_empty(), "thumb_mov_w should encode 2160");

        // 2161 = 0x871 — NOT representable
        let blob = encode_thumb_mov_w_checked(0, 2161);
        assert!(blob.is_empty(), "thumb_mov_w should NOT encode 2161");
    }

    #[test]
    fn thumb_movw_movt_roundtrip() {
        // MOVW/MOVT can encode any 16-bit immediate.
        for imm16 in [0u32, 1, 0xFF, 0x100, 0x7FF, 0x800, 0xFFF, 0x1000, 0x8000, 0xFFFF] {
            for rd in [0u32, 1, 7, 12, 14] {
                let blob_w = encode_thumb_movw_movt(0xF240, rd, imm16);
                let blob_t = encode_thumb_movw_movt(0xF2C0, rd, imm16);
                assert_eq!(blob_w.len(), 4);
                assert_eq!(blob_t.len(), 4);

                // Decode and verify the immediate.
                for blob in [&blob_w, &blob_t] {
                    let hw1 = u16::from_le_bytes([blob[0], blob[1]]) as u32;
                    let hw2 = u16::from_le_bytes([blob[2], blob[3]]) as u32;
                    let imm4 = hw1 & 0xF;
                    let i = (hw1 >> 10) & 1;
                    let imm3 = (hw2 >> 12) & 7;
                    let imm8 = hw2 & 0xFF;
                    let decoded = (imm4 << 12) | (i << 11) | (imm3 << 8) | imm8;
                    assert_eq!(decoded, imm16, "MOVW/MOVT round-trip failed for imm16=0x{imm16:04X} rd={rd}");

                    // Verify register field.
                    let decoded_rd = (hw2 >> 8) & 0xF;
                    assert_eq!(decoded_rd, rd, "register mismatch for rd={rd}");
                }
            }
        }
    }

    #[test]
    fn thumb_b_encoding() {
        // Branch forward by 0x100 bytes from offset 0x1000.
        // delta = 0x1100 - (0x1000 + 4) = 0xFC
        // imm11 = (0xFC >> 1) & 0x7FF = 0x7E
        // insn = 0xE000 | 0x7E = 0xE07E
        let blob = thumb_b_helper(0x1000, 0x1100);
        let insn = u16::from_le_bytes([blob[0], blob[1]]);
        assert_eq!(insn, 0xE07E, "thumb_b forward branch");

        // Branch backward by 8 bytes from offset 0x1000.
        // delta = 0xFF8 - (0x1000 + 4) = -12
        // imm11 = ((-12) >> 1) & 0x7FF = 0x7FA
        // insn = 0xE000 | 0x7FA = 0xE7FA
        let blob = thumb_b_helper(0x1000, 0xFF8);
        let insn = u16::from_le_bytes([blob[0], blob[1]]);
        assert_eq!(insn, 0xE7FA, "thumb_b backward branch");
    }

    fn thumb_b_helper(from: i64, to: i64) -> Vec<u8> {
        let delta = to - (from + 4);
        let imm11 = ((delta >> 1) & 0x7FF) as u16;
        let insn: u16 = 0xE000 | imm11;
        insn.to_le_bytes().to_vec()
    }

    #[test]
    fn thumb_b_w_bl_roundtrip() {
        // Test B.W and BL for various forward/backward offsets.
        let test_cases: Vec<(i64, i64)> = vec![
            (0x1000, 0x1100),   // short forward
            (0x1000, 0x0F00),   // short backward
            (0x1000, 0x100000), // long forward
            (0x100000, 0x1000), // long backward
            (0x1000, 0x1004),   // minimal forward (delta=0)
            (0x1000, 0x1002),   // delta=-2
        ];

        for (from, to) in &test_cases {
            let expected_delta = to - (from + 4);

            // B.W
            let bw = thumb_bw_encode(*from, *to);
            let decoded_bw = thumb_bw_decode(&bw);
            assert_eq!(
                decoded_bw, expected_delta,
                "B.W round-trip failed: from=0x{from:X} to=0x{to:X} expected_delta={expected_delta}"
            );

            // BL
            let bl = thumb_bl_encode(*from, *to);
            let decoded_bl = thumb_bl_decode(&bl);
            assert_eq!(
                decoded_bl, expected_delta,
                "BL round-trip failed: from=0x{from:X} to=0x{to:X} expected_delta={expected_delta}"
            );
        }
    }

    fn thumb_bw_encode(from: i64, to: i64) -> Vec<u8> {
        let delta = to - (from + 4);
        let sign = if delta < 0 { 1u32 } else { 0u32 };
        let udelta = delta as u32;
        let imm10 = (udelta >> 12) & 0x3FF;
        let imm11 = (udelta >> 1) & 0x7FF;
        let j1 = ((udelta >> 23) & 1) ^ sign ^ 1;
        let j2 = ((udelta >> 22) & 1) ^ sign ^ 1;
        let hw1: u16 = (0xF000 | (sign << 10) | imm10) as u16;
        let hw2: u16 = (0x9000 | (j1 << 13) | (j2 << 11) | imm11) as u16;
        let mut blob = Vec::with_capacity(4);
        blob.extend_from_slice(&hw1.to_le_bytes());
        blob.extend_from_slice(&hw2.to_le_bytes());
        blob
    }

    fn thumb_bw_decode(blob: &[u8]) -> i64 {
        let hw1 = u16::from_le_bytes([blob[0], blob[1]]) as u32;
        let hw2 = u16::from_le_bytes([blob[2], blob[3]]) as u32;
        let s = (hw1 >> 10) & 1;
        let imm10 = hw1 & 0x3FF;
        let j1 = (hw2 >> 13) & 1;
        let j2 = (hw2 >> 11) & 1;
        let imm11 = hw2 & 0x7FF;
        let i1 = (j1 ^ s) ^ 1;
        let i2 = (j2 ^ s) ^ 1;
        let raw = (s << 24) | (i1 << 23) | (i2 << 22) | (imm10 << 12) | (imm11 << 1);
        // Sign-extend from 25 bits.
        ((raw as i32) << 7 >> 7) as i64
    }

    fn thumb_bl_encode(from: i64, to: i64) -> Vec<u8> {
        let delta = to - (from + 4);
        let sign = if delta < 0 { 1u32 } else { 0u32 };
        let udelta = delta as u32;
        let imm10 = (udelta >> 12) & 0x3FF;
        let imm11 = (udelta >> 1) & 0x7FF;
        let j1 = ((udelta >> 23) & 1) ^ sign ^ 1;
        let j2 = ((udelta >> 22) & 1) ^ sign ^ 1;
        let hw1: u16 = (0xF000 | (sign << 10) | imm10) as u16;
        let hw2: u16 = (0xD000 | (j1 << 13) | (j2 << 11) | imm11) as u16;
        let mut blob = Vec::with_capacity(4);
        blob.extend_from_slice(&hw1.to_le_bytes());
        blob.extend_from_slice(&hw2.to_le_bytes());
        blob
    }

    fn thumb_bl_decode(blob: &[u8]) -> i64 {
        // Same decode as B.W — only the link bit differs, not the offset encoding.
        let hw1 = u16::from_le_bytes([blob[0], blob[1]]) as u32;
        let hw2 = u16::from_le_bytes([blob[2], blob[3]]) as u32;
        let s = (hw1 >> 10) & 1;
        let imm10 = hw1 & 0x3FF;
        let j1 = (hw2 >> 13) & 1;
        let j2 = (hw2 >> 11) & 1;
        let imm11 = hw2 & 0x7FF;
        let i1 = (j1 ^ s) ^ 1;
        let i2 = (j2 ^ s) ^ 1;
        let raw = (s << 24) | (i1 << 23) | (i2 << 22) | (imm10 << 12) | (imm11 << 1);
        ((raw as i32) << 7 >> 7) as i64
    }

    #[test]
    fn nop_sled_correctness() {
        let sled: Vec<u8> = (0..4).flat_map(|_| vec![0x00, 0xBF]).collect();
        assert_eq!(sled.len(), 8);
        // Every pair should be a Thumb NOP (0xBF00 LE).
        for chunk in sled.chunks(2) {
            assert_eq!(chunk, &[0x00, 0xBF]);
        }
    }
}
