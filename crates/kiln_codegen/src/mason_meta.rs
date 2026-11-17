//! Link-time metadata for the mortar linker: the `.mason.meta` container
//! (`-fmason-metadata[=level]`, spec/codegen.md "Link-time metadata
//! (`.mason.meta`)", WP-1557).
//!
//! The bytes are one **unit** as mortar's `spec/meta.md` §4 frames it: a
//! 32-byte header, blocks sorted by record kind, then the unit's string
//! table, all little-endian and padded with zeros to a multiple of 8.
//! [`encode_unit`] is a pure function from blocks to bytes, so a later
//! record kind adds a [`Block`] without touching the framing. Only kind 1
//! (the TU summary, MT-26) is written today ([`Cg::emit_mason_meta`]).

use std::collections::BTreeMap;

use kiln_abi::mangle;
use kiln_ast::decl::{DeclFlags, DeclKind};
use kiln_ast::sanitize::SanitizerSet;
use kiln_ast::types::{BuiltinType, TypeView};
use kiln_ast::{DeclId, QualType};
use kiln_llvm::{Linkage, TypeId as LTy, Value};
use kiln_source::fx::FxHashMap;

use crate::mason_ctype;
use crate::module::{Cg, gval};

/// The unit's magic (MT-10).
const MAGIC: [u8; 4] = *b"MASN";
/// The container's major and minor version (MT-10, MT-36: the lowest
/// minor with every feature used).
const MAJOR: u16 = 1;
const MINOR: u16 = 0;
/// The header's size in version 1.0 (MT-10).
const HEADER_SIZE: u32 = 32;
/// The producer code of kiln (MT-10).
pub const PRODUCER_KILN: u16 = 1;
/// Record kind 1, the TU summary (MT-26), and its row layout version.
pub const KIND_TU_SUMMARY: u16 = 1;
const TU_SUMMARY_VERSION: u16 = 1;
/// A kind 1 row: `{u32 name; u32 value; u32 flags; u32 reserved}`.
const TU_SUMMARY_ROW_SIZE: u32 = 16;
/// Kind 1 flag bit 0 (MT-26).
pub const FLAG_ENFORCE: u32 = 1;
/// Record kind 5, the declarations (mortar `spec/odr.md` O-35), and its
/// row layout version.
pub const KIND_DECLARATIONS: u16 = 5;
const DECLARATIONS_VERSION: u16 = 1;
/// A kind 5 row: `{u64 key; u64 type; u64 size; u32 name; u32 type_name;
/// u16 flags; u16 reserved; u32 reserved}`.
const DECLARATION_ROW_SIZE: u32 = 40;
/// Kind 5 flag bits (O-35).
pub const DECL_DEFINITION: u16 = 1;
/// The entity is a variable.
pub const DECL_VARIABLE: u16 = 2;
/// The entity has C language linkage (every row of a C unit).
pub const DECL_EXTERN_C: u16 = 4;
/// The function's type is `noexcept` (C++ linkage only).
pub const DECL_NOEXCEPT: u16 = 8;

/// Record kind 3, the type definitions (mortar `spec/odr.md` O-17), and
/// its row layout version.
pub const KIND_TYPES: u16 = 3;
const TYPES_VERSION: u16 = 1;
/// A kind 3 row: `{u64 key; u8 hash[16]; u64 layout; u32 name; u32 size;
/// u16 align; u16 flags; u32 file; u32 line; u32 details}`.
const TYPE_ROW_SIZE: u32 = 56;
/// A kind 3 row's `details` without a kind 4 entry.
const NO_DETAILS: u32 = 0xffff_ffff;
/// Kind 3 flag bits (O-17; bit 0 `ENUM` and bit 4 `PARTIAL` are never
/// set: no enumeration gets a row, and a layout kiln cannot serialize
/// whole gets no row): a union.
pub const TYPE_UNION: u16 = 2;
/// Polymorphic ([class.virtual]/1).
pub const TYPE_POLYMORPHIC: u16 = 4;
/// Has a virtual base, direct or indirect.
pub const TYPE_HAS_VBASES: u16 = 8;
/// Defined in a system header.
pub const TYPE_SYSTEM: u16 = 32;
/// Kind 1's `odr.scheme` (O-11) of the rows kiln hashes: the layout
/// serialization of `kiln_abi::odr::LayoutSerializer` (version 1) under
/// FNV-1a 64. Any change to what is serialized or how changes the name
/// (O-11's bump rule; `layout_hash_golden` guards it).
pub const ODR_SCHEME: &str = "kiln-odr-1";

/// How much `-fmason-metadata=<level>` asks for (mortar `spec/odr.md`
/// K-1: `min` kinds 1 and 2; `default` 1, 2, 3, 5; `detail` + 4; `all` +
/// rows for definitions inlined away). Kiln writes kind 1 at every level
/// and kinds 3 and 5 from `default` up (WP-1564, WP-1566); kinds 2 and 4
/// are later work.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MasonMetaLevel {
    /// `min`.
    Min,
    /// `default` (`-fmason-metadata` without a level).
    #[default]
    Default,
    /// `detail`.
    Detail,
    /// `all`.
    All,
}

impl MasonMetaLevel {
    /// The level writes record kind 5 (K-1: `default` and up).
    #[must_use]
    pub fn writes_declarations(self) -> bool {
        self != MasonMetaLevel::Min
    }

    /// The level writes record kind 3 (K-1: `default` and up; C++ units
    /// only, O-33).
    #[must_use]
    pub fn writes_types(self) -> bool {
        self != MasonMetaLevel::Min
    }

    /// The level writes the type string of a C++-linkage function's kind 5
    /// row (mortar's ruling of 11-16: `detail` and up; at `default` its
    /// `type_name` is the empty string, its `type` hash is written all the
    /// same). Variables and C-linkage rows keep theirs at every level.
    #[must_use]
    pub fn writes_cxx_function_type_names(self) -> bool {
        matches!(self, MasonMetaLevel::Detail | MasonMetaLevel::All)
    }

    /// The level named `name` on the command line.
    #[must_use]
    pub fn from_name(name: &str) -> Option<MasonMetaLevel> {
        Some(match name {
            "min" => MasonMetaLevel::Min,
            "default" => MasonMetaLevel::Default,
            "detail" => MasonMetaLevel::Detail,
            "all" => MasonMetaLevel::All,
            _ => return None,
        })
    }
}

/// What the driver decided for the TU summary that codegen cannot see
/// itself (spec/codegen.md "Link-time metadata (`.mason.meta`)" says
/// where each value comes from).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MasonMetaOptions {
    /// The level: kind 5 from `default` up.
    pub level: MasonMetaLevel,
    /// `producer`: `kiln <version>`.
    pub producer: String,
    /// `std`: the language standard's name (`c++26`, `c17`).
    pub std: String,
    /// `abi.short-enums`: `-fshort-enums`.
    pub short_enums: bool,
    /// The C++ standard library rows; `None` for a C unit.
    pub stdlib: Option<StdlibSummary>,
}

