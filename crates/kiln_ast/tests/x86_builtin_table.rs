//! The x86 target builtin table (spec/landed/sema/WP-630.md "Target
//! builtins"), in its plain-data layout (spec/landed/sema/WP-652.md): every
//! row decodes, ids and name lookups agree, and sample rows carry the
//! prototype, attributes, features and immediate checks the generator
//! recorded. Implementation tables, no standard section.

use kiln_ast::decl::builtin::{self, attr, x86};

#[test]
fn every_row_decodes_and_round_trips() {
    let rows: Vec<x86::Row> = x86::rows().collect();
    assert_eq!(rows.len(), x86::len());
    assert_eq!(rows.len(), 1947, "spec/landed/sema/WP-630.md counts 1,947 rows");
    for (i, r) in rows.iter().enumerate() {
        let id = x86::BASE + u32::try_from(i).unwrap();
        assert_eq!(r.info.id, id);
        assert!(x86::is_x86(id));
        assert_eq!(x86::by_name(r.info.name), Some(id), "{}", r.info.name);
        assert_eq!(builtin::by_name(r.info.name), Some(id), "{}", r.info.name);
        let info = builtin::info(id).unwrap();
        assert_eq!((info.name, info.sig, info.attrs), (r.info.name, r.info.sig, r.info.attrs));
        assert!(!r.info.sig.is_empty(), "{}", r.info.name);
        let prefix = if r.header { x86::HEADER_PREFIX } else { x86::PREFIX };
        assert!(r.info.name.starts_with(prefix), "{}", r.info.name);
    }
    assert!(rows.windows(2).all(|w| w[0].info.name < w[1].info.name), "sorted, distinct");
    assert_eq!(rows.iter().filter(|r| r.header).count(), 7);
}

#[test]
fn rows_carry_their_fields() {
    let row = |name| x86::row(x86::by_name(name).unwrap()).unwrap();

    let r = row("__builtin_ia32_aadd32");
    assert_eq!((r.info.sig, r.info.attrs, r.features), ("vv*i", attr::NOTHROW, "raoint"));
    assert!(r.imms.is_empty() && !r.header);

    let r = row("__builtin_ia32_addpd512");
    assert_eq!(r.info.sig, "V8dV8dV8dIi");
    assert_eq!(r.info.attrs, attr::NOTHROW | attr::CONST);
    assert_eq!(r.features, "avx512f,evex512");
    assert_eq!(r.imms, [x86::Imm::Rounding(2)]);

    let r = row("__builtin_ia32_tdpbf16ps");
    assert_eq!((r.info.sig, r.features), ("vIUcIUcIUc", "amx-bf16"));
    assert_eq!(
        r.imms,
        [
            x86::Imm::RangeError(0, 0, 7),
            x86::Imm::RangeError(1, 0, 7),
            x86::Imm::RangeError(2, 0, 7),
            x86::Imm::DistinctTiles,
        ]
    );

    let r = row("__builtin_ia32_readeflags_u32");
    assert_eq!((r.info.sig, r.features), ("Ui", ""));
    assert_eq!(r.imms, [x86::Imm::Only32Bit]);

    let r = row("_mm_pause");
    assert!(r.header);
    assert_eq!(x86::header_of(r.info.id), Some("emmintrin.h"));
    assert_eq!(x86::header_of(row("_mm_getcsr").info.id), Some("xmmintrin.h"));
    assert_eq!(x86::header_of(row("__builtin_ia32_pause").info.id), None);

    assert_eq!(x86::required_features("__builtin_ia32_aadd32"), Some("raoint"));
    assert_eq!(x86::required_features("_mm_pause"), None);
}

#[test]
fn ids_outside_the_table() {
    let end = x86::BASE + u32::try_from(x86::len()).unwrap();
    for id in [0, x86::BASE - 1, end, u32::MAX] {
        assert!(!x86::is_x86(id) && x86::row(id).is_none(), "{id:#x}");
    }
    assert!(x86::row(end - 1).is_some());
    assert_eq!(x86::by_name("__builtin_ia32_not_a_builtin"), None);
    assert_eq!(x86::by_name(""), None);
}
