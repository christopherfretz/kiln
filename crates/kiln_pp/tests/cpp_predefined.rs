//! [cpp.predefined] Predefined macro names and the builtin function-like macros the core answers.
//!
//! Goldens: `clang++-19 -std=c++26 -E -P` (generated).
#![allow(clippy::needless_raw_string_hashes, clippy::doc_markdown, reason = "generated goldens")]

mod common;

use kiln_pp::{MemoryResolver, PrintOptions};

/// [cpp.predefined]/1
#[test]
fn cplusplus() {
    common::check(
        r#"__cplusplus __STDC_HOSTED__ __STDCPP_DEFAULT_NEW_ALIGNMENT__ __STDCPP_THREADS__
"#,
        r#"202400L 1 16UL 1
"#,
    );
}

/// [cpp.predefined]/1
#[test]
fn feature_test_macros() {
    common::check(
        r#"__cpp_constexpr __cpp_concepts __cpp_pack_indexing __cpp_static_assert __cpp_variadic_friend
"#,
        r#"202306L 202002 202311L 202306L __cpp_variadic_friend
"#,
    );
}

/// [cpp.predefined]/1
#[test]
fn file_and_line() {
    common::check(
        r#"__FILE__ __LINE__

__LINE__ __FILE_NAME__ __BASE_FILE__
"#,
        r#""t.cpp" 1
3 "t.cpp" "t.cpp"
"#,
    );
}

/// [cpp.predefined]/1
#[test]
fn line_continued_line() {
    common::check(
        r#"a \
__LINE__
"#,
        r#"a 2
"#,
    );
}

/// [cpp.predefined]/1
#[test]
fn line_in_macro_argument() {
    common::check(
        r#"#define F(x, y) x y __LINE__
F(__LINE__,
__LINE__
)
"#,
        r#"
2 3 4
"#,
    );
}

/// [cpp.predefined]/1
#[test]
fn counter() {
    common::check(
        r#"__COUNTER__ __COUNTER__
#define C __COUNTER__
C C
"#,
        r#"0 1
2 3
"#,
    );
}

/// [cpp.predefined]/1
#[test]
fn include_level() {
    let r = MemoryResolver::new().with_header(
        "a.h",
        r#"__INCLUDE_LEVEL__ __FILE_NAME__ __BASE_FILE__
"#,
    );
    common::expect(
        &common::run_with(
            r#"__INCLUDE_LEVEL__
#include "a.h"
"#,
            r,
            PrintOptions::default(),
            |_| {},
        ),
        r#"0
1 "a.h" "t.cpp"

"#,
        &[],
    );
}

/// [cpp.predefined]/1
#[test]
fn flt_eval_method() {
    common::check(
        r#"__FLT_EVAL_METHOD__
"#,
        r#"0
"#,
    );
}

/// [cpp.cond]/2
#[test]
fn has_include() {
    let r = MemoryResolver::new().with_header("a.h", r#""#);
    common::expect(
        &common::run_with(
            r#"#if __has_include("a.h") && __has_include(<a.h>) && !__has_include("nope.h")
yes
#endif
#define HDR <a.h>
#if __has_include(HDR)
macro
#endif
#ifdef __has_include
defined
#endif
"#,
            r,
            PrintOptions::default(),
            |_| {},
        ),
        r#"
yes
macro
defined
"#,
        &[],
    );
}

/// [cpp.cond]/2
#[test]
fn has_include_outside_if() {
    common::check_diags(
        r#"__has_include("a.h")
"#,
        r#"__has_include("a.h")
"#,
        &[r#"1:1: error: '__has_include' must be used within a preprocessing directive"#],
    );
}

/// [cpp.cond]/2
#[test]
fn has_include_malformed() {
    common::check_diags(
        r#"#if __has_include
#endif
#if __has_include(
#endif
#if __has_include("a.h"
#endif
"#,
        r#"
"#,
        &[
            r#"1:18: error: missing '(' after '__has_include'"#,
            r#"1:18: error: expected value in expression"#,
            r#"3:19: error: expected "FILENAME" or <FILENAME>"#,
            r#"3:19: error: expected value in expression"#,
            r#"5:24: error: missing ')' after '__has_include'"#,
            r#"5:24: error: expected value in expression"#,
        ],
    );
}

/// [cpp.cond]/5
#[test]
fn has_cpp_attribute() {
    common::check(
        r#"__has_cpp_attribute(nodiscard) __has_cpp_attribute(maybe_unused) __has_cpp_attribute(assume) __has_cpp_attribute(likely) __has_cpp_attribute(__noreturn__) __has_cpp_attribute(bogus)
"#,
        r#"                             201907L 201603L 202207L 201803L 200809L 0
"#,
    );
}

/// [cpp.cond]/5
#[test]
fn has_cpp_attribute_malformed() {
    common::check_diags(
        r#"__has_cpp_attribute(1)
__has_cpp_attribute nodiscard
"#,
        r#"                     0
                    0
"#,
        &[
            r#"1:21: error: builtin feature check macro requires a parenthesized identifier"#,
            r#"2:21: error: missing '(' after '__has_cpp_attribute'"#,
        ],
    );
}

/// [cpp.predefined]/1
///
/// Deviation: `contract_assert` is a C++26 keyword (P2900); clang 19 predates it and gives 1.
#[test]
fn is_identifier() {
    common::check(
        r#"__is_identifier(int) __is_identifier(foo) __is_identifier(and) __is_identifier(co_await) __is_identifier(contract_assert)
"#,
        r#"                   0 1 0 0 0
"#,
    );
}

/// [cpp.predefined]/1
///
/// `PpOptions::reproducible_time` replaces the clock (clang `-ffixed-date`-like).
#[test]
fn date_time_shape() {
    common::check(
        r#"#define S(x) #x
#define X(x) S(x)
X(__DATE__) X(__TIME__)
"#,
        r#""\"Jan  1 2000\"" "\"00:00:00\""
"#,
    );
}

/// [cpp.predefined]/4
#[test]
fn builtin_macro_defined() {
    common::check(
        r#"#if defined(__LINE__) && defined(__has_include) && defined(__COUNTER__) && defined(_Pragma)
yes
#endif
"#,
        r#"
yes
"#,
    );
}

/// [cpp.predefined]/4
#[test]
fn undef_builtin() {
    common::check_diags(
        r#"#undef __LINE__
__LINE__
"#,
        r#"
__LINE__
"#,
        &[r#"1:8: warning: undefining builtin macro"#],
    );
}

/// [cpp.cond]/2
#[test]
fn has_include_next_in_main() {
    let r = MemoryResolver::new().with_header("a.h", r#""#);
    common::expect(
        &common::run_with(
            r#"#if __has_include_next("a.h")
yes
#endif
"#,
            r,
            PrintOptions::default(),
            |_| {},
        ),
        r#"
yes
"#,
        &[
            r#"1:5: warning: #include_next in primary source file; will search from start of include path"#,
        ],
    );
}
