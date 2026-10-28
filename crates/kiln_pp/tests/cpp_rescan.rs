//! [cpp.rescan] Rescanning and further replacement; [cpp.scope].
//!
//! Goldens: `clang++-19 -std=c++26 -E -P` (generated).
#![allow(clippy::needless_raw_string_hashes, clippy::doc_markdown, reason = "generated goldens")]

mod common;

/// [cpp.rescan]/1
#[test]
fn rescan_object() {
    common::check(
        r#"#define A B
#define B C
#define C 1
A
"#,
        r#"1
"#,
    );
}

/// [cpp.rescan]/2
#[test]
fn self_reference_object() {
    common::check(
        r#"#define A A + 1
#define B C
#define C B
A B C
"#,
        r#"A + 1 B C
"#,
    );
}

/// [cpp.rescan]/2
#[test]
fn self_reference_function() {
    common::check(
        r#"#define f(x) f(x) + 1
f(f(1))
"#,
        r#"
f(f(1) + 1) + 1
"#,
    );
}

/// [cpp.rescan]/2
#[test]
fn painted_blue_stays_blue() {
    common::check(
        r#"#define foo foo
#define bar(x) x
bar(foo)
"#,
        r#"foo
"#,
    );
}

/// [cpp.rescan]/1
#[test]
fn function_name_from_rescan() {
    common::check(
        r#"#define F(x) [x]
#define G F
#define H G(1)
G(2) H G
(3)
"#,
        r#"[2] [1] [3]
"#,
    );
}

/// [cpp.rescan]/1
#[test]
fn function_name_at_end_of_expansion() {
    common::check(
        r#"#define F(x) [x]
#define G F
G (1) G
not_paren
"#,
        r#"[1] F
not_paren
"#,
    );
}

/// [cpp.rescan]/1
#[test]
fn arguments_from_outside_expansion() {
    common::check(
        r#"#define f(a) a*g
#define g(a) f(a)
f(2)(9)
"#,
        r#"2*9*g
"#,
    );
}

/// [cpp.rescan]/2
#[test]
fn nested_self_reference() {
    common::check(
        r#"#define A(x) B(x)
#define B(x) A(x)
A(1) B(A(1))
"#,
        r#"A(1) B(A(1))
"#,
    );
}

/// [cpp.rescan]/3
#[test]
fn directive_like_result_not_directive() {
    common::check(
        r#"#define EMPTY
#define D # define X 1
D
EMPTY # define Y
"#,
        r#"# define X 1
      # define Y
"#,
    );
}

/// [cpp.rescan]/3
#[test]
fn pragma_in_expansion_not_directive() {
    common::check(
        r#"#define P #pragma x
P
"#,
        r#"
 #pragma x
"#,
    );
}

/// [cpp.rescan]/1
#[test]
fn obj_with_paren_after() {
    common::check(
        r#"#define A (x)
#define F(y) <y>
F A
"#,
        r#"F (x)
"#,
    );
}

/// [cpp.rescan]/1
#[test]
fn deep_nesting() {
    common::check(
        r#"#define A0 x
#define A1 A0 A0
#define A2 A1 A1
#define A3 A2 A2
#define A4 A3 A3
A4
"#,
        r#"x x x x x x x x x x x x x x x x
"#,
    );
}

/// [cpp.scope]/1
#[test]
fn scope_undef_redefine() {
    common::check(
        r#"#define A 1
A
#undef A
#define A 2
A
"#,
        r#"
1
2
"#,
    );
}

/// [cpp.scope]/1
#[test]
fn macro_in_directive_line() {
    common::check(
        r#"#define H "x"
#line 10 H
__FILE__ __LINE__
"#,
        r#""x" 10
"#,
    );
}
