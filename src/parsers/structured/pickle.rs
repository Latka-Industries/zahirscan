//! Python pickle (`.pkl`, `.pickle`) — read-only opcode scan; no unpickling.
//!
//! Walks pickle opcodes like CPython's `pickletools.genops`, but never reconstructs
//! objects or executes import hooks. Large binary payloads (`BYTEARRAY8`, `BINBYTES8`, …)
//! are skipped by length so multi‑GB array dumps stay cheap to scan.
//!
//! A partial stack + memo simulation tracks enough state to resolve `GLOBAL`, `INST`, and
//! `STACK_GLOBAL` references. Protocol 4+ `MEMOIZE` copies the stack top into memo
//! **without popping** — real pickles rely on that before `STACK_GLOBAL`.

use std::collections::HashSet;

use anyhow::Result;
use memmap2::Mmap;

use crate::config::RuntimeConfig;
use crate::parsers::{ParseResult, traits::empty_mining_result};
use crate::results::{MiningResult, PickleMetadata};

const MAX_OPCODES: usize = 500_000;
const MAX_GLOBALS: usize = 256;

/// Upper bound on bytes read during one opcode walk.
struct ScanLimits {
    len: usize,
}

/// Accumulated results from [`scan_pickle_ops`].
struct PickleScan {
    protocols_seen: Vec<u8>,
    frame_count: usize,
    frame_bytes_total: u64,
    referenced_globals: Vec<String>,
    /// True when `EMPTY_DICT` / `EMPTY_LIST` / `EMPTY_TUPLE` opcodes appear (pure-builtin payloads).
    saw_builtin_container: bool,
    scan_truncated: bool,
    scan_error: Option<String>,
}

/// Stack slot during opcode simulation. Only `String` values participate in `STACK_GLOBAL`.
#[derive(Clone)]
enum StackSlot {
    String(String),
    Mark,
    Opaque,
}

/// Partial pickle VM state: operand stack + memo table for `BINGET` / `MEMOIZE`.
struct PickleStack {
    stack: Vec<StackSlot>,
    memo: Vec<StackSlot>,
}

impl PickleStack {
    fn new() -> Self {
        Self {
            stack: Vec::new(),
            memo: Vec::new(),
        }
    }

    fn push_string(&mut self, s: String) {
        self.stack.push(StackSlot::String(s));
    }

    fn push_opaque(&mut self) {
        self.stack.push(StackSlot::Opaque);
    }

    fn push_mark(&mut self) {
        self.stack.push(StackSlot::Mark);
    }

    fn pop(&mut self) -> Option<StackSlot> {
        self.stack.pop()
    }

    fn dup_top(&mut self) {
        if let Some(top) = self.stack.last().cloned() {
            self.stack.push(top);
        }
    }

    fn pop_mark(&mut self) {
        while let Some(slot) = self.stack.pop() {
            if matches!(slot, StackSlot::Mark) {
                break;
            }
        }
    }

    fn pop_until_mark(&mut self) {
        while let Some(top) = self.stack.last() {
            if matches!(top, StackSlot::Mark) {
                self.stack.pop();
                break;
            }
            self.stack.pop();
        }
    }

    /// Protocol 4+ `MEMOIZE`: append stack top to memo; does **not** pop (CPython behavior).
    fn memoize_top(&mut self) {
        let Some(top) = self.stack.last().cloned() else {
            return;
        };
        self.memo.push(top);
    }

    fn memo_put(&mut self, idx: usize) {
        let Some(top) = self.stack.last().cloned() else {
            return;
        };
        if self.memo.len() <= idx {
            self.memo.resize(idx + 1, StackSlot::Opaque);
        }
        self.memo[idx] = top;
    }

    fn memo_get(&mut self, idx: usize) {
        if let Some(slot) = self.memo.get(idx) {
            self.stack.push(slot.clone());
        } else {
            self.push_opaque();
        }
    }

    /// `STACK_GLOBAL`: pop `name`, then `module`; record `module.name` when both are strings.
    fn stack_global(&mut self, out: &mut Vec<String>, seen: &mut HashSet<String>) {
        if self.stack.len() < 2 {
            self.push_opaque();
            return;
        }
        let name = self.pop();
        let module = self.pop();
        if let (Some(StackSlot::String(name)), Some(StackSlot::String(module))) = (name, module)
            && is_valid_global_reference(&module, &name)
        {
            push_global(out, seen, &module, &name);
        }
        self.push_opaque();
    }
}