/// The `abi.stdlib*` rows of a C++ unit.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StdlibSummary {
    /// `abi.stdlib`: `libc++` or `libstdc++`.
    pub name: &'static str,
    /// `abi.stdlib-abi`: the library's ABI version at the end of the TU,
    /// for libc++ followed by `+<macro>` for each layout-changing ABI
    /// switch defined (WP-1564); `None` (no row) when the TU defines none
    /// (no library header).
    pub abi: Option<String>,
    /// `abi.stdlib-debug`: the layout-changing debug mode (`_GLIBCXX_DEBUG`
    /// or `off`; always `ENFORCE`, WP-1564); `None` (no row) when the TU
    /// includes no library header.
    pub debug: Option<String>,
    /// `abi.stdlib-hardening`: the checking mode that changes no layout
    /// (`_GLIBCXX_ASSERTIONS`, libc++'s `none|fast|extensive|debug`;
    /// information, WP-1564); `None` (no row) when there is none.
    pub hardening: Option<String>,
}

/// One kind 1 row before encoding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SummaryRow {
    /// The key (MT-26's names).
    pub name: &'static str,
    /// The value.
    pub value: String,
    /// The row carries `ENFORCE`.
    pub enforce: bool,
}

/// A unit's string table under construction (MT-12): offset 0 is the
/// empty string; strings are laid out in the order they are first added
/// (MT-38: the sorted rows' order) and added once.
#[derive(Debug)]
pub struct StrTab {
    bytes: Vec<u8>,
    index: FxHashMap<Vec<u8>, u32>,
}

impl Default for StrTab {
    fn default() -> Self {
        StrTab { bytes: vec![0], index: FxHashMap::default() }
    }
}

impl StrTab {
    /// The offset of `s`, added if new. A string is cut at an interior NUL
    /// (none of kiln's can hold one: they are paths, names and macro
    /// spellings).
    pub fn add(&mut self, s: &[u8]) -> u32 {
        let s = s.split(|&b| b == 0).next().unwrap_or_default();
        if s.is_empty() {
            return 0;
        }
        if let Some(&o) = self.index.get(s) {
            return o;
        }
        let o = u32::try_from(self.bytes.len()).unwrap_or(u32::MAX);
        self.bytes.extend_from_slice(s);
        self.bytes.push(0);
        self.index.insert(s.to_vec(), o);
        o
    }

    /// Only the empty string.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bytes.len() == 1
    }
}

/// One block (MT-11): `row_count` rows of `row_size` bytes, then any
/// kind-specific bytes, in `body`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    /// The record kind (§7).
    pub kind: u16,
    /// The kind's row layout version.
    pub version: u16,
    /// A row's size, a multiple of 8.
    pub row_size: u32,
    /// The number of rows.
    pub row_count: u32,
    /// The rows (and any kind-specific bytes after them).
    pub body: Vec<u8>,
}

/// The kind 1 block of `rows` (MT-26): sorted by name bytes, one row per
/// name (a later duplicate is dropped), strings added to `st` in row order.
#[must_use]
pub fn tu_summary_block(rows: &[SummaryRow], st: &mut StrTab) -> Block {
    let mut rows: Vec<&SummaryRow> = rows.iter().collect();
    rows.sort_by(|a, b| a.name.as_bytes().cmp(b.name.as_bytes()));
    rows.dedup_by(|b, a| a.name == b.name);
    let mut body = Vec::with_capacity(rows.len() * TU_SUMMARY_ROW_SIZE as usize);
    for r in &rows {
        let name = st.add(r.name.as_bytes());
        let value = st.add(r.value.as_bytes());
        body.extend_from_slice(&name.to_le_bytes());
        body.extend_from_slice(&value.to_le_bytes());
        body.extend_from_slice(&(if r.enforce { FLAG_ENFORCE } else { 0 }).to_le_bytes());
        body.extend_from_slice(&0u32.to_le_bytes());
    }
    Block {
        kind: KIND_TU_SUMMARY,
        version: TU_SUMMARY_VERSION,
        row_size: TU_SUMMARY_ROW_SIZE,
        row_count: u32::try_from(rows.len()).unwrap_or(u32::MAX),
        body,
    }
}

/// FNV-1a 64 of `bytes` (MT-13: offset basis `0xcbf29ce484222325`, prime
/// `0x100000001b3`): a kind 5 row's `key` (of the symbol name) and `type`
/// (of the type string), a kind 3 row's `key` (of the `<type>` encoding)
/// and `layout` (of the layout serialization).
pub use kiln_abi::odr::fnv1a64;

/// One kind 3 row before encoding (mortar `spec/odr.md` O-17).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct TypeRow {
    /// FNV-1a 64 of `name`.
    pub key: u64,
    /// The `<type>` encoding of the class (what `_ZTS` names).
    pub name: String,
    /// FNV-1a 64 of the layout serialization (O-18).
    pub layout: u64,
    /// `sizeof`, bytes.
    pub size: u32,
    /// `alignof`, bytes.
    pub align: u16,
    /// `TYPE_*` bits.
    pub flags: u16,
    /// The definition's file, after the prefix maps (empty: unknown).
    pub file: String,
    /// The definition's line (0: unknown).
    pub line: u32,
}

/// The kind 3 block of `rows` (O-17, MT-11): sorted by `key`, then name
/// bytes, one row per name (exact duplicates dropped; a name with two
/// different rows gets none), strings added to `st` in row order (the
/// name, then the file). `hash` is all zero ("not hashed": kiln has no
/// definition serializer yet, K-3) and `details` is `0xffffffff` (no
/// kind 4).
#[must_use]
pub fn types_block(rows: &[TypeRow], st: &mut StrTab) -> Block {
    let mut rows: Vec<&TypeRow> = rows.iter().collect();
    rows.sort_by(|a, b| (a.key, a.name.as_bytes()).cmp(&(b.key, b.name.as_bytes())).then(a.cmp(b)));
    rows.dedup();
    let mut kept: Vec<&TypeRow> = Vec::with_capacity(rows.len());
    let mut i = 0;
    while i < rows.len() {
        let mut j = i + 1;
        while j < rows.len() && rows[j].name == rows[i].name {
            j += 1;
        }
        if j == i + 1 {
            kept.push(rows[i]);
        }
        i = j;
    }
    let mut body = Vec::with_capacity(kept.len() * TYPE_ROW_SIZE as usize);
    for r in &kept {
        let name = st.add(r.name.as_bytes());
        let file = st.add(r.file.as_bytes());
        body.extend_from_slice(&r.key.to_le_bytes());
        body.extend_from_slice(&[0u8; 16]); // hash: not hashed
        body.extend_from_slice(&r.layout.to_le_bytes());
        body.extend_from_slice(&name.to_le_bytes());
        body.extend_from_slice(&r.size.to_le_bytes());
        body.extend_from_slice(&r.align.to_le_bytes());
        body.extend_from_slice(&r.flags.to_le_bytes());
        body.extend_from_slice(&file.to_le_bytes());
        body.extend_from_slice(&r.line.to_le_bytes());
        body.extend_from_slice(&NO_DETAILS.to_le_bytes());
    }
    Block {
        kind: KIND_TYPES,
        version: TYPES_VERSION,
        row_size: TYPE_ROW_SIZE,
        row_count: u32_len(kept.len()),
        body,
    }
}

