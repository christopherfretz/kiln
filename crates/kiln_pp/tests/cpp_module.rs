//! [cpp.module] / [cpp.import] Module and import directives (printed as text; markers are checked in `hooks.rs`).
//!
//! Goldens: `clang++-19 -std=c++26 -E -P` (generated).
#![allow(clippy::needless_raw_string_hashes, clippy::doc_markdown, reason = "generated goldens")]

mod common;

/// [cpp.module]/1
#[test]
fn module_decl_text() {
    common::check(
        r#"export module m.n:p;
import q;
int x;
"#,
        r#"export module m.n:p;
import q;
int x;
"#,
    );
}

/// [cpp.module]/1
#[test]
fn global_module_fragment() {
    common::check(
        r#"module;
#define A 1
export module m;
int y = A;
"#,
        r#"module;
export module m;
int y = 1;
"#,
    );
}

/// [cpp.module]/3
#[test]
fn module_tokens_expanded() {
    common::check(
        r#"#define ATTR [[]]
export module m ATTR;
"#,
        r#"
export module m [[]];
"#,
    );
}

/// [cpp.module]/2
///
/// Deviation: [cpp.module]/2 makes this ill-formed; clang 19 does not diagnose it.
#[test]
fn module_name_is_macro() {
    common::check_diags(
        r#"#define m x
export module m;
"#,
        r#"
export module x;
"#,
        &[r#"2:15: error: module name component 'm' cannot be an object-like macro"#],
    );
}

/// [cpp.import]/1
///
/// Deviation: header units are not supported (clang looks the header up, then rejects it).
#[test]
fn import_header_unit() {
    common::check_diags(
        r#"import <vector>;
import "a.h";
"#,
        r#"import <vector>;
import "a.h";
"#,
        &[
            r#"1:8: error: importing a header unit is not supported yet"#,
            r#"2:8: error: importing a header unit is not supported yet"#,
        ],
    );
}

/// [cpp.module]/1
#[test]
fn module_not_at_line_start() {
    common::check(
        r#"int module; x module m;
"#,
        r#"int module; x module m;
"#,
    );
}

/// [cpp.module]/1
#[test]
fn module_as_identifier() {
    common::check(
        r#"module = 1;
import + 2;
export { }
"#,
        r#"module = 1;
import + 2;
export { }
"#,
    );
}

/// [cpp.import]/1
#[test]
fn import_private_partition() {
    common::check(
        r#"module :private;
import :p;
"#,
        r#"module :private;
import :p;
"#,
    );
}
