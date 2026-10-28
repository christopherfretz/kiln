//! [cpp.line] Line control; [cpp.error] diagnostic directives; [cpp.null].
//!
//! Goldens: `clang++-19 -std=c++26 -E -P` (generated).
#![allow(clippy::needless_raw_string_hashes, clippy::doc_markdown, reason = "generated goldens")]

mod common;

/// [cpp.line]/3
#[test]
fn line_number() {
    common::check(
        r#"a __LINE__
#line 100
b __LINE__
c __LINE__
"#,
        r#"a 1
b 100
c 101
"#,
    );
}

/// [cpp.line]/4
#[test]
fn line_number_and_file() {
    common::check(
        r#"#line 7 "other.cpp"
__LINE__ __FILE__
"#,
        r#"7 "other.cpp"
"#,
    );
}

/// [cpp.line]/5
#[test]
fn line_macro_expanded() {
    common::check(
        r#"#define N 42
#define F "f.cpp"
#line N F
__LINE__ __FILE__
"#,
        r#"42 "f.cpp"
"#,
    );
}

/// [cpp.line]/3
#[test]
fn line_out_of_range() {
    common::check_diags(
        r#"#line 0
#line 2147483648
#line 4294967296
"#,
        r#"
"#,
        &[r#"2147483648:7: error: #line directive requires a positive integer argument"#],
    );
}

/// [cpp.line]/3
#[test]
fn line_not_digit_sequence() {
    common::check_diags(
        r#"#line 0x10
#line 1u
#line -1
#line a
"#,
        r#"
"#,
        &[
            r#"1:8: error: #line directive requires a simple digit sequence"#,
            r#"2:8: error: #line directive requires a simple digit sequence"#,
            r#"3:7: error: #line directive requires a positive integer argument"#,
            r#"4:7: error: #line directive requires a positive integer argument"#,
        ],
    );
}

/// [cpp.line]/4
#[test]
fn line_bad_filename() {
    common::check_diags(
        r#"#line 5 foo
#line 6 L"w"
#line 7 "a" extra
"#,
        r#"
"#,
        &[
            r#"1:9: error: invalid filename for #line directive"#,
            r#"2:9: error: invalid filename for #line directive"#,
            r#"3:13: warning: extra tokens at end of #line directive"#,
        ],
    );
}

/// [cpp.line]/3
#[test]
fn line_missing_number() {
    common::check_diags(
        r#"#line
"#,
        r#"
"#,
        &[r#"1:6: error: #line directive requires a positive integer argument"#],
    );
}

/// [cpp.line]/5
#[test]
fn line_marker_gnu() {
    common::check(
        r#"# 33 "g.cpp"
__LINE__ __FILE__
# 40 "h.h" 1 3
__LINE__ __FILE__
"#,
        r#"33 "g.cpp"
40 "h.h"
"#,
    );
}

/// [cpp.line]/5
#[test]
fn line_marker_bad_flags() {
    common::check_diags(
        r#"# 1 "a" 5
# 1 "a" 2
"#,
        r#"
"#,
        &[
            r#"1:9: error: invalid flag line marker directive"#,
            r#"2:9: error: invalid line marker flag '2': cannot pop empty include stack"#,
        ],
    );
}

/// [cpp.line]/5
#[test]
fn line_marker_push_pop() {
    common::check_diags(
        r#"# 1 "a.h" 1
__FILE__ __LINE__
# 5 "t.cpp" 2
__FILE__ __LINE__
# 9 "t.cpp" 2
"#,
        r#""a.h" 1
"t.cpp" 5
"#,
        &[r#"6:13: error: invalid line marker flag '2': cannot pop empty include stack"#],
    );
}

/// [cpp.predefined]/1
#[test]
fn line_in_multiline_invocation() {
    common::check(
        r#"#define F(x) __LINE__ x
F(
1
)
__LINE__
"#,
        r#"
4 1
5
"#,
    );
}

/// [cpp.predefined]/1
#[test]
fn line_in_nested_multiline_invocation() {
    common::check(
        r#"#define F(x) x
#define G(x) F(x __LINE__)
G(a
)
F(__LINE__
)
"#,
        r#"a 4
5
"#,
    );
}

/// [cpp.error]/1
#[test]
fn error_directive() {
    common::check_diags(
        r#"#error this is "bad" 'x'
after
"#,
        r#"
after
"#,
        &[r#"1:2: error: this is "bad" 'x'"#],
    );
}

/// [cpp.error]/1
#[test]
fn warning_directive() {
    common::check_diags(
        r#"#warning careful  now
after
"#,
        r#"
after
"#,
        &[r#"1:2: warning: careful  now"#],
    );
}

/// [cpp.error]/1
#[test]
fn error_empty() {
    common::check_diags(
        r#"#error
#warning
"#,
        r#"
"#,
        &[r#"1:2: error: "#, r#"2:2: warning: "#],
    );
}

/// [cpp.error]/1
#[test]
fn error_unterminated_literal() {
    common::check_diags(
        r#"#error don't
"#,
        r#"
"#,
        &[r#"1:2: error: don't"#],
    );
}

/// [cpp.null]/1
#[test]
fn null_directive() {
    common::check(
        r#"#
# /* comment */
x
"#,
        r#"x
"#,
    );
}

/// [cpp.pre]/1
#[test]
fn unknown_directive() {
    common::check_diags(
        r#"#frobnicate
#123
#"x"
"#,
        r#"
"#,
        &[
            r#"1:2: error: invalid preprocessing directive"#,
            r#"123:2: error: invalid preprocessing directive"#,
        ],
    );
}

/// [cpp.pre]/1
#[test]
fn ident_directive() {
    common::check_diags(
        r#"#ident "v1"
#sccs "v2"
#ident v3
"#,
        r#"#ident "v1"
#ident "v2"
"#,
        &[r#"3:8: error: invalid #ident directive"#],
    );
}

/// [cpp.pre]/1
#[test]
fn assert_unassert() {
    common::check_diags(
        r#"#assert machine(x)
#unassert machine
"#,
        r#"
"#,
        &[
            r#"1:2: error: invalid preprocessing directive"#,
            r#"2:2: error: invalid preprocessing directive"#,
        ],
    );
}

/// [cpp.pre]/1
#[test]
fn conditionally_supported_directive() {
    common::check(
        r#"#if 0
#frobnicate
#endif
x
"#,
        r#"x
"#,
    );
}