/// One kind 5 row before encoding (O-35).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DeclRow {
    /// FNV-1a 64 of the symbol's name (anchored: the name is not written).
    pub key: u64,
    /// `DECL_*` bits.
    pub flags: u16,
    /// FNV-1a 64 of `type_name`.
    pub type_hash: u64,
    /// `sizeof` of a variable's declared type; 0 for a function or an
    /// unknown size.
    pub size: u64,
    /// The hashed type string (Itanium `<type>` for C++ linkage, the
    /// canonical C type for C linkage), for the message; empty (string
    /// offset 0) for a C++-linkage function below `detail`.
    pub type_name: String,
}

/// The kind 5 block of `rows` (O-35, MT-11): sorted by `key`, then
/// `flags` (exact duplicates dropped), type strings added to `st` in row
/// order.
#[must_use]
pub fn declarations_block(rows: &[DeclRow], st: &mut StrTab) -> Block {
    let mut rows: Vec<&DeclRow> = rows.iter().collect();
    rows.sort();
    rows.dedup();
    let mut body = Vec::with_capacity(rows.len() * DECLARATION_ROW_SIZE as usize);
    for r in &rows {
        let type_name = st.add(r.type_name.as_bytes());
        body.extend_from_slice(&r.key.to_le_bytes());
        body.extend_from_slice(&r.type_hash.to_le_bytes());
        body.extend_from_slice(&r.size.to_le_bytes());
        body.extend_from_slice(&0u32.to_le_bytes()); // name: anchored
        body.extend_from_slice(&type_name.to_le_bytes());
        body.extend_from_slice(&r.flags.to_le_bytes());
        body.extend_from_slice(&0u16.to_le_bytes());
        body.extend_from_slice(&0u32.to_le_bytes());
    }
    Block {
        kind: KIND_DECLARATIONS,
        version: DECLARATIONS_VERSION,
        row_size: DECLARATION_ROW_SIZE,
        row_count: u32_len(rows.len()),
        body,
    }
}

fn pad8(out: &mut Vec<u8>) {
    while !out.len().is_multiple_of(8) {
        out.push(0);
    }
}

