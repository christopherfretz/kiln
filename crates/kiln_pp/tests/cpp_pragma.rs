//! [cpp.pragma] Pragma directive; [cpp.pragma.op] the _Pragma operator.
//!
//! Goldens: `clang++-19 -std=c++26 -E -P` (generated).
#![allow(clippy::needless_raw_string_hashes, clippy::doc_markdown, reason = "generated goldens")]

mod common;

use kiln_pp::{MemoryResolver, PrintOptions};

/// [cpp.pragma]/1
#[test]
fn unknown_pragma_passed_through() {
    common::check(
        r#"#pragma foo bar(1, "s")
x
"#,
        r#"#pragma foo bar(1, "s")
x
"#,
    );
}

/// [cpp.pragma]/1
#[test]
fn pragma_not_macro_expanded() {
    common::check(
        r#"#define A 1
#pragma foo A
#pragma STDC FP_CONTRACT ON
"#,
        r#"
#pragma foo A
#pragma STDC FP_CONTRACT ON
"#,
    );
}

/// [cpp.pragma]/1
#[test]
fn pragma_omp_expanded() {
    common::check(
        r#"#define N 4
#pragma omp parallel for num_threads(N)
x
"#,
        r#"
#pragma omp parallel for num_threads(4)
x
"#,
    );
}

