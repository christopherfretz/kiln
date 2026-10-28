//! Driver options of the preprocessor (`PpOptions`): `-C`, `-CC`, `-D`, `-U`, include depth; [cpp.predefined]/4.
//!
//! Goldens: `clang++-19 -std=c++26 -E -P` (generated).
#![allow(clippy::needless_raw_string_hashes, clippy::doc_markdown, reason = "generated goldens")]

mod common;

use kiln_pp::{MemoryResolver, PrintOptions};

/// [lex.comment]/1
#[test]
fn keep_comments() {
    let r = MemoryResolver::new();
    common::expect(
        &common::run_with(
            r#"a /* c1 */ b // c2
/* multi
   line */ c
"#,
            r,
            PrintOptions::default(),
            |o| o.keep_comments = true,
        ),
        r#"a /* c1 */ b // c2
/* multi
   line */ c
"#,
        &[],
    );
}

/// [lex.comment]/1
#[test]
fn keep_comments_not_in_macros() {
    let r = MemoryResolver::new();
    common::expect(
        &common::run_with(
            r#"#define M a /* in */ b
M // tail
"#,
            r,
            PrintOptions::default(),
            |o| o.keep_comments = true,
        ),
        r#"
a b // tail
"#,
        &[],
    );
}

/// [lex.comment]/1
///
/// Deviation: comments in `#define` bodies are not retained (clang prints `a /* in */ b`).
#[test]
fn keep_macro_comments() {
    let r = MemoryResolver::new();
    common::expect(
        &common::run_with(
            r#"#define M a /* in */ b
M // tail
"#,
            r,
            PrintOptions::default(),
            |o| o.keep_macro_comments = true,
        ),
        r#"
a b // tail
"#,
        &[],
    );
}

/// [lex.comment]/1
#[test]
fn keep_macro_comments_in_args() {
    let r = MemoryResolver::new();
    common::expect(
        &common::run_with(
            r#"#define F(x) [x]
F(a /* c */ b)
"#,
            r,
            PrintOptions::default(),
            |o| o.keep_macro_comments = true,
        ),
        r#"
[a /* c */ b]
"#,
        &[],
    );
}

/// [cpp.predefined]/4
#[test]
fn command_line_define() {
    let r = MemoryResolver::new();
    common::expect(
        &common::run_with(
            r#"A B C
#ifdef U
u
#endif
"#,
            r,
            PrintOptions::default(),
            |o| {
                o.defines = vec![
                    ("A".into(), None),
                    ("B".into(), Some("two".into())),
                    ("C(x)".into(), Some("x".into())),
                    ("U".into(), None),
                ];
                o.undefines = vec!["U".into()];
            },
        ),
        r#"1 two C
"#,
        &[],
    );
}

/// [cpp.include]/6
///
/// clang 19 uses its default limit of 200 here; the text and diagnostic do not depend on it.
#[test]
fn include_depth_limit() {
    let r = MemoryResolver::new().with_header(
        "r.h",
        r#"#include "r.h"
"#,
    );
    common::expect(
        &common::run_with(
            r#"#include "r.h"
"#,
            r,
            PrintOptions::default(),
            |o| o.max_include_depth = 3,
        ),
        r#"
"#,
        &[r#"1:10: error: #include nested too deeply"#],
    );
}
