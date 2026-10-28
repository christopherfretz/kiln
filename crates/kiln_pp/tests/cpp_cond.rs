//! [cpp.cond] Conditional inclusion: group structure and skipping (expressions via the stub evaluator).
//!
//! Goldens: `clang++-19 -std=c++26 -E -P` (generated).
#![allow(clippy::needless_raw_string_hashes, clippy::doc_markdown, reason = "generated goldens")]

mod common;

/// [cpp.cond]/1
#[test]
fn if_true_false() {
    common::check(
        r#"#if 1
a
#endif
#if 0
b
#endif
"#,
        r#"
a
"#,
    );
}

/// [cpp.cond]/1
#[test]
fn else_branches() {
    common::check(
        r#"#if 0
a
#else
b
#endif
#if 1
c
#else
d
#endif
"#,
        r#"b
c
"#,
    );
}

/// [cpp.cond]/1
#[test]
fn elif_chain() {
    common::check(
        r#"#define X 2
#if X == 1
a
#elif X == 2
b
#elif X == 2
c
#else
d
#endif
"#,
        r#"b
"#,
    );
}

/// [cpp.cond]/14
#[test]
fn ifdef_ifndef() {
    common::check(
        r#"#define A
#ifdef A
a
#endif
#ifndef A
b
#endif
#ifdef B
c
#endif
#ifndef B
d
#endif
"#,
        r#"a
d
"#,
    );
}

/// [cpp.cond]/14
#[test]
fn elifdef_elifndef() {
    common::check(
        r#"#define B
#if 0
a
#elifdef B
b
#endif
#ifdef A
c
#elifndef A
d
#endif
"#,
        r#"b
d
"#,
    );
}

/// [cpp.cond]/10
#[test]
fn defined_operator() {
    common::check(
        r#"#define A
#if defined A && defined(A) && !defined B
yes
#endif
"#,
        r#"yes
"#,
    );
}

/// [cpp.cond]/1
#[test]
fn nested_skipped_groups() {
    common::check_diags(
        r#"#if 0
#if garbage (
#else
#elif
#endif
x
#else
y
#endif
"#,
        r#"y
"#,
        &[r#"4:2: error: #elif after #else"#],
    );
}

/// [cpp.cond]/1
#[test]
fn skipped_group_ignores_bad_directives() {
    common::check(
        r#"#if 0
#bogus
#define
#include
#error no
'unterminated
#endif
ok
"#,
        r#"ok
"#,
    );
}

/// [cpp.cond]/1
#[test]
fn skipped_elif_not_evaluated() {
    common::check(
        r#"#if 1
a
#elif 1/0
b
#endif
"#,
        r#"
a
"#,
    );
}

/// [cpp.cond]/13
#[test]
fn undefined_identifier_is_zero() {
    common::check(
        r#"#if UNDEFINED
a
#else
b
#endif
"#,
        r#"b
"#,
    );
}

/// [cpp.cond]/13
#[test]
fn true_false_keywords() {
    common::check(
        r#"#if true && !false
a
#endif
"#,
        r#"
a
"#,
    );
}

/// [cpp.cond]/13
#[test]
fn arithmetic() {
    common::check(
        r#"#if (1 + 2 * 3 == 7) && (-1 < 0) && (0x10 == 16) && (010 == 8) && (1'000 == 1000) && (5 % 3 == 2) && (1 << 4 == 16)
ok
#endif
#if (1 ? 2 : 3) == 2 && (0 || 1) && ~0 == -1
ok2
#endif
"#,
        r#"
ok
ok2
"#,
    );
}

/// [cpp.cond]/1
#[test]
fn else_after_else() {
    common::check_diags(
        r#"#if 1
#else
#else
#endif
"#,
        r#"
"#,
        &[r#"3:2: error: #else after #else"#],
    );
}

/// [cpp.cond]/1
#[test]
fn elif_after_else() {
    common::check_diags(
        r#"#if 0
#else
#elif 1
#endif
"#,
        r#"
"#,
        &[r#"3:2: error: #elif after #else"#],
    );
}

/// [cpp.cond]/1
#[test]
fn endif_without_if() {
    common::check_diags(
        r#"#endif
#else
#elif 1
"#,
        r#"
"#,
        &[
            r#"1:2: error: #endif without #if"#,
            r#"2:2: error: #else without #if"#,
            r#"3:2: error: #elif without #if"#,
        ],
    );
}

/// [cpp.cond]/1
#[test]
fn unterminated_conditional() {
    common::check_diags(
        r#"#if 1
a
"#,
        r#"
a
"#,
        &[r#"1:2: error: unterminated conditional directive"#],
    );
}

/// [cpp.cond]/1
#[test]
fn missing_expression() {
    common::check_diags(
        r#"#if
#endif
#ifdef
#endif
"#,
        r#"
"#,
        &[r#"1:4: error: expected value in expression"#, r#"3:7: error: macro name missing"#],
    );
}

/// [cpp.cond]/1
#[test]
fn extra_tokens_after_endif() {
    common::check_diags(
        r#"#if 1
#else junk
#endif junk
#ifdef A junk
#endif
"#,
        r#"
"#,
        &[
            r#"2:7: warning: extra tokens at end of #else directive"#,
            r#"3:8: warning: extra tokens at end of #endif directive"#,
            r#"4:10: warning: extra tokens at end of #ifdef directive"#,
        ],
    );
}

/// [cpp.cond]/14
#[test]
fn ifdef_non_identifier() {
    common::check_diags(
        r#"#ifdef 3
#endif
"#,
        r#"
"#,
        &[r#"1:8: error: macro name must be an identifier"#],
    );
}

/// [cpp.cond]/13
#[test]
fn defined_in_macro() {
    common::check_diags(
        r#"#define D defined(X)
#if D
a
#else
b
#endif
"#,
        r#"b
"#,
        &[r#"2:5: warning: macro expansion producing 'defined' has undefined behavior"#],
    );
}

/// [cpp.cond]/13
#[test]
fn defined_in_function_like_macro() {
    common::check(
        r#"#define D(x) defined(x)
#if D(X)
a
#else
b
#endif
"#,
        r#"b
"#,
    );
}

/// [cpp.cond]/1
#[test]
fn has_include_in_skipped() {
    common::check(
        r#"#if 0
#if __has_include(<nope>)
#endif
#endif
ok
"#,
        r#"ok
"#,
    );
}

/// [cpp.cond]/1
#[test]
fn conditional_in_macro_args() {
    common::check(
        r#"#define F(x) [x]
F(a
#if 1
b
#else
c
#endif
)
"#,
        r#"
[a b]
"#,
    );
}

/// [cpp.cond]/13
#[test]
fn division_by_zero() {
    common::check_diags(
        r#"#if 1 / 0
#endif
"#,
        r#"
"#,
        &[r#"1:7: error: division by zero in preprocessor expression"#],
    );
}

/// [cpp.cond]/13
#[test]
fn missing_paren() {
    common::check_diags(
        r#"#if (1
#endif
"#,
        r#"
"#,
        &[r#"1:7: error: expected ')' in preprocessor expression"#],
    );
}