fn u32_len(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// The bytes of one unit (MT-10): the header, `blocks` sorted by kind (a
/// kind given twice keeps its first block: MT-11 allows one), each
/// padded to 8, then the string table `st` (none when it holds only the
/// empty string), the whole padded with zeros to a multiple of 8.
#[must_use]
pub fn encode_unit(producer: u16, blocks: &[Block], st: &StrTab) -> Vec<u8> {
    let mut blocks: Vec<&Block> = blocks.iter().collect();
    blocks.sort_by_key(|b| b.kind);
    blocks.dedup_by_key(|b| b.kind);
    let mut out = vec![0u8; HEADER_SIZE as usize];
    for b in &blocks {
        let start = out.len();
        out.extend_from_slice(&b.kind.to_le_bytes());
        out.extend_from_slice(&b.version.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes()); // block_size, below
        out.extend_from_slice(&b.row_size.to_le_bytes());
        out.extend_from_slice(&b.row_count.to_le_bytes());
        out.extend_from_slice(&b.body);
        pad8(&mut out);
        let size = u32_len(out.len() - start).to_le_bytes();
        out[start + 4..start + 8].copy_from_slice(&size);
    }
    let (strtab_offset, strtab_size) = if st.is_empty() {
        (0, 0)
    } else {
        let at = u32_len(out.len());
        out.extend_from_slice(&st.bytes);
        (at, u32_len(st.bytes.len()))
    };
    pad8(&mut out);
    let unit_size = u32_len(out.len());
    let mut h = Vec::with_capacity(HEADER_SIZE as usize);
    h.extend_from_slice(&MAGIC);
    h.extend_from_slice(&MAJOR.to_le_bytes());
    h.extend_from_slice(&MINOR.to_le_bytes());
    h.extend_from_slice(&unit_size.to_le_bytes());
    h.extend_from_slice(&HEADER_SIZE.to_le_bytes());
    h.extend_from_slice(&u32_len(blocks.len()).to_le_bytes());
    h.extend_from_slice(&strtab_offset.to_le_bytes());
    h.extend_from_slice(&strtab_size.to_le_bytes());
    h.extend_from_slice(&producer.to_le_bytes());
    h.extend_from_slice(&0u16.to_le_bytes()); // flags
    out[..HEADER_SIZE as usize].copy_from_slice(&h);
    out
}

/// Whether a symbol of IR linkage `l` is one a kind 5 row describes, and
/// if so whether the object defines it (`has_body`: a function body or a
/// variable initializer): `None` for a symbol with no external name
/// (`internal`, `private`, `appending`). An `available_externally`
/// definition is a reference: the object never defines the symbol.
fn symbol_defined(l: Linkage, has_body: bool) -> Option<bool> {
    match l {
        Linkage::External | Linkage::ExternWeak => Some(has_body),
        Linkage::LinkOnce
        | Linkage::LinkOnceOdr
        | Linkage::Weak
        | Linkage::WeakOdr
        | Linkage::Common => Some(true),
        Linkage::AvailableExternally => Some(false),
        Linkage::Internal | Linkage::Private | Linkage::Appending => None,
    }
}

/// "on" or "off".
fn on_off(b: bool) -> String {
    (if b { "on" } else { "off" }).to_owned()
}

impl Cg<'_> {
    /// The kind 1 rows of this unit (MT-26; spec/codegen.md "Link-time
    /// metadata (`.mason.meta`)" has the source of each value). Never
    /// `odr.scheme` (no hashed rows yet), `abi.struct-return` (i386 only)
    /// or `reloc-model` (hod's).
    pub(crate) fn mason_summary_rows(&self, m: &MasonMetaOptions) -> Vec<SummaryRow> {
        let row = |name, value: String, enforce| SummaryRow { name, value, enforce };
        let types = &self.target.types;
        let source = if self.opts.source_filename.is_empty() {
            String::new()
        } else {
            self.opts.debug.remap(&self.opts.source_filename)
        };
        let mut rows = vec![
            row("producer", m.producer.clone(), false),
            row("source", source, false),
            row("std", m.std.clone(), false),
            row("lang", (if self.opts.c.is_some() { "c" } else { "c++" }).to_owned(), false),
            row("abi.rtti", on_off(self.opts.rtti), false),
            row("abi.exceptions", on_off(self.opts.exceptions), false),
            row("abi.short-enums", on_off(m.short_enums), true),
            row(
                "abi.char",
                (if types.char_signed { "signed" } else { "unsigned" }).to_owned(),
                true,
            ),
            row("abi.wchar", types.builtin_size_align(BuiltinType::WChar).0.to_string(), true),
            row(
                "abi.long-double",
                (u32::from(types.builtin_size_align(BuiltinType::LongDouble).0) * 8).to_string(),
                true,
            ),
            row("isa", self.features_attr(None), false),
        ];
        if let Some(s) = &m.stdlib {
            rows.push(row("abi.stdlib", s.name.to_owned(), true));
            if let Some(abi) = &s.abi {
                rows.push(row("abi.stdlib-abi", abi.clone(), true));
            }
            if let Some(mode) = &s.debug {
                rows.push(row("abi.stdlib-debug", mode.clone(), true));
            }
            if let Some(mode) = &s.hardening {
                rows.push(row("abi.stdlib-hardening", mode.clone(), false));
            }
        }
        rows
    }

    /// The kind 5 rows of this unit (O-35; spec/codegen.md "Link-time
    /// metadata (`.mason.meta`)", "Kind 5"): one per external symbol that
    /// a function or variable declared in the source defines or references
    /// in the module as emitted, read from the declaration maps
    /// ([`Cg::fns`], [`Cg::gvars`]) at the end of module emission: nothing
    /// is recorded while emitting. A unit holds at most one row per key: a
    /// symbol whose declarations give different rows (or one of which has
    /// no row) gets none.
    pub(crate) fn mason_decl_rows(&mut self) -> Vec<DeclRow> {
        let ast = self.ast;
        let mut cands: Vec<(String, DeclId, bool, bool)> = Vec::new();
        // Compiler-generated: no declaration in the source (a library
        // builtin the source redeclares, C's `abort` in `<stdlib.h>`, is
        // declared in the source).
        let decl_ok = |d: DeclId| {
            ast.redecls_of(d).any(|x| !ast.decls.get(x).has(DeclFlags::IMPLICIT))
                && ast.weakref_target(d).is_none()
        };
        for (&first, &f) in &self.fns {
            let k = ast.decls.get(first).kind();
            if !matches!(k, DeclKind::Function | DeclKind::CXXMethod | DeclKind::CXXConversion)
                || !decl_ok(first)
                || self.m.resolve(Value::func(f)) != Value::func(f)
            {
                continue;
            }
            let func = self.m.func(f);
            if let Some(def) = symbol_defined(func.linkage, !func.is_declaration()) {
                cands.push((func.name().to_owned(), first, def, false));
            }
        }
        for (&first, &g) in &self.gvars {
            let k = ast.decls.get(first).kind();
            if !matches!(k, DeclKind::Var | DeclKind::VarTemplateSpecialization)
                || !decl_ok(first)
                || self.m.resolve(Value::global(g)) != Value::global(g)
            {
                continue;
            }
            let gv = self.m.global(g);
            if let Some(def) = symbol_defined(gv.linkage, gv.initializer().is_some()) {
                cands.push((gv.name().to_owned(), first, def, true));
            }
        }
        // Symbol order, so that anything the mangler numbers is numbered
        // the same way in every run (MT-38).
        cands.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        let mut types: FxHashMap<(QualType, bool), Option<String>> = FxHashMap::default();
        let mut by_symbol: BTreeMap<String, Option<DeclRow>> = BTreeMap::new();
        for (name, first, def, var) in cands {
            let name = name.strip_prefix('\u{1}').unwrap_or(&name).to_owned();
            if name.is_empty() || name.starts_with("llvm.") || name.starts_with("_ZDC") {
                continue;
            }
            let row = self.mason_decl_row(&name, first, def, var, &mut types);
            match by_symbol.get_mut(&name) {
                None => {
                    by_symbol.insert(name, row);
                }
                Some(slot) => {
                    // One symbol from several declarations (an asm label
                    // shared, an `extern "C"` function declared in two
                    // namespaces, block-scope `extern` declarations): one
                    // row if type hash, size and flags agree up to
                    // `DEFINITION` (which is or-ed), else none; `NOEXCEPT`
                    // must agree (mortar's ruling 1(a) of 11-16).
                    *slot = match (slot.take(), row) {
                        (Some(a), Some(b))
                            if (a.flags | DECL_DEFINITION) == (b.flags | DECL_DEFINITION)
                                && (a.type_hash, a.size) == (b.type_hash, b.size) =>
                        {
                            let flags = a.flags | (b.flags & DECL_DEFINITION);
                            Some(DeclRow { flags, ..a })
                        }
                        _ => None,
                    };
                }
            }
        }
        by_symbol.into_values().flatten().collect()
    }

    /// The row of symbol `name` for the function or variable whose first
    /// declaration is `first`, or `None` when its type has no encoding
    /// (an unprototyped C function, a C-linkage type with no canonical C
    /// form) or its name is not a function of its declaration (a
    /// C++-linkage entity named by an asm label: glibc's `extern "C++"`
    /// overloads of `strchr` share one; mortar's ruling 1(a)).
    fn mason_decl_row(
        &mut self,
        name: &str,
        first: DeclId,
        def: bool,
        var: bool,
        types: &mut FxHashMap<(QualType, bool), Option<String>>,
    ) -> Option<DeclRow> {
        let ast = self.ast;
        let extern_c = ast.lang_c || mangle::is_extern_c(ast, first);
        if !extern_c && mangle::asm_label(ast, first).is_some() {
            return None;
        }
        let decl = if var || !extern_c {
            ast.definition(first).or_else(|| {
                ast.redecls_of(first).find(|&x| !ast.decls.get(x).has(DeclFlags::IMPLICIT))
            })?
        } else {
            // The composite type of a C function has a prototype when
            // any declaration has one (C17 6.2.7/3); the latest written
            // one carries it (sema merges it into later declarations).
            let prototyped = |d: &DeclId| {
                let n = ast.decls.get(*d);
                !n.has(DeclFlags::IMPLICIT)
                    && n.ty().and_then(|t| ast.types.proto(t)).is_some_and(|p| !p.info().no_proto())
            };
            ast.definition(first)
                .filter(prototyped)
                .or_else(|| ast.redecls_of(first).find(prototyped))?
        };
        let ty = ast.decls.get(decl).ty()?;
        let type_name = if let Some(t) = types.get(&(ty, extern_c)) {
            t.clone()
        } else {
            let t = if !extern_c {
                Some(mangle::mangle_declared_type(ast, &mut self.anon, ty))
            } else if var {
                mason_ctype::object(ast, self.target, ty)
            } else {
                mason_ctype::function(ast, self.target, ty)
            };
            types.insert((ty, extern_c), t.clone());
            t
        }?;
        let mut flags = 0;
        if def {
            flags |= DECL_DEFINITION;
        }
        if var {
            flags |= DECL_VARIABLE;
        }
        if extern_c {
            flags |= DECL_EXTERN_C;
        } else if !var && ast.types.proto(ty).is_some_and(|p| ast.types.exc_spec(p).is_nothrow()) {
            flags |= DECL_NOEXCEPT;
        }
        let size = if var { self.mason_object_size(ty) } else { 0 };
        Some(DeclRow {
            key: fnv1a64(name.as_bytes()),
            flags,
            type_hash: fnv1a64(type_name.as_bytes()),
            size,
            type_name,
        })
    }

    /// The kind 3 rows of this C++ unit (mortar `spec/odr.md` O-17, O-18;
    /// spec/codegen.md "Link-time metadata (`.mason.meta`)", "Kind 3"):
    /// one per class, struct or union whose layout the TU computed (a
    /// layout exists in sema's cache, codegen's or the vtable builder's at
    /// the end of module emission; none is computed for the metadata), that
    /// is complete, valid, not templated and passes the syntactic
    /// not-TU-local test, and whose layout serializes without a per-TU
    /// component. No enumeration gets a row (kiln does not record which
    /// enumerations an emitted definition uses).
    pub(crate) fn mason_type_rows(&mut self) -> Vec<TypeRow> {
        use kiln_abi::odr::{LayoutSerializer, NotTuLocal, is_templated};
        use kiln_ast::decl::DefinitionBits;
        let ast = self.ast;
        // The classes laid out so far, in declaration order (MT-38).
        let mut ids: Vec<DeclId> = ast
            .layouts
            .iter()
            .map(|(d, _)| d)
            .chain(self.tys.layouts().map(|(d, _)| d))
            .chain(self.vts.layouts().map(|(d, _)| d))
            .map(|d| ast.first(d))
            .collect();
        ids.sort_unstable();
        ids.dedup();
        let mut test = NotTuLocal::new(ast);
        let cands: Vec<(DeclId, DeclId)> = ids
            .into_iter()
            .filter_map(|first| {
                let def = ast.definition(first)?;
                let n = ast.decls.get(def);
                let complete = ast
                    .decls
                    .record(def)
                    .is_some_and(|r| r.defn.contains(DefinitionBits::COMPLETE));
                (complete
                    && !n.has(DeclFlags::INVALID)
                    // The compiler's own classes (`std::__va_list`): the
                    // target ABI fixes them; no source, no row.
                    && !n.has(DeclFlags::IMPLICIT)
                    && matches!(
                        n.kind(),
                        DeclKind::CXXRecord | DeclKind::ClassTemplateSpecialization
                    )
                    && !is_templated(ast, def)
                    && test.tag(first))
                .then_some((first, def))
            })
            .collect();
        // The vtable groups of the dynamic ones (the builder's cache;
        // classes it lays out now are not candidates).
        let vtables: Vec<Option<std::rc::Rc<kiln_ast::class::VTableLayout>>> = cands
            .iter()
            .map(|&(first, def)| {
                let dynamic = ast.decls.record(def).is_some_and(|r| r.defn.is_dynamic());
                if dynamic { self.vts.vtable_layout(first).ok() } else { None }
            })
            .collect();
        let mut layouts: FxHashMap<DeclId, &kiln_ast::types::RecordLayout> = FxHashMap::default();
        for (d, l) in ast.layouts.iter().chain(self.tys.layouts()).chain(self.vts.layouts()) {
            layouts.entry(ast.first(d)).or_insert(l);
        }
        let mut ser =
            LayoutSerializer { ast, anon: &mut self.anon, test: &mut test, layouts: &layouts };
        let sm = self.sm;
        let mut rows = Vec::with_capacity(cands.len());
        for (&(first, def), vt) in cands.iter().zip(&vtables) {
            let Some(bytes) = ser.class(def, vt.as_deref()) else { continue };
            let Some(l) = layouts.get(&first) else { continue };
            let (Ok(size), Ok(align)) = (u32::try_from(l.size), u16::try_from(l.align)) else {
                continue;
            };
            let defn = ast.decls.record(def).map(|r| r.defn);
            let mut flags = 0;
            if ast.decls.record(def).is_some_and(|r| r.tag == kiln_ast::decl::TagKind::Union) {
                flags |= TYPE_UNION;
            }
            if defn.is_some_and(|b| b.contains(DefinitionBits::POLYMORPHIC)) {
                flags |= TYPE_POLYMORPHIC;
            }
            if !l.vbases.is_empty() {
                flags |= TYPE_HAS_VBASES;
            }
            let loc = ast.decls.get(def).loc();
            let (mut file, mut line) = (String::new(), 0);
            if let Some(sm) = sm {
                if sm.is_in_system_header(loc) {
                    flags |= TYPE_SYSTEM;
                }
                if let Some(p) = sm.presumed_line_col(loc) {
                    file = self.opts.debug.remap(&sm.presumed_file_name(&p, loc));
                    line = p.line;
                }
            }
            let name = ser.type_name(first);
            rows.push(TypeRow {
                key: fnv1a64(name.as_bytes()),
                name,
                layout: fnv1a64(&bytes),
                size,
                align,
                flags,
                file,
                line,
            });
        }
        rows
    }

    /// `sizeof` of a variable's declared type `q`, 0 when it is unknown
    /// here: an array of unknown (or variable) bound, an incomplete class
    /// or enumeration, `void`.
    fn mason_object_size(&mut self, q: QualType) -> u64 {
        let ast = self.ast;
        let mut t = q;
        loop {
            match ast.types.canonical_view(t) {
                TypeView::ConstantArray(_) => match ast.types.element(t) {
                    Some(e) => t = e,
                    None => return 0,
                },
                TypeView::Record(d) | TypeView::Enum(d) if ast.definition(d).is_none() => {
                    return 0;
                }
                TypeView::Record(_) | TypeView::Enum(_) => break,
                TypeView::Builtin(BuiltinType::Void)
                | TypeView::IncompleteArray(_)
                | TypeView::VariableArray { .. }
                | TypeView::DependentSizedArray { .. } => return 0,
                _ => break,
            }
        }
        self.size_align(q).0
    }

    /// `-fmason-metadata`: the module's one `.mason.meta` unit, as
    /// `@__mason_meta = private constant [N x i8] c"...", section
    /// ".mason.meta", align 8, !exclude !{}` listed in
    /// `@llvm.compiler.used` (mortar `spec/meta.md` MT-1, MT-2, MT-2a,
    /// MT-6, MT-20). Called once, before the used lists are written.
    pub(crate) fn emit_mason_meta(&mut self) {
        let Some(m) = self.opts.mason_meta.clone() else {
            return;
        };
        let types = if m.level.writes_types() && self.opts.c.is_none() {
            Some(self.mason_type_rows())
        } else {
            None
        };
        let mut rows = self.mason_summary_rows(&m);
        if types.as_ref().is_some_and(|t| !t.is_empty()) {
            // Written by a unit with kind 2 or 3 rows and by no other
            // (mortar's answer to WP-1557; O-11).
            rows.push(SummaryRow {
                name: "odr.scheme",
                value: ODR_SCHEME.to_owned(),
                enforce: false,
            });
        }
        let mut st = StrTab::default();
        let mut blocks = vec![tu_summary_block(&rows, &mut st)];
        if let Some(types) = &types {
            blocks.push(types_block(types, &mut st));
        }
        if m.level.writes_declarations() {
            let mut decls = self.mason_decl_rows();
            if !m.level.writes_cxx_function_type_names() {
                for r in &mut decls {
                    if r.flags & (DECL_EXTERN_C | DECL_VARIABLE) == 0 {
                        r.type_name.clear();
                    }
                }
            }
            blocks.push(declarations_block(&decls, &mut st));
        }
        let bytes = encode_unit(PRODUCER_KILN, &blocks, &st);
        let init = self.m.const_bytes(&bytes);
        let ty = self.m.types.array(LTy::I8, bytes.len() as u64);
        let g = self.m.add_global("__mason_meta", ty);
        let _ = self.m.set_initializer(g, Some(init));
        let exclude = self.m.md_kind("exclude");
        let empty = self.m.md_tuple(&[]);
        self.m.add_global_metadata(g, exclude, empty);
        let san = &self.opts.sanitize;
        let asan = san.has(SanitizerSet::ADDRESS) || san.has(SanitizerSet::KERNEL_ADDRESS);
        let hwasan = san.has(SanitizerSet::HWADDRESS) || san.has(SanitizerSet::KERNEL_HWADDRESS);
        let gv = self.m.global_mut(g);
        gv.linkage = Linkage::Private;
        gv.is_constant = true;
        gv.section = Some(".mason.meta".into());
        gv.align = Some(8);
        // A redzone would grow the section past the unit and move it.
        gv.sanitizer.no_address = asan;
        gv.sanitizer.no_hwaddress = hwasan;
        self.gi_add_compiler_used(gval(g));
    }
}

