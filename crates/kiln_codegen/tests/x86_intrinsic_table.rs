//! The x86 1:1 intrinsic table (spec/landed/codegen/WP-630.md "1:1
//! intrinsics"), in its plain-data layout (spec/landed/codegen/WP-660.md):
//! every row decodes, the names are sorted and each finds its own row, sample
//! rows carry the fields the pre-WP-660 literal table held, and the whole
//! table hashes to the pre-WP-660 table's hash. Implementation tables, no
//! standard section.

use kiln_codegen::x86_intrinsic_table::{self as t, Map, Mem};

/// FNV-1a (64-bit) of `bytes`, continuing from `h`.
fn fnv(mut h: u64, bytes: &[u8]) -> u64 {
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

fn m(
    name: &'static str,
    intr: &'static str,
    ret: &'static str,
    params: &'static str,
    mem: Mem,
) -> Map {
    Map { name, intr, ret, params, mem }
}

#[test]
fn every_row_decodes_and_round_trips() {
    let rows: Vec<Map> = t::rows().collect();
    assert_eq!(rows.len(), t::len());
    assert_eq!(rows.len(), 1129, "spec/landed/codegen/WP-630.md counts 1,129 rows");
    assert!(rows.windows(2).all(|w| w[0].name < w[1].name), "sorted, distinct");
    for r in &rows {
        assert_eq!(t::by_name(r.name), Some(*r), "{}", r.name);
        assert!(r.intr.starts_with("llvm.x86."), "{}", r.name);
        assert!(!r.ret.is_empty(), "{}", r.name);
        assert!(r.params.is_empty() || r.params.split(", ").all(|p| !p.is_empty()), "{}", r.name);
    }
}

/// The hash of every row as `name\0intr\0ret\0params\0Mem\n`, computed from
/// main's literal `m("name", "intr", "ret", "params", Mem::X)` rows at
/// d47d9d1a6 (before WP-660): the decoded table is byte-identical to it.
#[test]
fn whole_table_matches_the_literal_one() {
    let mut h = 0xcbf2_9ce4_8422_2325;
    for r in t::rows() {
        let line = format!("{}\0{}\0{}\0{}\0{:?}\n", r.name, r.intr, r.ret, r.params, r.mem);
        h = fnv(h, line.as_bytes());
    }
    assert_eq!(h, 0x58a7_4d6e_9fc6_1e07);
}

#[test]
fn rows_carry_their_fields() {
    let expected = [
        m("__builtin_ia32_aadd32", "llvm.x86.aadd32", "void", "ptr, i32", Mem::UnknownArgReadWrite),
        m(
            "__builtin_ia32_addpd512",
            "llvm.x86.avx512.add.pd.512",
            "<8 x double>",
            "<8 x double>, <8 x double>, i32 immarg",
            Mem::None,
        ),
        m(
            "__builtin_ia32_cvtpd2udq512_mask",
            "llvm.x86.avx512.mask.cvtpd2udq.512",
            "<8 x i32>",
            "<8 x double>, <8 x i32>, i8, i32 immarg",
            Mem::None,
        ),
        m(
            "__builtin_ia32_gather3siv8sf",
            "llvm.x86.avx512.mask.gather3siv8.sf",
            "<8 x float>",
            "<8 x float>, ptr, <8 x i32>, <8 x i1>, i32 immarg",
            Mem::Read,
        ),
        m(
            "__builtin_ia32_maskloadd",
            "llvm.x86.avx2.maskload.d",
            "<4 x i32>",
            "ptr, <4 x i32>",
            Mem::ArgRead,
        ),
        m(
            "__builtin_ia32_pmovdb128mem_mask",
            "llvm.x86.avx512.mask.pmov.db.mem.128",
            "void",
            "ptr, <4 x i32>, i8",
            Mem::ArgReadWrite,
        ),
        m(
            "__builtin_ia32_pmovsqw512mem_mask",
            "llvm.x86.avx512.mask.pmovs.qw.mem.512",
            "void",
            "ptr, <8 x i64>, i8",
            Mem::ArgReadWrite,
        ),
        m(
            "__builtin_ia32_subsd_round_mask",
            "llvm.x86.avx512.mask.sub.sd.round",
            "<2 x double>",
            "<2 x double>, <2 x double>, <2 x double>, i8, i32 immarg",
            Mem::None,
        ),
        m("__builtin_ia32_urdmsr", "llvm.x86.urdmsr", "i64", "i64", Mem::UnknownInaccessible),
        m("__builtin_ia32_cldemote", "llvm.x86.cldemote", "void", "ptr", Mem::Unknown),
        m("_mm_sfence", "llvm.x86.sse.sfence", "void", "", Mem::Unknown),
    ];
    for e in expected {
        assert_eq!(t::by_name(e.name), Some(e), "{}", e.name);
    }
    let rows: Vec<Map> = t::rows().collect();
    assert_eq!(rows[0], expected[0]);
    assert_eq!(rows[4], expected[1]);
    assert_eq!(rows[100], expected[2]);
    assert_eq!(rows[500], expected[6]);
    assert_eq!(rows[777], expected[7]);
    assert_eq!(rows[1128], expected[10]);
}

#[test]
fn names_outside_the_table() {
    for n in ["", "__builtin_ia32_", "__builtin_ia32_not_a_builtin", "_mm_pause_", "zzz"] {
        assert_eq!(t::by_name(n), None, "{n}");
    }
    // A builtin clang lowers with generic IR, not 1:1.
    assert_eq!(t::by_name("__builtin_ia32_pshufd"), None);
}