/// [cpp.pragma]/1
#[test]
fn pragma_once_in_main_file() {
    common::check_diags(
        r#"#pragma once
x
"#,
        r#"
x
"#,
        &[r#"1:9: warning: #pragma once in main file"#],
    );
}

/// [cpp.pragma]/1
#[test]
fn pragma_once_header() {
    let r = MemoryResolver::new()
        .with_header(
            "a.h",
            r#"#pragma once
int a;
"#,
        )
        .with_header(
            "b.h",
            r#"int b;
"#,
        );
    common::expect(
        &common::run_with(
            r#"#include "a.h"
#include "a.h"
#include "b.h"
#include "b.h"
"#,
            r,
            PrintOptions::default(),
            |_| {},
        ),
        r#"
int a;

int b;
int b;

"#,
        &[],
    );
}

/// [cpp.pragma.op]/1
#[test]
fn pragma_operator() {
    common::check(
        r#"_Pragma("foo \"bar\" \\x") after
"#,
        r#"#pragma foo "bar" \x
                           after
"#,
    );
}

/// [cpp.pragma.op]/2
#[test]
fn pragma_operator_in_macro() {
    common::check(
        r#"#define LISTING(x) PRAGMA(listing on #x)
#define PRAGMA(x) _Pragma(#x)
LISTING( ..\listing.dir )
"#,
        r#"#pragma listing on "..\listing.dir"
"#,
    );
}

/// [cpp.pragma.op]/1
#[test]
fn pragma_operator_wide_strings() {
    common::check(
        r#"_Pragma(L"a") _Pragma(u8"b") _Pragma(R"(c)")
"#,
        r#"#pragma a
#pragma b
#pragma c
"#,
    );
}

/// [cpp.pragma.op]/1
#[test]
fn pragma_operator_malformed() {
    common::check_diags(
        r#"_Pragma x
_Pragma(
_Pragma("a"
"#,
        r#" x
"#,
        &[
            r#"1:1: error: _Pragma takes a parenthesized string literal"#,
            r#"3:1: error: _Pragma takes a parenthesized string literal"#,
            r#"2:1: error: _Pragma takes a parenthesized string literal"#,
        ],
    );
}

/// [cpp.pragma.op]/1
#[test]
fn pragma_operator_once() {
    common::check_diags(
        r#"_Pragma("once") x
"#,
        r#"                x
"#,
        &[r#"1:1: warning: #pragma once in main file"#],
    );
}

/// [cpp.pragma]/1
#[test]
fn push_pop_macro() {
    common::check_diags(
        r#"#define A 1
#pragma push_macro("A")
#undef A
#define A 2
A
#pragma pop_macro("A")
A
#pragma pop_macro("B")
"#,
        r#"2
1
"#,
        &[r#"8:9: warning: pragma pop_macro could not pop 'B', no matching push_macro"#],
    );
}

/// [cpp.pragma]/1
#[test]
fn push_macro_undefined() {
    common::check(
        r#"#pragma push_macro("U")
#define U 3
U
#pragma pop_macro("U")
U
"#,
        r#"3
U
"#,
    );
}

/// [cpp.pragma]/1
#[test]
fn poison() {
    common::check_diags(
        r#"#pragma GCC poison bad worse
int bad;
#define worse 1
#ifdef bad
#endif
"#,
        r#"
int bad;
"#,
        &[
            r#"2:5: error: attempt to use a poisoned identifier"#,
            r#"3:9: error: attempt to use a poisoned identifier"#,
            r#"4:8: error: attempt to use a poisoned identifier"#,
        ],
    );
}

/// [cpp.pragma]/1
#[test]
fn poison_existing_macro() {
    common::check_diags(
        r#"#define M 1
#pragma GCC poison M
"#,
        r#"
"#,
        &[r#"2:20: warning: poisoning existing macro"#],
    );
}

/// [cpp.pragma]/1
#[test]
fn pragma_message() {
    common::check_diags(
        r#"#pragma message("hello")
#pragma message "plain"
#pragma GCC warning "w"
#pragma GCC error "e"
x
"#,
        r#"#pragma message("hello")
#pragma message("plain")
#pragma GCC warning "w"
#pragma GCC error "e"
x
"#,
        &[
            r#"1:9: warning: hello"#,
            r#"2:9: warning: plain"#,
            r#"3:13: warning: w"#,
            r#"4:13: error: e"#,
        ],
    );
}

/// [cpp.pragma]/1
#[test]
fn pragma_message_macro_concat() {
    common::check_diags(
        r#"#define M "a" "b"
#pragma message(M "c")
"#,
        r#"
#pragma message("abc")
"#,
        &[r#"2:9: warning: abc"#],
    );
}

/// [cpp.pragma]/1
#[test]
fn pragma_message_malformed() {
    common::check_diags(
        r#"#pragma message
#pragma message(1)
#pragma message("a"
"#,
        r#"
"#,
        &[
            r#"1:9: error: pragma message requires parenthesized string"#,
            r#"2:17: error: expected string literal in pragma message"#,
            r#"3:20: error: pragma message requires parenthesized string"#,
        ],
    );
}

/// [cpp.pragma]/1
#[test]
fn pragma_diagnostic() {
    common::check_diags(
        r#"#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wfoo"
#pragma clang diagnostic pop
#pragma GCC diagnostic warning "-Wall"
#pragma clang diagnostic bogus
"#,
        r#"#pragma clang diagnostic push
#pragma clang diagnostic pop
#pragma GCC diagnostic warning "-Wall"
"#,
        &[
            r#"2:34: warning: unknown warning group '-Wfoo', ignored"#,
            r#"5:31: warning: pragma diagnostic expected 'error', 'warning', 'ignored', 'fatal', 'push', or 'pop'"#,
        ],
    );
}

/// [cpp.pragma]/1
#[test]
fn pragma_system_header_in_main() {
    common::check_diags(
        r#"#pragma GCC system_header
x
"#,
        r#"
x
"#,
        &[r#"1:13: warning: #pragma system_header ignored in main file"#],
    );
}

/// [cpp.pragma]/1
#[test]
fn pragma_mark_region() {
    common::check(
        r#"#pragma mark - section
#pragma region R
#pragma endregion R
x
"#,
        r#"x
"#,
    );
}

/// [cpp.pragma.op]/1
#[test]
fn pragma_in_macro_args() {
    common::check(
        r#"#define F(x) [x]
F(a _Pragma("p") b)
"#,
        r#"
[a
#pragma p
 b]
"#,
    );
}

/// [cpp.pragma]/2
#[test]
fn pragma_stdc() {
    common::check(
        r#"#pragma STDC FENV_ACCESS ON
#pragma STDC CX_LIMITED_RANGE OFF
"#,
        r#"#pragma STDC FENV_ACCESS ON
#pragma STDC CX_LIMITED_RANGE OFF
"#,
    );
}