/// Extract pickle metadata via header sniff and read-only opcode walk.
///
/// # Errors
///
/// Currently always returns [`Ok`].
pub fn extract_pickle_metadata(mmap: &Mmap, stats: &ParseResult) -> Result<PickleMetadata> {
    let bytes = mmap.as_ref();
    let scan = scan_pickle_ops(bytes);
    let (builtin_types, referenced_globals): (Vec<_>, Vec<_>) = scan
        .referenced_globals
        .into_iter()
        .partition(|r| is_builtin_type(r.as_str()));
    let hint = content_hint(
        &referenced_globals,
        &builtin_types,
        scan.saw_builtin_container,
    );

    Ok(PickleMetadata {
        byte_count: stats.byte_count,
        protocol: header_protocol(bytes),
        encoding: pickle_encoding(bytes).to_string(),
        protocols_seen: scan.protocols_seen,
        frame_count: scan.frame_count,
        frame_bytes_total: scan.frame_bytes_total,
        referenced_globals,
        builtin_types,
        content_hint: hint,
        scan_truncated: scan.scan_truncated,
        scan_error: scan.scan_error,
    })
}

/// Pickle files: metadata only, no template mining.
///
/// # Errors
///
/// Currently always returns [`Ok`].
pub fn extract_pickle_templates(
    _mmap: &Mmap,
    stats: &ParseResult,
    _config: &RuntimeConfig,
) -> Result<MiningResult> {
    Ok(empty_mining_result(stats))
}

fn header_protocol(bytes: &[u8]) -> Option<u8> {
    // Binary pickles start with `\x80` + protocol number (2–5 in practice).
    if bytes.len() >= 2 && bytes[0] == 0x80 && (2..=5).contains(&bytes[1]) {
        Some(bytes[1])
    } else {
        None
    }
}

fn pickle_encoding(bytes: &[u8]) -> &'static str {
    // Protocol 0/1 are printable ASCII; protocol 2+ uses a binary header.
    if header_protocol(bytes).is_some() || std::str::from_utf8(bytes).is_err() {
        "binary"
    } else {
        "text"
    }
}

/// Walk opcodes from `data`, collecting globals and frame stats until `STOP` or a cap.
fn scan_pickle_ops(data: &[u8]) -> PickleScan {
    let limits = ScanLimits { len: data.len() };
    let mut scan = PickleScan {
        protocols_seen: Vec::new(),
        frame_count: 0,
        frame_bytes_total: 0,
        referenced_globals: Vec::new(),
        saw_builtin_container: false,
        scan_truncated: false,
        scan_error: None,
    };
    let mut pos = 0_usize;
    let mut stack = PickleStack::new();
    let mut seen = HashSet::new();
    let mut op_count = 0_usize;

    while pos < limits.len && op_count < MAX_OPCODES {
        op_count += 1;
        let Some(op) = data.get(pos).copied() else {
            break;
        };
        pos += 1;

        if !dispatch_opcode(
            op, data, &mut pos, &limits, &mut stack, &mut scan, &mut seen,
        ) {
            break;
        }
    }

    if op_count >= MAX_OPCODES {
        scan.scan_truncated = true;
    }
    scan
}

