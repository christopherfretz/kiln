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

use kiln_ast::sanitize::SanitizerSet;
use kiln_ast::types::BuiltinType;
use kiln_llvm::{Linkage, TypeId as LTy};
use kiln_source::fx::FxHashMap;

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

/// How much `-fmason-metadata=<level>` asks for (mortar `spec/odr.md`
/// K-1: `min` kinds 1 and 2; `default` 1, 2, 3, 5; `detail` + 4; `all` +
/// rows for definitions inlined away). Kiln writes kind 1 alone at every
/// level today: the other kinds are later work.
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
    /// The level (recorded; every level writes kind 1 alone today).
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
    /// `abi.stdlib-abi`: the library's ABI macro's value at the end of the
    /// TU; `None` (no row) when the TU defines none (no library header).
    pub abi: Option<String>,
    /// `abi.stdlib-debug`: the debug or hardening mode and whether it
    /// changes layouts (the row's `ENFORCE`); `None` (no row) when the TU
    /// includes no library header.
    pub debug: Option<(String, bool)>,
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
            if let Some((mode, layout)) = &s.debug {
                rows.push(row("abi.stdlib-debug", mode.clone(), *layout));
            }
        }
        rows
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
        let rows = self.mason_summary_rows(&m);
        let mut st = StrTab::default();
        let blocks = [tu_summary_block(&rows, &mut st)];
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

    #[test]
    fn levels() {
        assert_eq!(MasonMetaLevel::from_name("min"), Some(MasonMetaLevel::Min));
        assert_eq!(MasonMetaLevel::from_name("default"), Some(MasonMetaLevel::Default));
        assert_eq!(MasonMetaLevel::from_name("detail"), Some(MasonMetaLevel::Detail));
        assert_eq!(MasonMetaLevel::from_name("all"), Some(MasonMetaLevel::All));
        assert_eq!(MasonMetaLevel::from_name("max"), None);
    }
}