#[cfg(test)]
mod tests {
    //! The framing checked by a decoder written from mortar's
    //! `spec/meta.md` §4 alone (it shares no code with the encoder), with
    //! every validity rule of MT-14 asserted on the output.
    use super::*;

    fn u16_at(b: &[u8], o: usize) -> u16 {
        u16::from_le_bytes([b[o], b[o + 1]])
    }
    fn u32_at(b: &[u8], o: usize) -> u32 {
        u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
    }

    #[derive(Debug, PartialEq)]
    struct DecodedBlock {
        kind: u16,
        version: u16,
        rows: Vec<Vec<u8>>,
    }

    #[derive(Debug)]
    struct Decoded {
        producer: u16,
        blocks: Vec<DecodedBlock>,
        strtab: Vec<u8>,
    }

    impl Decoded {
        fn string(&self, off: u32) -> Vec<u8> {
            let s = &self.strtab[off as usize..];
            let end = s.iter().position(|&b| b == 0).expect("MT-14: a string has its NUL");
            s[..end].to_vec()
        }
    }

    /// Decodes one unit at the start of `b`, panicking on any MT-10,
    /// MT-11, MT-12 or MT-14 violation.
    fn decode(b: &[u8]) -> Decoded {
        assert!(b.len() >= 32, "MT-10: a header");
        assert_eq!(&b[0..4], b"MASN", "MT-10 magic");
        assert_eq!(u16_at(b, 4), 1, "major");
        assert_eq!(u16_at(b, 6), 0, "minor");
        let unit_size = u32_at(b, 8) as usize;
        assert_eq!(unit_size % 8, 0, "MT-14: unit_size a multiple of 8");
        assert!(unit_size >= 32, "MT-14: at least a header");
        assert!(unit_size <= b.len(), "MT-14: unit_size within the section");
        let header_size = u32_at(b, 12) as usize;
        assert_eq!(header_size, 32, "MT-10: 32 in 1.0");
        let block_count = u32_at(b, 16) as usize;
        let strtab_offset = u32_at(b, 20) as usize;
        let strtab_size = u32_at(b, 24) as usize;
        let producer = u16_at(b, 28);
        assert_eq!(u16_at(b, 30), 0, "flags 0 in 1.0");
        let mut at = header_size;
        let mut blocks = Vec::new();
        let mut last_kind = None;
        for _ in 0..block_count {
            assert!(at + 16 <= unit_size, "MT-14: a block within the unit");
            let kind = u16_at(b, at);
            let version = u16_at(b, at + 2);
            let block_size = u32_at(b, at + 4) as usize;
            let row_size = u32_at(b, at + 8) as usize;
            let row_count = u32_at(b, at + 12) as usize;
            assert_eq!(at % 8, 0, "blocks are 8-aligned");
            assert_eq!(block_size % 8, 0, "MT-11: block_size a multiple of 8");
            assert_eq!(row_size % 8, 0, "MT-11: row_size a multiple of 8");
            assert!(at + block_size <= unit_size, "MT-14: a block runs past the unit");
            assert!(16 + row_size * row_count <= block_size, "MT-14: rows run past the block");
            if let Some(k) = last_kind {
                assert!(kind > k, "MT-14: blocks strictly increasing by kind");
            }
            last_kind = Some(kind);
            let rows = (0..row_count)
                .map(|i| b[at + 16 + i * row_size..at + 16 + (i + 1) * row_size].to_vec())
                .collect();
            blocks.push(DecodedBlock { kind, version, rows });
            at += block_size;
        }
        let strtab = if strtab_offset == 0 {
            assert_eq!(strtab_size, 0);
            Vec::new()
        } else {
            assert!(strtab_offset >= header_size, "MT-14: the table overlaps the header");
            assert!(strtab_offset + strtab_size <= unit_size, "MT-14: the table runs past");
            let t = b[strtab_offset..strtab_offset + strtab_size].to_vec();
            assert_eq!(t.first(), Some(&0), "MT-12: offset 0 is the empty string");
            assert_eq!(t.last(), Some(&0), "MT-14: the last string has its NUL");
            t
        };
        // Padding is zero (MT-38).
        let end = if strtab_offset == 0 { at } else { strtab_offset + strtab_size };
        assert!(b[end..unit_size].iter().all(|&x| x == 0), "zero padding");
        Decoded { producer, blocks, strtab }
    }