/// Route one opcode to a category handler. Returns `false` on `STOP` or fatal parse error.
fn dispatch_opcode(
    op: u8,
    data: &[u8],
    pos: &mut usize,
    limits: &ScanLimits,
    stack: &mut PickleStack,
    scan: &mut PickleScan,
    seen: &mut HashSet<String>,
) -> bool {
    match op {
        b'.' => false,
        0x80 | 0x95 => dispatch_meta_opcode(op, data, pos, limits, scan),
        b'c' | b'i' | 0x93 => dispatch_global_opcode(op, data, pos, limits, stack, scan, seen),
        0x8c | b'X' | 0x8d => dispatch_unicode_opcode(op, data, pos, limits, stack, scan),
        b'V' | b'L' | b'I' | b'F' | b'S' | b'P' | b'g' | b'p' => {
            dispatch_line_operand(data, pos, limits, stack, scan)
        }
        0x96 | 0x8e | b'B' | b'T' | b'C' | b'U' | 0x8a | 0x8b => {
            dispatch_length_payload(op, data, pos, limits, stack, scan)
        }
        b'J' | b'j' | b'K' | 0x82 | 0x83 | 0x84 | b'M' | b'G' => {
            dispatch_fixed_operand(op, pos, limits, stack, scan)
        }
        b'r' | b'q' | b'h' | 0x94 => dispatch_memo_opcode(op, data, pos, limits, stack, scan),
        0x81 | b'R' | b'b' | 0x86 | 0x92 | 0x87 | b'(' | b'0' | b'2' | b'1' | b't' | b'l'
        | b'd' | b'u' | b'e' | b's' | b'a' | b']' | b')' | b'}' | b'N' | b'O' | 0x97 | 0x98
        | 0x8f | 0x90 | 0x91 | b'o' | b'D' | 0x51 | 0x88 | 0x89 | 0x85 => {
            dispatch_stack_opcode(op, stack, scan)
        }
        other => {
            scan.scan_truncated = true;
            scan.scan_error = Some(format!(
                "unsupported opcode 0x{other:02x} at offset {}",
                *pos - 1
            ));
            false
        }
    }
}

/// `PROTO` (0x80) and `FRAME` (0x95) — protocol version and frame size tallies.
fn dispatch_meta_opcode(
    op: u8,
    data: &[u8],
    pos: &mut usize,
    limits: &ScanLimits,
    scan: &mut PickleScan,
) -> bool {
    match op {
        0x80 => match read_u8(data, pos, limits) {
            Some(p) => {
                scan.protocols_seen.push(p);
                true
            }
            None => fail_scan(scan, "truncated PROTO operand"),
        },
        0x95 => match read_u64_le(data, pos, limits) {
            Some(n) => {
                scan.frame_count += 1;
                scan.frame_bytes_total += n;
                true
            }
            None => fail_scan(scan, "truncated FRAME operand"),
        },
        _ => true,
    }
}

/// `GLOBAL` / `INST` / `STACK_GLOBAL` — import references embedded in the pickle stream.
fn dispatch_global_opcode(
    op: u8,
    data: &[u8],
    pos: &mut usize,
    limits: &ScanLimits,
    stack: &mut PickleStack,
    scan: &mut PickleScan,
    seen: &mut HashSet<String>,
) -> bool {
    match op {
        b'c' | b'i' => match read_global_pair(data, pos, limits) {
            Some((module, name)) => {
                if is_valid_global_reference(&module, &name) {
                    push_global(&mut scan.referenced_globals, seen, &module, &name);
                }
                stack.push_opaque();
                true
            }
            None => fail_scan(scan, "truncated GLOBAL/INST operand"),
        },
        0x93 => {
            stack.stack_global(&mut scan.referenced_globals, seen);
            true
        }
        _ => true,
    }
}

/// `SHORT_BINUNICODE`, `BINUNICODE`, `BINUNICODE8` — push UTF-8 onto the stack.
fn dispatch_unicode_opcode(
    op: u8,
    data: &[u8],
    pos: &mut usize,
    limits: &ScanLimits,
    stack: &mut PickleStack,
    scan: &mut PickleScan,
) -> bool {
    let s = match op {
        0x8c => read_short_binunicode(data, pos, limits),
        b'X' => read_binunicode(data, pos, limits),
        0x8d => read_binunicode8(data, pos, limits),
        _ => None,
    };
    match s {
        Some(s) => {
            stack.push_string(s);
            true
        }
        None => fail_scan(
            scan,
            match op {
                0x8c => "truncated SHORT_BINUNICODE operand",
                b'X' => "truncated BINUNICODE operand",
                _ => "truncated BINUNICODE8 operand",
            },
        ),
    }
}

/// Protocol 0/1 operands terminated by newline (`INT`, `FLOAT`, `PUT`, …).
fn dispatch_line_operand(
    data: &[u8],
    pos: &mut usize,
    limits: &ScanLimits,
    stack: &mut PickleStack,
    scan: &mut PickleScan,
) -> bool {
    if read_line(data, pos, limits).is_none() {
        return fail_scan(scan, "truncated line operand");
    }
    stack.push_opaque();
    true
}

