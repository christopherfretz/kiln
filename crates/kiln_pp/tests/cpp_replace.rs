//! [cpp.replace] Macro replacement: definition, redefinition, invocation.
//!
//! Goldens: `clang++-19 -std=c++26 -E -P` (generated).
#![allow(clippy::needless_raw_string_hashes, reason = "generated goldens")]

mod common;

/// [cpp.replace.general]/9
#[test]
fn object_like() {
    common::check(
        r#"#define A 1 + 2
int x = A;
"#,
        r#"
int x = 1 + 2;
"#,
    );
}

/// [cpp.replace.general]/9
#[test]
fn object_like_empty() {
    common::check(
        r#"#define E
[E]
"#,
        r#"
[]
"#,
    );
}

/// [cpp.replace.general]/10
#[test]
fn function_like() {
    common::check(
        r#"#define F(a, b) a * b
F(1, 2) F (3,4)
"#,
        r#"
1 * 2 3 * 4
"#,
    );
}

/// [cpp.replace.general]/10
#[test]
fn function_like_without_parens() {
    common::check(
        r#"#define F(a) a
F + F
int F;
"#,
        r#"
F + F
int F;
"#,
    );
}

/// [cpp.replace.general]/11
#[test]
fn invocation_spans_lines() {
    common::check(
        r#"#define F(a, b) [a|b]
F(
1
,
2
) after
"#,
        r#"
[1|2] after
"#,
    );
}

/// [cpp.replace.general]/11
#[test]
fn nested_parens_in_args() {
    common::check(
        r#"#define F(a, b) [a|b]
F((1, 2), (3))
"#,
        r#"
[(1, 2)|(3)]
"#,
    );
}

/// [cpp.replace.general]/11
#[test]
fn empty_arguments() {
    common::check(
        r#"#define F(a, b) [a|b]
F(,) F(1,) F(,2)
#define G(a) [a]
G() G( )
"#,
        r#"
[|] [1|] [|2]
[] []
"#,
    );
}

