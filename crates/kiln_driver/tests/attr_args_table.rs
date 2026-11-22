//! WP-1879 (spec/parse.md §6.2 "Argument table"): the argument table
//! generated from clang (`scripts/gen-attr-args.py`,
//! `kiln_sema::attr_args`) against itself and kiln's attribute triage
//! ([dcl.attr.grammar]/4: an attribute's argument clause is
//! implementation-defined; kiln's is clang's).
//!
//! - the rows are sorted by name and one name's configurations do not
//!   overlap, so the lookup finds the one rule;
//! - every spelling a row covers is one kiln's triage knows there (a rule
//!   kiln can never reach would mean the two tables disagree on what
//!   clang knows);
//! - spot checks of rules that differ by release or target.
#![forbid(unsafe_code)]

use kiln_ast::attr::AttrSyntax;
use kiln_sema::attr_args::{self, table_rule};
use kiln_sema::attr_triage::{self, CL, G, GS, Support};

/// (spelling bit, syntax, scope) of the table's spelling indices.
const SPELLINGS: [(u8, AttrSyntax, Option<&str>); 3] = [
    (G, AttrSyntax::Gnu, None),
    (GS, AttrSyntax::Cxx11, Some("gnu")),
    (CL, AttrSyntax::Cxx11, Some("clang")),
];

/// (clang release, x86-64) of the table's configuration indices.
const CONFIGS: [(u32, bool); 4] = [(19, false), (22, false), (19, true), (22, true)];

#[test]
fn rows_are_sorted_and_disjoint() {
    let rows: Vec<_> = attr_args::table_rows().collect();
    assert!(!rows.is_empty());
    for w in rows.windows(2) {
        let ((a, am), (b, bm)) = (w[0], w[1]);
        assert!(a <= b, "rows out of order: {a} before {b}");
        if a == b {
            assert_eq!(am & bm, 0, "{a}: overlapping configurations {am:#x} and {bm:#x}");
        }
    }
}

#[test]
fn every_row_is_known_to_the_triage() {
    let mut unknown = Vec::new();
    for (name, mask) in attr_args::table_rows() {
        for (s, &(_, syntax, scope)) in SPELLINGS.iter().enumerate() {
            for (c, &(compat, x86)) in CONFIGS.iter().enumerate() {
                if mask & (1 << (s * 4 + c)) == 0 {
                    continue;
                }
                // `format` and the C-only class B attributes are decided
                // before the triage (`Sema::attribute_is_unknown`).
                if name == "format" {
                    continue;
                }
                let support =
                    attr_triage::attribute_support(name, syntax, scope, compat, true, x86);
                if support == Support::Unknown {
                    unknown.push(format!("{name} {syntax:?} {scope:?} @{compat} x86={x86}"));
                }
            }
        }
    }
    assert!(unknown.is_empty(), "table rows the triage calls unknown:\n{}", unknown.join("\n"));
}

#[test]
fn rules_follow_release_and_target() {
    // `cold` takes none everywhere.
    let cold = table_rule("cold", G, 22, true).unwrap();
    assert_eq!((cold.min, cold.max), (0, Some(0)));
    // clang 22 gives `malloc` its deallocator arguments.
    assert_eq!(table_rule("malloc", G, 19, false).unwrap().max, Some(0));
    assert_eq!(table_rule("malloc", G, 22, false).unwrap().max, Some(2));
    // `visibility` takes an identifier first and a string at 0.
    let vis = table_rule("visibility", GS, 22, false).unwrap();
    assert!(vis.ident && vis.string_at(0) && !vis.string_at(1));
    // `nvptx_kernel` is clang 22's (a `device_kernel` spelling).
    assert!(table_rule("nvptx_kernel", G, 19, false).is_none());
    assert_eq!(table_rule("nvptx_kernel", CL, 22, false).unwrap().max, Some(0));
    // `gcc_struct` is new in clang 22.
    assert!(table_rule("gcc_struct", G, 19, true).is_none());
    assert!(table_rule("gcc_struct", G, 22, true).is_some());
    // Late-parsed and custom-parsed clauses have no rule.
    assert!(table_rule("guarded_by", G, 22, false).is_none());
    assert!(table_rule("availability", CL, 22, false).is_none());
}
