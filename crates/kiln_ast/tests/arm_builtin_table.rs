//! The `AArch64` ACLE target builtin table (spec/landed/sema/WP-724.md
//! "ACLE builtins"): the rows are sorted and unique, every row's id and
//! name lookup agree with the global builtin lookups, and sample rows
//! carry clang 19's `BuiltinsAArch64.def` prototypes and features.
//! Implementation tables, no standard section.

use kiln_ast::decl::builtin::{self, arm, attr};

fn rows() -> Vec<arm::Row> {
    (0..arm::len()).map(|i| arm::row(arm::BASE + i as u32).expect("row in range")).collect()
}

#[test]
fn rows_are_sorted_and_round_trip() {
    let rows = rows();
    assert!(!rows.is_empty());
    for w in rows.windows(2) {
        assert!(w[0].info.name < w[1].info.name, "{} !< {}", w[0].info.name, w[1].info.name);
    }
    for r in &rows {
        assert!(r.info.name.starts_with(arm::PREFIX));
        assert!(arm::is_arm(r.info.id));
        assert_eq!(arm::by_name(r.info.name), Some(r.info.id));
        assert_eq!(builtin::by_name(r.info.name), Some(r.info.id));
        let info = builtin::info(r.info.id).expect("global info");
        assert_eq!((info.name, info.sig, info.attrs), (r.info.name, r.info.sig, r.info.attrs));
    }
    assert!(!arm::is_arm(arm::BASE + arm::len() as u32));
    assert!(arm::row(arm::BASE - 1).is_none());
    assert_eq!(arm::by_name("__builtin_arm_no_such_builtin"), None);
}

#[test]
fn sample_rows_match_clang_19() {
    let row = |n: &str| arm::row(arm::by_name(n).expect(n)).expect(n);
    let crc = row("__builtin_arm_crc32cd");
    assert_eq!((crc.info.sig, crc.features), ("UiUiWUi", "crc"));
    assert_eq!(crc.info.attrs, attr::NOTHROW | attr::CONST);
    assert_eq!(row("__builtin_arm_clz").info.sig, "UiZUi");
    assert_eq!(row("__builtin_arm_gcsss").info.sig, "vC*vC*");
    assert_eq!(row("__builtin_arm_trap").info.attrs & attr::NORETURN, attr::NORETURN);
    assert_eq!(row("__builtin_arm_ldrex").info.attrs & attr::CUSTOM, attr::CUSTOM);
}

#[test]
fn exclusive_accesses_are_classified() {
    let ex = |n: &str| arm::exclusive_load(arm::by_name(n).expect(n));
    assert_eq!(ex("__builtin_arm_ldrex"), Some(true));
    assert_eq!(ex("__builtin_arm_ldaex"), Some(true));
    assert_eq!(ex("__builtin_arm_strex"), Some(false));
    assert_eq!(ex("__builtin_arm_stlex"), Some(false));
    assert_eq!(ex("__builtin_arm_clrex"), None);
}