    /// (name, value, flags) of each kind 1 row, checking MT-26's row.
    fn summary(d: &Decoded) -> Vec<(String, String, u32)> {
        let b = d.blocks.iter().find(|b| b.kind == 1).expect("a kind 1 block");
        assert_eq!(b.version, 1);
        let rows: Vec<_> = b
            .rows
            .iter()
            .map(|r| {
                assert_eq!(r.len(), 16, "MT-26 row size");
                assert_eq!(u32_at(r, 12), 0, "reserved");
                let name = String::from_utf8(d.string(u32_at(r, 0))).unwrap();
                let value = String::from_utf8(d.string(u32_at(r, 4))).unwrap();
                (name, value, u32_at(r, 8))
            })
            .collect();
        for w in rows.windows(2) {
            assert!(w[0].0.as_bytes() < w[1].0.as_bytes(), "MT-26: sorted, names unique");
        }
        rows
    }

    fn row(name: &'static str, value: &str, enforce: bool) -> SummaryRow {
        SummaryRow { name, value: value.to_owned(), enforce }
    }

    #[test]
    fn tu_summary_round_trips() {
        let rows = [
            row("std", "c++26", false),
            row("abi.char", "signed", true),
            row("producer", "kiln 9.9.9", false),
            row("abi.rtti", "on", false),
            row("abi.exceptions", "on", false),
            row("abi.char", "unsigned", true),
        ];
        let mut st = StrTab::default();
        let blocks = [tu_summary_block(&rows, &mut st)];
        let bytes = encode_unit(PRODUCER_KILN, &blocks, &st);
        assert_eq!(bytes.len() % 8, 0);
        let d = decode(&bytes);
        assert_eq!(d.producer, 1);
        assert_eq!(d.blocks.len(), 1);
        let got = summary(&d);
        let want = [
            ("abi.char", "signed", 1),
            ("abi.exceptions", "on", 0),
            ("abi.rtti", "on", 0),
            ("producer", "kiln 9.9.9", 0),
            ("std", "c++26", 0),
        ];
        assert_eq!(got.len(), want.len());
        for (g, w) in got.iter().zip(want) {
            assert_eq!((g.0.as_str(), g.1.as_str(), g.2), w);
        }
        // MT-38: strings in the order the sorted rows first use them,
        // each once ("on" is shared).
        let t = &d.strtab;
        let pos = |s: &[u8]| t.windows(s.len()).position(|w| w == s).unwrap();
        assert!(pos(b"abi.char\0") < pos(b"signed\0"));
        assert!(pos(b"signed\0") < pos(b"abi.exceptions\0"));
        assert_eq!(t.windows(3).filter(|w| w == b"on\0").count(), 1);
    }

