//! [cpp.include] Source file inclusion through the `IncludeResolver` (here `MemoryResolver`).
//!
//! Goldens: `clang++-19 -std=c++26 -E -P` (generated).
#![allow(clippy::needless_raw_string_hashes, reason = "generated goldens")]

mod common;

use kiln_pp::{MemoryResolver, PrintOptions};

/// [cpp.include]/2
#[test]
fn include_quoted_and_angled() {
    let r = MemoryResolver::new()
        .with_header("a.h", r#"in_a
"#);
    common::expect(
        &common::run_with(r#"#include "a.h"
#include <a.h>
main
"#, r, PrintOptions::default(), |_| {}),
        r#"in_a
in_a
main
"#,
        &[],
    );
}

/// [cpp.include]/7
#[test]
fn include_macro_name() {
    let r = MemoryResolver::new()
        .with_header("a.h", r#"in_a
"#);
    common::expect(
        &common::run_with(r#"#define Q "a.h"
#define A <a.h>
#include Q
#include A
"#, r, PrintOptions::default(), |_| {}),
        r#"in_a
in_a

"#,
        &[],
    );
}

/// [cpp.include]/7
#[test]
fn include_macro_angled_pieces() {
    let r = MemoryResolver::new()
        .with_header("a.h", r#"in_a
"#);
    common::expect(
        &common::run_with(r#"#define H(x) <x.h>
#include H(a)
"#, r, PrintOptions::default(), |_| {}),
        r#"
in_a

"#,
        &[],
    );
}

/// [cpp.include]/1
#[test]
fn include_not_found() {
    common::check_diags(
        r#"#include "nope.h"
after
"#,
        r#"
after
"#,
        &[r#"1:10: fatal error: 'nope.h' file not found"#],
    );
}

/// [cpp.include]/1
#[test]
fn include_malformed() {
    common::check_diags(
        r#"#include
#include x
#include <a.h
"#,
        r#"
"#,
        &[r#"1:9: error: expected "FILENAME" or <FILENAME>"#, r#"2:10: error: expected "FILENAME" or <FILENAME>"#, r#"3:14: error: expected '>'"#],
    );
}

/// [cpp.include]/1
#[test]
fn include_extra_tokens() {
    let r = MemoryResolver::new()
        .with_header("a.h", r#"in_a
"#);
    common::expect(
        &common::run_with(r#"#include "a.h" junk
"#, r, PrintOptions::default(), |_| {}),
        r#"in_a

"#,
        &[r#"1:16: warning: extra tokens at end of #include directive"#],
    );
}

/// [cpp.include]/6
///
/// clang prints the -I directory in the path (`"inc/b.h"`); MemoryResolver names the file by its include spelling.
#[test]
fn include_nested() {
    let r = MemoryResolver::new()
        .with_header("a.h", r#"a1
#include "b.h"
a2 __LINE__
"#)
        .with_header("b.h", r#"b __FILE__
"#);
    common::expect(
        &common::run_with(r#"#include "a.h"
main __LINE__
"#, r, PrintOptions::default(), |_| {}),
        r#"a1
b "b.h"
a2 3
main 2
"#,
        &[],
    );
}

/// [cpp.include]/6
#[test]
fn include_guard() {
    let r = MemoryResolver::new()
        .with_header("g.h", r#"#ifndef G_H
#define G_H
#define X 1
once
#endif
"#);
    common::expect(
        &common::run_with(r#"#include "g.h"
#include "g.h"
X
"#, r, PrintOptions::default(), |_| {}),
        r#"once

1
"#,
        &[],
    );
}

/// [cpp.include]/6
#[test]
fn include_guard_if_not_defined() {
    let r = MemoryResolver::new()
        .with_header("g.h", r#"#if !defined(G_H)
#define G_H
#define X 1
once
#endif
"#);
    common::expect(
        &common::run_with(r#"#include "g.h"
#include "g.h"
X
"#, r, PrintOptions::default(), |_| {}),
        r#"once

1
"#,
        &[],
    );
}

/// [cpp.include]/6
#[test]
fn include_guard_if_not_paren_defined() {
    let r = MemoryResolver::new()
        .with_header("g.h", r#"#if !(defined G_H)
#define G_H
#define X 1
once
#endif
"#);
    common::expect(
        &common::run_with(r#"#include "g.h"
#include "g.h"
X
"#, r, PrintOptions::default(), |_| {}),
        r#"once

1
"#,
        &[],
    );
}

/// [cpp.include]/6
#[test]
fn include_guard_tokens_after() {
    let r = MemoryResolver::new()
        .with_header("g.h", r#"#ifndef G_H
#define G_H
#define X 1
once
#endif
after
"#);
    common::expect(
        &common::run_with(r#"#include "g.h"
#include "g.h"
X
"#, r, PrintOptions::default(), |_| {}),
        r#"once
after
after
1
"#,
        &[],
    );
}

/// [cpp.include]/6
#[test]
fn include_guard_else() {
    let r = MemoryResolver::new()
        .with_header("g.h", r#"#ifndef G_H
#define G_H
#define X 1
once
#else
again
#endif
"#);
    common::expect(
        &common::run_with(r#"#include "g.h"
#include "g.h"
X
"#, r, PrintOptions::default(), |_| {}),
        r#"once
again
1
"#,
        &[],
    );
}

/// [cpp.include]/6
#[test]
fn include_guard_tokens_before() {
    let r = MemoryResolver::new()
        .with_header("g.h", r#"before
#ifndef G_H
#define G_H
#define X 1
#endif
"#);
    common::expect(
        &common::run_with(r#"#include "g.h"
#include "g.h"
X
"#, r, PrintOptions::default(), |_| {}),
        r#"before
before
1
"#,
        &[],
    );
}

/// [cpp.include]/6
#[test]
fn include_guard_undefined_between() {
    let r = MemoryResolver::new()
        .with_header("g.h", r#"#ifndef G_H
#define G_H
once
#endif
"#);
    common::expect(
        &common::run_with(r#"#include "g.h"
#undef G_H
#include "g.h"
X
"#, r, PrintOptions::default(), |_| {}),
        r#"once

once
X
"#,
        &[],
    );
}

/// [cpp.include]/6
#[test]
fn include_macros_visible() {
    let r = MemoryResolver::new()
        .with_header("a.h", r#"BEFORE
#define AFTER 2
"#);
    common::expect(
        &common::run_with(r#"#define BEFORE 1
#include "a.h"
AFTER
"#, r, PrintOptions::default(), |_| {}),
        r#"
1
2
"#,
        &[],
    );
}

/// [cpp.cond]/1
#[test]
fn include_unterminated_conditional_in_header() {
    let r = MemoryResolver::new()
        .with_header("a.h", r#"#if 1
"#);
    common::expect(
        &common::run_with(r#"#include "a.h"
#endif
"#, r, PrintOptions::default(), |_| {}),
        r#"
"#,
        &[r#"1:2: error: unterminated conditional directive"#, r#"2:2: error: #endif without #if"#],
    );
}

/// [cpp.include]/1
#[test]
fn include_next() {
    let r = MemoryResolver::new()
        .with_header("a.h", r#"first
#include_next <a.h>
"#)
        .with_next_header("nx/a.h", r#"second
"#);
    common::expect(
        &common::run_with(r#"#include "a.h"
"#, r, PrintOptions::default(), |_| {}),
        r#"first

"#,
        &[r#"2:15: fatal error: 'a.h' file not found"#],
    );
}

/// [cpp.include]/1
#[test]
fn import_directive() {
    let r = MemoryResolver::new()
        .with_header("a.h", r#"in_a
"#);
    common::expect(
        &common::run_with(r#"#import "a.h"
#import "a.h"
#include "a.h"
"#, r, PrintOptions::default(), |_| {}),
        r#"in_a

"#,
        &[],
    );
}

/// [cpp.include]/1
#[test]
fn system_header() {
    let r = MemoryResolver::new()
        .with_system_header("s.h", r#"#warning hidden
sys
"#);
    common::expect(
        &common::run_with(r#"#include <s.h>
"#, r, PrintOptions::default(), |_| {}),
        r#"
sys

"#,
        &[r#"1:2: warning: hidden"#],
    );
}

/// [cpp.pragma]/1
#[test]
fn pragma_system_header() {
    let r = MemoryResolver::new()
        .with_header("a.h", r#"x
#pragma GCC system_header
#warning hidden
y
"#);
    common::expect(
        &common::run_with(r#"#include "a.h"
"#, r, PrintOptions::default(), |_| {}),
        r#"x

y

"#,
        &[r#"3:2: warning: hidden"#],
    );
}

/// [cpp.include]/6
#[test]
fn include_depth_line_numbers() {
    let r = MemoryResolver::new()
        .with_header("a.h", r#"


in_a
"#);
    common::expect(
        &common::run_with(r#"#include "a.h"


main
"#, r, PrintOptions::default(), |_| {}),
        r#"in_a
main
"#,
        &[],
    );
}

/// [cpp.include]/6
#[test]
fn include_last_line_without_newline() {
    let r = MemoryResolver::new()
        .with_header("a.h", r#"in_a"#);
    common::expect(
        &common::run_with(r#"#include "a.h"
main
"#, r, PrintOptions::default(), |_| {}),
        r#"in_a
main
"#,
        &[],
    );
}
