//! [cpp.stringize] The # operator and [cpp.concat] the ## operator.
//!
//! Goldens: `clang++-19 -std=c++26 -E -P` (generated).
#![allow(clippy::needless_raw_string_hashes, reason = "generated goldens")]

mod common;

/// [cpp.stringize]/2
#[test]
fn stringize_basic() {
    common::check(
        r#"#define S(x) #x
S(a) S( a  b ) S() S(a
 b)
"#,
        r#"
"a" "a b" "" "a b"
"#,
    );
}

/// [cpp.stringize]/2
#[test]
fn stringize_escapes() {
    common::check(
        r#"#define S(x) #x
S("a\n" '"' '\'' L"x" u8"\\")
"#,
        r#"
"\"a\\n\" '\"' '\\'' L\"x\" u8\"\\\\\""
"#,
    );
}

/// [cpp.stringize]/2
#[test]
fn stringize_raw_string() {
    common::check(
        r#"#define S(x) #x
S(R"(a"\b)")
"#,
        r#"
"R\"(a\"\\b)\""
"#,
    );
}

/// [cpp.stringize]/2
#[test]
fn stringize_unexpanded() {
    common::check(
        r#"#define A 1
#define S(x) #x
#define X(x) S(x)
S(A) X(A)
"#,
        r#""A" "1"
"#,
    );
}

/// [cpp.stringize]/2
#[test]
fn stringize_variadic() {
    common::check(
        r#"#define S(...) #__VA_ARGS__
S() S(a,b) S(a , b)
"#,
        r#"
"" "a,b" "a , b"
"#,
    );
}