/// Length-prefixed blobs — skip payload bytes without reading (arrays, strings, …).
fn dispatch_length_payload(
    op: u8,
    data: &[u8],
    pos: &mut usize,
    limits: &ScanLimits,
    stack: &mut PickleStack,
    scan: &mut PickleScan,
) -> bool {
    let ok = match op {
        0x96 | 0x8e => skip_u64_payload(data, pos, limits, scan),
        b'B' | b'T' | 0x8b => skip_u32_payload(data, pos, limits, scan),
        b'C' | b'U' | 0x8a => skip_u8_payload(data, pos, limits, scan),
        _ => false,
    };
    if !ok {
        return fail_scan(
            scan,
            match op {
                0x96 | 0x8e => "truncated BYTEARRAY8/BINBYTES8 operand",
                b'B' | b'T' => "truncated BINBYTES/BINSTRING operand",
                b'C' | b'U' => "truncated short-length operand",
                0x8a => "truncated LONG1 operand",
                _ => "truncated LONG4 operand",
            },
        );
    }
    stack.push_opaque();
    true
}

/// Fixed-width numeric / extension operands (`BININT`, `BINFLOAT`, `EXT1`, …).
fn dispatch_fixed_operand(
    op: u8,
    pos: &mut usize,
    limits: &ScanLimits,
    stack: &mut PickleStack,
    scan: &mut PickleScan,
) -> bool {
    let (n, err) = match op {
        b'J' | b'j' => (4, "truncated 4-byte operand"),
        b'K' | 0x82 => (1, "truncated 1-byte operand"),
        0x83 => (2, "truncated EXT2 operand"),
        0x84 => (4, "truncated EXT4 operand"),
        b'M' => (2, "truncated BININT2 operand"),
        b'G' => (8, "truncated BINFLOAT operand"),
        _ => (0, "truncated operand"),
    };
    if !skip_bytes(pos, n, limits) {
        return fail_scan(scan, err);
    }
    stack.push_opaque();
    true
}

/// Memo table: `MEMOIZE`, `BINPUT`, `LONG_BINPUT`, `BINGET`.
fn dispatch_memo_opcode(
    op: u8,
    data: &[u8],
    pos: &mut usize,
    limits: &ScanLimits,
    stack: &mut PickleStack,
    scan: &mut PickleScan,
) -> bool {
    match op {
        b'r' => match read_u32_le(data, pos, limits) {
            Some(idx) => {
                stack.memo_put(idx as usize);
                true
            }
            None => fail_scan(scan, "truncated LONG_BINPUT operand"),
        },
        b'q' => match read_u8(data, pos, limits) {
            Some(idx) => {
                stack.memo_put(usize::from(idx));
                true
            }
            None => fail_scan(scan, "truncated BINPUT operand"),
        },
        b'h' => match read_u8(data, pos, limits) {
            Some(idx) => {
                stack.memo_get(usize::from(idx));
                true
            }
            None => fail_scan(scan, "truncated BINGET operand"),
        },
        0x94 => {
            stack.memoize_top();
            true
        }
        _ => true,
    }
}

/// Stack-shaping opcodes (`REDUCE`, `NEWOBJ`, `BUILD`, `TUPLE`, empty containers, …).
fn dispatch_stack_opcode(op: u8, stack: &mut PickleStack, scan: &mut PickleScan) -> bool {
    match op {
        0x81 | b'R' | b'b' | 0x86 => {
            stack.pop();
            stack.pop();
            stack.push_opaque();
        }
        0x92 | 0x87 => {
            stack.pop();
            stack.pop();
            stack.pop();
            stack.push_opaque();
        }
        b'(' => stack.push_mark(),
        b'0' => {
            stack.pop();
        }
        b'2' => stack.dup_top(),
        b'1' => stack.pop_mark(),
        b't' | b'l' | b'd' | b'u' | b'e' => {
            stack.pop_until_mark();
            stack.push_opaque();
        }
        b's' | b'a' => {
            stack.pop();
            stack.pop();
        }
        b']' | b')' | b'}' => {
            scan.saw_builtin_container = true;
            stack.push_opaque();
        }
        b'N' | b'O' | 0x97 | 0x98 | 0x8f | 0x90 | 0x91 | b'o' | b'D' | 0x51 | 0x88 | 0x89 => {
            stack.push_opaque();
        }
        0x85 => {
            stack.pop();
            stack.push_opaque();
        }
        _ => {}
    }
    true
}