    #[test]
    fn framing_of_several_blocks_is_sorted_and_padded() {
        let mut st = StrTab::default();
        let a = st.add(b"x");
        let odd = Block { kind: 5, version: 1, row_size: 8, row_count: 1, body: vec![7; 11] };
        let k1 = tu_summary_block(&[row("lang", "c", false)], &mut st);
        let bytes = encode_unit(PRODUCER_KILN, &[odd, k1.clone(), k1], &st);
        let d = decode(&bytes);
        assert_eq!(d.blocks.iter().map(|b| b.kind).collect::<Vec<_>>(), [1, 5]);
        assert_eq!(d.string(a), b"x");
        assert_eq!(summary(&d)[0].0, "lang");
    }

    #[test]
    fn empty_unit_is_a_header() {
        let bytes = encode_unit(PRODUCER_KILN, &[], &StrTab::default());
        assert_eq!(bytes.len(), 32);
        let d = decode(&bytes);
        assert!(d.blocks.is_empty() && d.strtab.is_empty());
    }

    #[test]
    fn same_rows_same_bytes() {
        let enc = |rows: &[SummaryRow]| {
            let mut st = StrTab::default();
            let b = tu_summary_block(rows, &mut st);
            encode_unit(PRODUCER_KILN, &[b], &st)
        };
        let a = [row("isa", "+neon", false), row("lang", "c++", false)];
        let b = [row("lang", "c++", false), row("isa", "+neon", false)];
        assert_eq!(enc(&a), enc(&b));
    }

    #[test]
    fn strings_never_hold_a_nul() {
        let mut st = StrTab::default();
        assert_eq!(st.add(b""), 0);
        let o = st.add(b"a\0b");
        assert_eq!(st.add(b"a"), o);
        let bytes = encode_unit(PRODUCER_KILN, &[], &st);
        assert_eq!(decode(&bytes).string(o), b"a");
    }

    /// (key, type, size, name, `type_name`, flags) of each kind 5 row,
    /// checking O-35's row (40 bytes, reserved zero) and MT-11's order.
    fn declarations(d: &Decoded) -> Vec<(u64, u64, u64, u32, String, u16)> {
        let b = d.blocks.iter().find(|b| b.kind == 5).expect("a kind 5 block");
        assert_eq!(b.version, 1);
        let u64_at = |r: &[u8], o: usize| u64::from_le_bytes(r[o..o + 8].try_into().unwrap());
        let rows: Vec<_> = b
            .rows
            .iter()
            .map(|r| {
                assert_eq!(r.len(), 40, "O-35 row size");
                assert_eq!((u16_at(r, 34), u32_at(r, 36)), (0, 0), "reserved");
                let tn = String::from_utf8(d.string(u32_at(r, 28))).unwrap();
                (u64_at(r, 0), u64_at(r, 8), u64_at(r, 16), u32_at(r, 24), tn, u16_at(r, 32))
            })
            .collect();
        for w in rows.windows(2) {
            assert!((w[0].0, w[0].5) < (w[1].0, w[1].5), "MT-11: sorted by key, then flags");
        }
        rows
    }

    fn decl(name: &str, ty: &str, size: u64, flags: u16) -> DeclRow {
        DeclRow {
            key: fnv1a64(name.as_bytes()),
            flags,
            type_hash: fnv1a64(ty.as_bytes()),
            size,
            type_name: ty.to_owned(),
        }
    }