/// [cpp.replace.general]/12
#[test]
fn too_many_arguments() {
    common::check_diags(
        r#"#define F(a) a
F(1, 2)
"#,
        r#"
F
"#,
        &[r#"2:6: error: too many arguments provided to function-like macro invocation"#],
    );
}

/// [cpp.replace.general]/12
#[test]
fn too_few_arguments() {
    common::check_diags(
        r#"#define F(a, b) a b
F(1)
"#,
        r#"
F
"#,
        &[r#"2:4: error: too few arguments provided to function-like macro invocation"#],
    );
}

/// [cpp.replace.general]/11
#[test]
fn unterminated_invocation() {
    common::check_diags(
        r#"#define F(a) a
F(1
"#,
        r#"
"#,
        &[r#"2:1: error: unterminated function-like macro invocation"#],
    );
}

/// [cpp.replace.general]/12
#[test]
fn directive_inside_arguments() {
    common::check(
        r#"#define F(a) [a]
F(1
#define X 2
X)
X
"#,
        r#"
[1 2]
2
"#,
    );
}

/// [cpp.replace.general]/13
#[test]
fn variadic() {
    common::check(
        r#"#define V(a, ...) a: __VA_ARGS__
V(1) V(1, 2, 3) V(1,)
"#,
        r#"
1: 1: 2, 3 1:
"#,
    );
}

/// [cpp.replace.general]/13
#[test]
fn variadic_only() {
    common::check(
        r#"#define V(...) <__VA_ARGS__>
V() V(x) V(x, (y, z))
"#,
        r#"
<> <x> <x, (y, z)>
"#,
    );
}

/// [cpp.replace.general]/13
#[test]
fn gnu_named_variadic() {
    common::check(
        r#"#define V(args...) <args>
V(1, 2)
"#,
        r#"
<1, 2>
"#,
    );
}

/// [cpp.replace.general]/14
#[test]
fn va_args_outside_variadic() {
    common::check(
        r#"#define A __VA_ARGS__
"#,
        r#"
"#,
    );
}

/// [cpp.replace.general]/2
#[test]
fn identical_redefinition() {
    common::check(
        r#"#define A 1  +  2
#define A 1 + 2
#define F(x) x
#define F( x ) x
A F(3)
"#,
        r#"1 + 2 3
"#,
    );
}

/// [cpp.replace.general]/2
#[test]
fn incompatible_redefinition() {
    common::check_diags(
        r#"#define A 1 + 2
#define A 1+2
#define B 1
#define B 2
#define F(x) x
#define F(y) y
A B F(3)
"#,
        r#"1+2 2 3
"#,
        &[r#"2:9: warning: 'A' macro redefined"#, r#"4:9: warning: 'B' macro redefined"#, r#"6:9: warning: 'F' macro redefined"#],
    );
}

/// [cpp.replace.general]/6
#[test]
fn duplicate_parameter() {
    common::check_diags(
        r#"#define F(a, a) a
"#,
        r#"
"#,
        &[r#"1:14: error: duplicate macro parameter name 'a'"#],
    );
}

/// [cpp.replace.general]/4
#[test]
fn missing_whitespace_after_name() {
    common::check_diags(
        r#"#define A+1
A
"#,
        r#"
+1
"#,
        &[r#"1:10: warning: ISO C99 requires whitespace after the macro name"#],
    );
}

/// [cpp.replace.general]/1
#[test]
fn missing_macro_name() {
    common::check_diags(
        r#"#define
#define 3
#undef
"#,
        r#"
"#,
        &[r#"1:8: error: macro name missing"#, r#"2:9: error: macro name must be an identifier"#, r#"3:7: error: macro name missing"#],
    );
}

/// [cpp.replace.general]/8
#[test]
fn define_defined() {
    common::check_diags(
        r#"#define defined
#undef defined
"#,
        r#"
"#,
        &[r#"1:9: error: 'defined' cannot be used as a macro name"#, r#"2:8: error: 'defined' cannot be used as a macro name"#],
    );
}

/// [cpp.replace.general]/8
#[test]
fn keyword_macro() {
    common::check(
        r#"#define int long
int x;
"#,
        r#"
long x;
"#,
    );
}

/// [cpp.scope]/2
#[test]
fn undef() {
    common::check_diags(
        r#"#define A 1
A
#undef A
A
#undef A
#undef B extra
"#,
        r#"
1
A
"#,
        &[r#"6:10: warning: extra tokens at end of #undef directive"#],
    );
}

/// [cpp.predefined]/4
#[test]
fn builtin_redefinition() {
    common::check_diags(
        r#"#define __LINE__ 1
#undef __FILE__
"#,
        r#"
"#,
        &[r#"1:9: warning: redefining builtin macro"#, r#"2:8: warning: undefining builtin macro"#],
    );
}

/// [cpp.predefined]/4
#[test]
fn predefined_undef() {
    common::check_diags(
        r#"#undef __cplusplus
__cplusplus
"#,
        r#"
__cplusplus
"#,
        &[r#"1:8: warning: undefining builtin macro"#],
    );
}

/// [cpp.replace.general]/10
#[test]
fn function_like_name_as_object() {
    common::check(
        r#"#define f(a) a*g
#define g(a) f(a)
f(2)(9)
"#,
        r#"2*9*g
"#,
    );
}

/// [cpp.replace.general]/1
#[test]
fn bad_parameter_list() {
    common::check_diags(
        r#"#define F(a b) a
#define G(a,) a
#define H(1) a
#define I(a
"#,
        r#"
"#,
        &[r#"1:13: error: expected comma in macro parameter list"#, r#"2:13: error: expected identifier in macro parameter list"#, r#"3:11: error: invalid token in macro parameter list"#, r#"4:12: error: expected comma in macro parameter list"#],
    );
}

/// [cpp.subst]/3
#[test]
fn va_opt_basic() {
    common::check(
        r#"#define F(a, ...) f(a __VA_OPT__(,) __VA_ARGS__)
F(1) F(1, 2) F(1, )
"#,
        r#"
f(1 ) f(1 , 2) f(1 )
"#,
    );
}

/// [cpp.subst]/3
#[test]
fn va_opt_empty_expansion() {
    common::check(
        r#"#define E
#define F(...) [__VA_OPT__(x)]
F() F(E) F(E E) F(1)
"#,
        r#"[] [] [] [x]
"#,
    );
}

/// [cpp.subst]/3
#[test]
fn va_opt_stringize_and_paste() {
    common::check(
        r#"#define S(...) #__VA_OPT__(a b)
#define P(x, ...) x ## __VA_OPT__(y) ## z
S() S(1) P(1) P(1, 2)
"#,
        r#""" "a b" 1z 1yz
"#,
    );
}

/// [cpp.subst]/3
#[test]
fn va_opt_outside_variadic() {
    common::check_diags(
        r#"#define F(a) __VA_OPT__(a)
"#,
        r#"
"#,
        &[r#"1:14: warning: __VA_OPT__ can only appear in the expansion of a variadic macro"#],
    );
}

/// [cpp.subst]/3
#[test]
fn va_opt_unterminated() {
    common::check_diags(
        r#"#define F(...) __VA_OPT__(a
"#,
        r#"
"#,
        &[r#"1:28: error: missing ')' after identifier"#],
    );
}

/// [cpp.subst]/3
#[test]
fn va_opt_nested() {
    common::check_diags(
        r#"#define F(...) __VA_OPT__(__VA_OPT__())
"#,
        r#"
"#,
        &[r#"1:27: error: __VA_OPT__ cannot be nested within its own replacement tokens"#],
    );
}

/// [cpp.subst]/5
#[test]
fn va_opt_standard_example() {
    common::check(
        r#"#define F(...) f(0 __VA_OPT__(,) __VA_ARGS__)
#define G(X, ...) f(0, X __VA_OPT__(,) __VA_ARGS__)
#define SDEF(sname, ...) S sname __VA_OPT__(= { __VA_ARGS__ })
#define EMP
F(a,b,c)
F()
F(EMP)
G(a,b,c)
G(a,)
G(a)
SDEF(foo);
SDEF(bar, 1, 2);
#define H2(X, Y, ...) __VA_OPT__(X ## Y,) __VA_ARGS__
H2(a, b, c, d)
#define H3(X, ...) #__VA_OPT__(X##X X##X)
H3(, 0)
#define H4(X, ...) __VA_OPT__(a X ## X) ## b
H4(, 1)
#define H5A(...) __VA_OPT__()/**/__VA_OPT__()
#define H5B(X) a ## X ## b
#define H5C(X) H5B(X)
H5C(H5A())
"#,
        r#"f(0 , a,b,c)
f(0 )
f(0 )
f(0, a , b,c)
f(0, a )
f(0, a )
S foo ;
S bar = { 1, 2 };
ab, c, d
""
a b
ab
"#,
    );
}

/// [cpp.subst]/1
#[test]
fn arguments_fully_expanded() {
    common::check(
        r#"#define A 1
#define F(x) [x]
#define G(x) F(x)
G(A) F(A)
"#,
        r#"[1] [1]
"#,
    );
}

/// [cpp.subst]/1
#[test]
fn argument_with_unbalanced_macro() {
    common::check(
        r#"#define LP (
#define F(x) x
#define G(x) F x 1)
G(LP)
"#,
        r#"1
"#,
    );
}