fn fail_scan(scan: &mut PickleScan, message: &str) -> bool {
    scan.scan_truncated = true;
    scan.scan_error = Some(message.to_string());
    false
}

fn skip_u8_payload(
    data: &[u8],
    pos: &mut usize,
    limits: &ScanLimits,
    scan: &mut PickleScan,
) -> bool {
    let Some(len) = read_u8(data, pos, limits).map(usize::from) else {
        return false;
    };
    skip_payload(data, pos, len, limits, scan)
}

fn skip_u32_payload(
    data: &[u8],
    pos: &mut usize,
    limits: &ScanLimits,
    scan: &mut PickleScan,
) -> bool {
    if *pos + 4 > limits.len {
        return false;
    }
    let len = u32::from_le_bytes(data[*pos..*pos + 4].try_into().expect("4 bytes")) as usize;
    *pos += 4;
    skip_payload(data, pos, len, limits, scan)
}

fn skip_u64_payload(
    data: &[u8],
    pos: &mut usize,
    limits: &ScanLimits,
    scan: &mut PickleScan,
) -> bool {
    let Some(len) = read_u64_le(data, pos, limits).map(|n| n as usize) else {
        return false;
    };
    skip_payload(data, pos, len, limits, scan)
}

/// Advance past `len` payload bytes without reading them.
fn skip_payload(
    _data: &[u8],
    pos: &mut usize,
    len: usize,
    limits: &ScanLimits,
    scan: &mut PickleScan,
) -> bool {
    if *pos + len > limits.len {
        scan.scan_truncated = true;
        return false;
    }
    *pos += len;
    true
}

fn push_global(out: &mut Vec<String>, seen: &mut HashSet<String>, module: &str, name: &str) {
    if out.len() >= MAX_GLOBALS {
        return;
    }
    let reference = format!("{module}.{name}");
    if seen.insert(reference.clone()) {
        out.push(reference);
    }
}

fn is_builtin_type(reference: &str) -> bool {
    reference.starts_with("builtins.")
        || reference.starts_with("__builtin__.")
        || reference.starts_with("collections.")
}

/// Reject dtype fragments and other non-import pairs (e.g. `"|"` + `"Index"`).
fn is_valid_global_reference(module: &str, name: &str) -> bool {
    fn is_ident(s: &str) -> bool {
        !s.is_empty()
            && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            && (s.as_bytes()[0].is_ascii_alphabetic() || s.as_bytes()[0] == b'_')
    }
    fn is_module(s: &str) -> bool {
        !s.is_empty() && s.split('.').all(is_ident)
    }
    is_module(module) && is_ident(name)
}

/// Heuristic payload label from globals: `tabular`, `ml_model`, `numeric_array`, `builtin_containers`.
fn content_hint(
    referenced_globals: &[String],
    builtin_types: &[String],
    saw_builtin_container: bool,
) -> Option<String> {
    let all: Vec<&str> = referenced_globals
        .iter()
        .chain(builtin_types.iter())
        .map(String::as_str)
        .collect();
    if all.iter().any(|r| r.contains("pandas")) {
        return Some("tabular".to_string());
    }
    if all
        .iter()
        .any(|r| r.contains("sklearn") || r.contains("torch") || r.contains("xgboost"))
    {
        return Some("ml_model".to_string());
    }
    if all.iter().any(|r| {
        r.contains("numpy._core") || r.contains("numpy.ndarray") || r.contains("numpy.dtype")
    }) {
        return Some("numeric_array".to_string());
    }
    if !all.is_empty() && all.iter().all(|r| r.starts_with("builtins.")) {
        return Some("builtin_containers".to_string());
    }
    if referenced_globals.is_empty() && builtin_types.is_empty() && saw_builtin_container {
        return Some("builtin_containers".to_string());
    }
    None
}

fn read_u32_le(data: &[u8], pos: &mut usize, limits: &ScanLimits) -> Option<u32> {
    if *pos + 4 > limits.len {
        return None;
    }
    let v = u32::from_le_bytes(data[*pos..*pos + 4].try_into().ok()?);
    *pos += 4;
    Some(v)
}