    #[test]
    fn fnv1a64_vectors() {
        // The published FNV-1a 64 test vectors.
        assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a64(b"foobar"), 0x8594_4171_f739_67e8);
    }

    #[test]
    fn declarations_round_trip() {
        let rows = [
            decl("_Z1fv", "FivE", 0, DECL_DEFINITION),
            decl("x", "i", 4, DECL_VARIABLE | DECL_EXTERN_C),
            decl("_Z1gi", "FivE", 0, DECL_NOEXCEPT),
            decl("x", "i", 4, DECL_VARIABLE | DECL_EXTERN_C),
            decl("arr", "Ai", 0, DECL_VARIABLE | DECL_EXTERN_C),
        ];
        let mut st = StrTab::default();
        let k1 = tu_summary_block(&[row("lang", "c++", false)], &mut st);
        let k5 = declarations_block(&rows, &mut st);
        let bytes = encode_unit(PRODUCER_KILN, &[k5, k1], &st);
        let d = decode(&bytes);
        assert_eq!(d.blocks.iter().map(|b| b.kind).collect::<Vec<_>>(), [1, 5]);
        let got = declarations(&d);
        assert_eq!(got.len(), 4, "the exact duplicate is dropped");
        let mut want: Vec<_> = rows[..4].iter().chain(&rows[4..]).cloned().collect();
        want.sort();
        want.dedup();
        for (g, w) in got.iter().zip(&want) {
            assert_eq!(
                (g.0, g.1, g.2, g.3, g.4.as_str(), g.5),
                (w.key, w.type_hash, w.size, 0, w.type_name.as_str(), w.flags)
            );
        }
        // Each type string once (MT-12 offsets shared), after kind 1's.
        let t = &d.strtab;
        assert_eq!(t.windows(6).filter(|w| w == b"\0FivE\0").count(), 1);
        let pos = |s: &[u8]| t.windows(s.len()).position(|w| w == s).unwrap();
        assert!(pos(b"lang\0") < pos(b"FivE\0"));
    }

    #[test]
    fn levels_and_declarations() {
        assert!(!MasonMetaLevel::Min.writes_declarations());
        for l in [MasonMetaLevel::Default, MasonMetaLevel::Detail, MasonMetaLevel::All] {
            assert!(l.writes_declarations());
            assert_eq!(l.writes_cxx_function_type_names(), l != MasonMetaLevel::Default);
        }
        // An empty kind 5 block is still a valid block.
        let mut st = StrTab::default();
        let bytes = encode_unit(PRODUCER_KILN, &[declarations_block(&[], &mut st)], &st);
        assert!(declarations(&decode(&bytes)).is_empty());
    }

    /// (key, hash, layout, name, size, align, flags, file, line, details)
    /// of each kind 3 row, checking O-17's row (56 bytes) and MT-11's
    /// order (key, then name bytes; names unique).
    #[allow(clippy::type_complexity, reason = "a decoded row")]
    fn types(d: &Decoded) -> Vec<(u64, [u8; 16], u64, String, u32, u16, u16, String, u32, u32)> {
        let b = d.blocks.iter().find(|b| b.kind == 3).expect("a kind 3 block");
        assert_eq!(b.version, 1);
        let u64_at = |r: &[u8], o: usize| u64::from_le_bytes(r[o..o + 8].try_into().unwrap());
        let rows: Vec<_> = b
            .rows
            .iter()
            .map(|r| {
                assert_eq!(r.len(), 56, "O-17 row size");
                let s = |o| String::from_utf8(d.string(u32_at(r, o))).unwrap();
                (
                    u64_at(r, 0),
                    r[8..24].try_into().unwrap(),
                    u64_at(r, 24),
                    s(32),
                    u32_at(r, 36),
                    u16_at(r, 40),
                    u16_at(r, 42),
                    s(44),
                    u32_at(r, 48),
                    u32_at(r, 52),
                )
            })
            .collect();
        for w in rows.windows(2) {
            assert!(
                (w[0].0, w[0].3.as_bytes()) < (w[1].0, w[1].3.as_bytes()),
                "MT-11: sorted by key, then name; one row per name"
            );
        }
        rows
    }

    fn trow(name: &str, layout: u64, size: u32, flags: u16) -> TypeRow {
        TypeRow {
            key: fnv1a64(name.as_bytes()),
            name: name.to_owned(),
            layout,
            size,
            align: 4,
            flags,
            file: "a.h".to_owned(),
            line: 3,
        }
    }

    #[test]
    fn types_round_trip() {
        let rows = [
            trow("1S", 7, 8, 0),
            trow("N2ns1TE", 9, 16, TYPE_POLYMORPHIC),
            trow("1S", 7, 8, 0),
            trow("1U", 1, 4, TYPE_UNION | TYPE_SYSTEM),
            // One name, two different rows: neither is written.
            trow("1X", 1, 4, 0),
            trow("1X", 2, 4, 0),
        ];
        let mut st = StrTab::default();
        let k1 = tu_summary_block(&[row("odr.scheme", ODR_SCHEME, false)], &mut st);
        let k3 = types_block(&rows, &mut st);
        let k5 = declarations_block(&[decl("_Z1fv", "1S", 0, DECL_DEFINITION)], &mut st);
        let bytes = encode_unit(PRODUCER_KILN, &[k5, k3, k1], &st);
        let d = decode(&bytes);
        assert_eq!(d.blocks.iter().map(|b| b.kind).collect::<Vec<_>>(), [1, 3, 5]);
        let got = types(&d);
        let mut want: Vec<&TypeRow> = vec![&rows[0], &rows[1], &rows[3]];
        want.sort_by_key(|r| r.key);
        assert_eq!(got.len(), 3);
        for (g, w) in got.iter().zip(want) {
            assert_eq!(g.0, w.key);
            assert_eq!(g.1, [0; 16], "hash: not hashed");
            assert_eq!(
                (g.2, g.3.as_str(), g.4, g.5, g.6),
                (w.layout, w.name.as_str(), w.size, 4, w.flags)
            );
            assert_eq!((g.7.as_str(), g.8, g.9), ("a.h", 3, 0xffff_ffff));
        }
        // One string table: kind 5's type string `1S` is kind 3's name.
        let t = &d.strtab;
        assert_eq!(t.windows(4).filter(|w| w == b"\x001S\x00").count(), 1);
        assert_eq!(t.windows(5).filter(|w| w == b"\0a.h\0").count(), 1);
        assert_eq!(summary(&d)[0], ("odr.scheme".to_owned(), "kiln-odr-1".to_owned(), 0));
    }

    #[test]
    fn levels_and_types() {
        assert!(!MasonMetaLevel::Min.writes_types());
        for l in [MasonMetaLevel::Default, MasonMetaLevel::Detail, MasonMetaLevel::All] {
            assert!(l.writes_types());
        }
        let mut st = StrTab::default();
        let bytes = encode_unit(PRODUCER_KILN, &[types_block(&[], &mut st)], &st);
        assert!(types(&decode(&bytes)).is_empty());
    }

    #[test]
    fn levels() {
        assert_eq!(MasonMetaLevel::from_name("min"), Some(MasonMetaLevel::Min));
        assert_eq!(MasonMetaLevel::from_name("default"), Some(MasonMetaLevel::Default));
        assert_eq!(MasonMetaLevel::from_name("detail"), Some(MasonMetaLevel::Detail));
        assert_eq!(MasonMetaLevel::from_name("all"), Some(MasonMetaLevel::All));
        assert_eq!(MasonMetaLevel::from_name("max"), None);
    }
}