/// [cpp.stringize]/1
#[test]
fn stringize_non_parameter() {
    common::check_diags(
        r#"#define S(x) #y
#define T(x) # 1
"#,
        r#"
"#,
        &[r#"1:15: error: '#' is not followed by a macro parameter"#, r#"2:16: error: '#' is not followed by a macro parameter"#],
    );
}

/// [cpp.stringize]/1
#[test]
fn stringize_object_like() {
    common::check(
        r#"#define S # x
S
"#,
        r#"
 # x
"#,
    );
}

/// [cpp.stringize]/2
#[test]
fn stringize_comments() {
    common::check(
        r#"#define S(x) #x
S(a/**/b /* c */ d // e
)
"#,
        r#"
"a b d"
"#,
    );
}

/// [cpp.stringize]/2
#[test]
fn stringize_digraph() {
    common::check(
        r#"#define S(x) %:x
S(<: :>)
"#,
        r#"
"<: :>"
"#,
    );
}

/// [cpp.concat]/3
#[test]
fn concat_basic() {
    common::check(
        r#"#define C(a, b) a ## b
C(x, y) C(1, 2) C(+, =) C(<, <=) C(., 5)
"#,
        r#"
xy 12 += <<= .5
"#,
    );
}

/// [cpp.concat]/3
#[test]
fn concat_empty() {
    common::check(
        r#"#define C(a, b) [a ## b]
C(,) C(x,) C(,y)
"#,
        r#"
[] [x] [y]
"#,
    );
}

/// [cpp.concat]/3
#[test]
fn concat_placemarker_chain() {
    common::check(
        r#"#define C3(a, b, c) a ## b ## c
C3(,,) C3(x,,) C3(,,z) C3(x,,z)
"#,
        r#"
       x z xz
"#,
    );
}

/// [cpp.concat]/3
#[test]
fn concat_result_rescanned() {
    common::check(
        r#"#define xy 42
#define C(a, b) a ## b
C(x, y)
"#,
        r#"42
"#,
    );
}

/// [cpp.concat]/3
#[test]
fn concat_invalid() {
    common::check_diags(
        r#"#define C(a, b) a ## b
C(+, -) C(x, +)
"#,
        r#"
+- x+
"#,
        &[r#"2:1: error: pasting formed '+-', an invalid preprocessing token"#, r#"2:9: error: pasting formed 'x+', an invalid preprocessing token"#],
    );
}

/// [cpp.concat]/1
#[test]
fn concat_at_edges() {
    common::check_diags(
        r#"#define A ## x
#define B x ##
#define F(a) ## a
"#,
        r#"
"#,
        &[r#"1:11: error: '##' cannot appear at start of macro expansion"#, r#"2:13: error: '##' cannot appear at end of macro expansion"#, r#"3:14: error: '##' cannot appear at start of macro expansion"#],
    );
}

/// [cpp.concat]/2
#[test]
fn concat_not_expanded() {
    common::check(
        r#"#define A 1
#define C(a, b) a ## b
#define X(a, b) C(a, b)
C(A, A) X(A, A)
"#,
        r#"AA 11
"#,
    );
}

/// [cpp.concat]/4
#[test]
fn concat_stringize_order() {
    common::check(
        r#"#define hash_hash # ## #
#define mkstr(a) # a
#define in_between(a) mkstr(a)
#define join(c, d) in_between(c hash_hash d)
char p[] = join(x, y);
"#,
        r#"char p[] = "x ## y";
"#,
    );
}

/// [cpp.concat]/3
#[test]
fn concat_gnu_comma() {
    common::check(
        r#"#define E(fmt, ...) f(fmt, ## __VA_ARGS__)
E(a) E(a, b) E(a,)
#define G(fmt, args...) f(fmt, ## args)
G(a) G(a, b)
"#,
        r#"
f(a) f(a, b) f(a,)
f(a) f(a, b)
"#,
    );
}

/// [cpp.concat]/3
#[test]
fn concat_ud_literals() {
    common::check(
        r#"#define C(a, b) a ## b
C(1, _x) C("s", _y) C(L, "w") C(u8, 'c')
"#,
        r#"
1_x "s"_y L"w" u8'c'
"#,
    );
}

/// [cpp.concat]/3
#[test]
fn concat_creates_comment() {
    common::check_diags(
        r#"#define C(a, b) a ## b
C(/, /) C(/, *)
"#,
        r#"
/ / / *
"#,
        &[r#"2:1: error: pasting formed '//', an invalid preprocessing token"#, r#"2:9: error: pasting formed '/*', an invalid preprocessing token"#],
    );
}

/// [cpp.scope]/5
#[test]
fn standard_example_1() {
    common::check(
        r#"#define x 3
#define f(a) f(x * (a))
#undef x
#define x 2
#define g f
#define z z[0]
#define h g(~
#define m(a) a(w)
#define w 0,1
#define t(a) a
#define p() int
#define q(x) x
#define r(x,y) x ## y
#define str(x) # x
f(y+1) + f(f(z)) % t(t(g)(0) + t)(1);
g(x+(3,4)-w) | h 5) & m
(f)^m(m);
p() i[q()] = { q(1), r(2,3), r(4,), r(,5), r(,) };
char c[2][6] = { str(hello), str() };
"#,
        r#"f(2 * (y+1)) + f(2 * (f(2 * (z[0])))) % f(2 * (0)) + t(1);
f(2 * (2 +(3,4)-0,1)) | f(2 * (~ 5)) & f(2 * (0,1))^m(0,1);
int i[] = { 1, 23, 4, 5, };
char c[2][6] = { "hello", "" };
"#,
    );
}

/// [cpp.scope]/6
#[test]
fn standard_example_2() {
    common::check(
        r#"#define str(s) # s
#define xstr(s) str(s)
#define debug(s, t) printf("x" # s "= %d, x" # t "= %s", \
 x ## s, x ## t)
#define INCFILE(n) vers ## n
#define glue(a, b) a ## b
#define xglue(a, b) glue(a, b)
#define HIGHLOW "hello"
#define LOW LOW ", world"
debug(1, 2);
fputs(str(strncmp("abc\0d", "abc", '\4') // this goes away
 == 0) str(: @\n), s);
xstr(INCFILE(2).h)
glue(HIGH, LOW);
xglue(HIGH, LOW)
"#,
        r#"printf("x" "1" "= %d, x" "2" "= %s", x1, x2);
fputs("strncmp(\"abc\\0d\", \"abc\", '\\4') == 0" ": @\n", s);
"vers2.h"
"hello";
"hello" ", world"
"#,
    );
}

/// [cpp.scope]/8
#[test]
fn standard_example_4() {
    common::check(
        r#"#define t(x,y,z) x ## y ## z
int j[] = { t(1,2,3), t(,4,5), t(6,,7), t(8,9,),
t(10,,), t(,11,), t(,,12), t(,,) };
"#,
        r#"
int j[] = { 123, 45, 67, 89,
10, 11, 12, };
"#,
    );
}

/// [cpp.scope]/9
#[test]
fn standard_example_5() {
    common::check_diags(
        r#"#define OBJ_LIKE (1-1)
#define OBJ_LIKE /* white space */ (1-1) /* other */
#define FUNC_LIKE(a) ( a )
#define FUNC_LIKE( a )( /* note the white space */ \
a /* other stuff on this line
*/ )
#define OBJ_LIKE (0)
#define OBJ_LIKE (1 - 1)
#define FUNC_LIKE(b) ( a )
#define FUNC_LIKE(b) ( b )
"#,
        r#"
"#,
        &[r#"7:9: warning: 'OBJ_LIKE' macro redefined"#, r#"8:9: warning: 'OBJ_LIKE' macro redefined"#, r#"9:9: warning: 'FUNC_LIKE' macro redefined"#, r#"10:9: warning: 'FUNC_LIKE' macro redefined"#],
    );
}

/// [cpp.scope]/10
#[test]
fn standard_example_6() {
    common::check(
        r#"#define debug(...) fprintf(stderr, __VA_ARGS__)
#define showlist(...) puts(#__VA_ARGS__)
#define report(test, ...) ((test)?puts(#test):\
 printf(__VA_ARGS__))
debug("Flag");
debug("X = %d\n", x);
showlist(The first, second, and third items.);
report(x>y, "x is %d but y is %d", x, y);
"#,
        r#"fprintf(stderr, "Flag");
fprintf(stderr, "X = %d\n", x);
puts("The first, second, and third items.");
((x>y)?puts("x>y"): printf("x is %d but y is %d", x, y));
"#,
    );
}