fn read_u8(data: &[u8], pos: &mut usize, limits: &ScanLimits) -> Option<u8> {
    if *pos >= limits.len {
        return None;
    }
    let v = *data.get(*pos)?;
    *pos += 1;
    Some(v)
}

fn read_u64_le(data: &[u8], pos: &mut usize, limits: &ScanLimits) -> Option<u64> {
    if *pos + 8 > limits.len {
        return None;
    }
    let mut buf = [0_u8; 8];
    buf.copy_from_slice(&data[*pos..*pos + 8]);
    *pos += 8;
    Some(u64::from_le_bytes(buf))
}

fn skip_bytes(pos: &mut usize, n: usize, limits: &ScanLimits) -> bool {
    if *pos + n > limits.len {
        return false;
    }
    *pos += n;
    true
}

fn read_line(data: &[u8], pos: &mut usize, limits: &ScanLimits) -> Option<String> {
    let start = *pos;
    while *pos < limits.len && data[*pos] != b'\n' {
        *pos += 1;
    }
    if *pos >= limits.len {
        return None;
    }
    let line = std::str::from_utf8(&data[start..*pos]).ok()?.to_string();
    *pos += 1;
    Some(line)
}

fn read_global_pair(data: &[u8], pos: &mut usize, limits: &ScanLimits) -> Option<(String, String)> {
    let module = read_line(data, pos, limits)?;
    let name = read_line(data, pos, limits)?;
    Some((module, name))
}

fn read_short_binunicode(data: &[u8], pos: &mut usize, limits: &ScanLimits) -> Option<String> {
    let len = usize::from(read_u8(data, pos, limits)?);
    if *pos + len > limits.len {
        return None;
    }
    let s = std::str::from_utf8(&data[*pos..*pos + len])
        .ok()?
        .to_string();
    *pos += len;
    Some(s)
}

fn read_binunicode(data: &[u8], pos: &mut usize, limits: &ScanLimits) -> Option<String> {
    if *pos + 4 > limits.len {
        return None;
    }
    let len = u32::from_le_bytes(data[*pos..*pos + 4].try_into().ok()?) as usize;
    *pos += 4;
    if *pos + len > limits.len {
        return None;
    }
    let s = std::str::from_utf8(&data[*pos..*pos + len])
        .ok()?
        .to_string();
    *pos += len;
    Some(s)
}

fn read_binunicode8(data: &[u8], pos: &mut usize, limits: &ScanLimits) -> Option<String> {
    let len = read_u64_le(data, pos, limits)? as usize;
    if *pos + len > limits.len {
        return None;
    }
    let s = std::str::from_utf8(&data[*pos..*pos + len])
        .ok()?
        .to_string();
    *pos += len;
    Some(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_finds_stack_global_reference() {
        // PROTO 4 + FRAME + SHORT_BINUNICODE __main__ + SHORT_BINUNICODE Dummy + STACK_GLOBAL + STOP
        let data = [
            0x80, 0x04, 0x95, 0x13, 0, 0, 0, 0, 0, 0, 0, 0x8c, 0x08, b'_', b'_', b'm', b'a', b'i',
            b'n', b'_', b'_', 0x8c, 0x05, b'D', b'u', b'm', b'm', b'y', 0x93, 0x2e,
        ];
        let scan = scan_pickle_ops(&data);
        assert!(
            scan.referenced_globals
                .contains(&"__main__.Dummy".to_string())
        );
        assert_eq!(scan.protocols_seen, vec![4]);
        assert_eq!(scan.frame_count, 1);
    }

    #[test]
    fn scan_stack_global_after_memoize() {
        // Real pickles memoize module/name without popping before STACK_GLOBAL.
        let data = [
            0x80, 0x05, 0x95, 0x17, 0, 0, 0, 0, 0, 0, 0, 0x8c, 0x06, b'p', b'a', b'n', b'd', b'a',
            b's', 0x94, 0x8c, 0x09, b'D', b'a', b't', b'a', b'F', b'r', b'a', b'm', b'e', 0x94,
            0x93, 0x2e,
        ];
        let scan = scan_pickle_ops(&data);
        assert!(
            scan.referenced_globals
                .contains(&"pandas.DataFrame".to_string())
        );
    }
}
