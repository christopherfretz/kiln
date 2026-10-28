//! [expr.const]/5: one test per core-constant-expression bullet reachable
//! in M2, with clang 19's exact notes, plus the positives that bound each
//! bullet. Bullet numbers are those of N4950 (C++23); every golden was
//! checked against `clang++-19 -std=c++2c -fsyntax-only
//! --target=x86_64-unknown-linux-gnu`. Golden lines are `line:col: text`
//! for the source exactly as written (line 1 starts right after the opening
//! quote).

mod common;
use common::*;

/// [expr.const]/5.2: control flow through the definition of a block variable with static storage duration that is not usable in constant expressions.
#[test]
fn expr_const_5_2_static_local() {
    fails_exactly(
        r"constexpr int f() {
  static int x = 1;
  return x;
}
static_assert(f() == 1);
",
        r"
5:15: static assertion expression is not an integral constant expression
2:14: control flows through the definition of a static variable
5:15: in call to 'f()'
",
    );
}

/// [expr.const]/5.3: an invocation of a non-constexpr function.
#[test]
fn expr_const_5_3_non_constexpr_function() {
    fails_exactly(
        r"int nc() { return 1; }
constexpr int f() { return nc(); }
static_assert(f() == 1);
",
        r"
3:15: static assertion expression is not an integral constant expression
2:28: non-constexpr function 'nc' cannot be used in a constant expression
3:15: in call to 'f()'
1:5: declared here
",
    );
}

/// [expr.const]/5.3: the non-constexpr call at the top level has no call frame.
#[test]
fn expr_const_5_3_non_constexpr_direct() {
    fails_exactly(
        r"int nc() { return 1; }
static_assert(nc() == 1);
",
        r"
2:15: static assertion expression is not an integral constant expression
2:15: non-constexpr function 'nc' cannot be used in a constant expression
1:5: declared here
",
    );
}

/// [expr.const]/5.4: an invocation of a constexpr function that is declared but not defined.
#[test]
fn expr_const_5_4_undefined_function() {
    fails_exactly(
        r"constexpr int u();
constexpr int f() { return u(); }
static_assert(f() == 1);
",
        r"
3:15: static assertion expression is not an integral constant expression
2:28: undefined function 'u' cannot be used in a constant expression
3:15: in call to 'f()'
1:15: declared here
",
    );
}

/// [expr.const]/5.7, [implimits]: recursion past the implementation's depth limit (512, as clang).
#[test]
fn expr_const_5_7_depth_limit() {
    fails_exactly(
        r"constexpr int f(int n) { return n == 0 ? 0 : f(n - 1); }
static_assert(f(1000) == 0);
",
        r"
2:15: static assertion expression is not an integral constant expression
1:46: constexpr evaluation exceeded maximum depth of 512 calls
1:46: in call to 'f(489)'
1:46: in call to 'f(490)'
1:46: in call to 'f(491)'
1:46: in call to 'f(492)'
1:46: in call to 'f(493)'
1:46: (skipping 502 calls in backtrace; use -fconstexpr-backtrace-limit=0 to see all)
1:46: in call to 'f(996)'
1:46: in call to 'f(997)'
1:46: in call to 'f(998)'
1:46: in call to 'f(999)'
2:15: in call to 'f(1000)'
",
    );
}

/// [expr.const]/5.7, [implimits]: an evaluation past the step limit (1048576, as clang).
#[test]
fn expr_const_5_7_step_limit() {
    fails_exactly(
        r"constexpr int f() {
  int i = 0;
  while (true)
    ++i;
  return i;
}
static_assert(f() == 0);
",
        r"
7:15: static assertion expression is not an integral constant expression
4:5: constexpr evaluation hit maximum step limit; possible infinite loop?
7:15: in call to 'f()'
",
    );
}

/// [expr.const]/5.8: an operation with undefined behavior (see tests/ub.rs for the catalogue).
#[test]
fn expr_const_5_8_undefined_behavior() {
    fails_exactly(
        r"constexpr int f(int a) { return a << 40; }
static_assert(f(1) == 0);
",
        r"
2:15: static assertion expression is not an integral constant expression
1:35: shift count 40 >= width of type 'int' (32 bits)
2:15: in call to 'f(1)'
",
    );
}

/// [expr.const]/5.9: L2R of a glvalue whose object is not usable in constant expressions and began its lifetime outside the evaluation.
#[test]
fn expr_const_5_9_non_const_global() {
    fails_exactly(
        r"int g = 1;
constexpr int f() { return g; }
static_assert(f() == 1);
",
        r"
3:15: static assertion expression is not an integral constant expression
2:28: read of non-const variable 'g' is not allowed in a constant expression
3:15: in call to 'f()'
1:5: declared here
",
    );
}

/// [expr.const]/5.9, [expr.const]/4: a const variable whose initializer is not a constant expression.
#[test]
fn expr_const_5_9_non_constant_initializer() {
    fails_exactly(
        r"int h() { return 1; }
const int k = h();
static_assert(k == 1);
",
        r"
3:15: static assertion expression is not an integral constant expression
3:15: initializer of 'k' is not a constant expression
2:11: declared here
",
    );
}

/// [expr.const]/5.9, [dcl.constexpr]/6: a constexpr variable's initializer must be a constant expression.
#[test]
fn expr_const_5_9_constexpr_init() {
    fails_exactly(
        r"int g = 1;
constexpr int x = g + 1;
",
        r"
2:15: constexpr variable must be initialized by a constant expression
2:19: read of non-const variable 'g' is not allowed in a constant expression
1:5: declared here
",
    );
}

/// [expr.const]/5.9: L2R of a volatile glvalue.
#[test]
fn expr_const_5_9_volatile() {
    fails_exactly(
        r"constexpr int f() {
  volatile int v = 1;
  return v;
}
static_assert(f() == 1);
",
        r"
5:15: static assertion expression is not an integral constant expression
3:10: read of volatile-qualified type 'volatile int' is not allowed in a constant expression
5:15: in call to 'f()'
",
    );
}

/// [expr.const]/5.10: L2R of a non-active union member.
#[test]
fn expr_const_5_10_inactive_member() {
    fails_exactly(
        r"union U { int i; char c; };
constexpr char f() {
  U u = {65};
  return u.c;
}
static_assert(f() == 'A');
",
        r"
6:15: static assertion expression is not an integral constant expression
4:10: read of member 'c' of union with active member 'i' is not allowed in a constant expression
6:15: in call to 'f()'
",
    );
}

/// [expr.const]/5.11: L2R of an object with an indeterminate value.
#[test]
fn expr_const_5_11_indeterminate() {
    fails_exactly(
        r"constexpr int f(bool b) {
  int x;
  if (b)
    x = 1;
  return x;
}
static_assert(f(false) == 0);
",
        r"
7:15: static assertion expression is not an integral constant expression
5:10: read of uninitialized object is not allowed in a constant expression
7:15: in call to 'f(false)'
",
    );
}

/// [expr.const]/5.14: a conversion from `cv void*` to a pointer to a type not similar to the object's.
#[test]
fn expr_const_5_14_void_ptr() {
    fails_exactly(
        r"constexpr int f() {
  int x = 1;
  void *v = &x;
  return *static_cast<char *>(v);
}
static_assert(f() == 1);
",
        r"
6:15: static assertion expression is not an integral constant expression
4:11: cast from 'void *' is not allowed in a constant expression because the pointed object type 'int' is not similar to the target type 'char'
6:15: in call to 'f()'
",
    );
}

/// [expr.const]/5.15: a `reinterpret_cast`.
#[test]
fn expr_const_5_15_reinterpret_cast() {
    fails_exactly(
        r"constexpr int f() {
  int x = 1;
  return *reinterpret_cast<int *>(&x);
}
static_assert(f() == 1);
",
        r"
5:15: static assertion expression is not an integral constant expression
3:11: reinterpret_cast is not allowed in a constant expression
5:15: in call to 'f()'
",
    );
}

/// [expr.const]/5.16: modification of an object whose lifetime began outside the evaluation.
#[test]
fn expr_const_5_16_modify_outside() {
    fails_exactly(
        r"int g = 0;
constexpr int f() {
  ++g;
  return g;
}
static_assert(f() == 1);
",
        r"
6:15: static assertion expression is not an integral constant expression
3:3: a constant expression cannot modify an object that is visible outside that expression
6:15: in call to 'f()'
",
    );
}

/// [expr.const]/5.16, [dcl.type.cv]/4: modification of a const object created by the evaluation.
#[test]
fn expr_const_5_16_modify_const() {
    fails_exactly(
        r"struct S { int a; };
constexpr int f() {
  const S s = {1};
  ((S *)&s)->a = 2;
  return s.a;
}
static_assert(f() == 2);
",
        r"
7:15: static assertion expression is not an integral constant expression
4:16: modification of object of const-qualified type 'const int' is not allowed in a constant expression
7:15: in call to 'f()'
",
    );
}

/// [expr.const]/5.24, [expr.rel]/4: a relational comparison of pointers to unrelated objects.
#[test]
fn expr_const_5_24_relational_unrelated() {
    fails_exactly(
        r"constexpr int a = 1, b = 2;
static_assert(&a < &b);
",
        r"
2:15: static assertion expression is not an integral constant expression
2:18: comparison between '&a' and '&b' has unspecified value
",
    );
}

/// [expr.const]/5.24, [expr.eq]/3.1: equality of a past-the-end pointer and a pointer to another object.
#[test]
fn expr_const_5_24_past_end_equality() {
    fails_exactly(
        r"constexpr int a[1] = {1};
constexpr int b = 2;
static_assert(a + 1 != &b);
",
        r"
3:15: static assertion expression is not an integral constant expression
3:21: comparison against pointer '&a[1]' that points past the end of a complete object has unspecified value
",
    );
}

/// [expr.const]/5.24, [lex.string]/9: two literal evaluations may or may not share an object.
#[test]
fn expr_const_5_24_string_literals() {
    fails_exactly(
        r#"constexpr bool f() { return "x" != "x"; }
static_assert(f());
"#,
        r"
2:15: static assertion expression is not an integral constant expression
1:33: comparison of addresses of literals has unspecified value
2:15: in call to 'f()'
",
    );
}

/// [expr.const]/5.15: a `reinterpret_cast` to an unrelated pointer type.
#[test]
fn expr_const_5_15_reinterpret_cast_type_changing() {
    fails_exactly(
        r"constexpr int f() {
  int x = 1;
  return *reinterpret_cast<char *>(&x);
}
static_assert(f() == 1);
",
        r"
5:15: static assertion expression is not an integral constant expression
3:11: reinterpret_cast is not allowed in a constant expression
5:15: in call to 'f()'
",
    );
}

/// [expr.const]/5.16: compound assignment to a global.
#[test]
fn expr_const_5_16_compound_assign_global() {
    fails_exactly(
        r"int g = 0;
constexpr int f() {
  g += 1;
  return 0;
}
static_assert(f() == 0);
",
        r"
6:15: static assertion expression is not an integral constant expression
3:5: a constant expression cannot modify an object that is visible outside that expression
6:15: in call to 'f()'
",
    );
}

/// [expr.const]/5.16: assignment to a global.
#[test]
fn expr_const_5_16_assign_global() {
    fails_exactly(
        r"int g = 0;
constexpr int f() {
  g = 1;
  return 0;
}
static_assert(f() == 0);
",
        r"
6:15: static assertion expression is not an integral constant expression
3:5: a constant expression cannot modify an object that is visible outside that expression
6:15: in call to 'f()'
",
    );
}

/// [expr.const]/5.16: decrement of a bit-field of a global.
#[test]
fn expr_const_5_16_decrement_bitfield_global() {
    fails_exactly(
        r"struct S { int a : 3; };
S s = {1};
constexpr int f() {
  s.a--;
  return 0;
}
static_assert(f() == 0);
",
        r"
7:15: static assertion expression is not an integral constant expression
4:6: a constant expression cannot modify an object that is visible outside that expression
7:15: in call to 'f()'
",
    );
}

/// [expr.const]/5.2: a static variable usable in constant expressions may be defined (C++23).
#[test]
fn expr_const_5_2_static_constexpr_local_ok() {
    ok(r"
constexpr int f() {
  static constexpr int x = 4;
  return x;
}
static_assert(f() == 4);
");
}

/// [expr.const]/4, [expr.const]/5.9: a const integral variable with a constant initializer is usable in constant expressions.
#[test]
fn expr_const_4_const_integral_usable_ok() {
    ok(r"
const int k = 3;
const int a[2] = {1, 2};
constexpr int x = 5;
static_assert(k + x == 8);
");
}

/// [expr.const]/5.9: an object whose lifetime began within the evaluation may be read.
#[test]
fn expr_const_5_9_lifetime_within_ok() {
    ok(r"
constexpr int f() {
  int x = 1;
  int *p = &x;
  return *p;
}
static_assert(f() == 1);
");
}

/// [expr.const]/5.10, [class.union]/6: assignment changes the active member.
#[test]
fn expr_const_5_10_active_member_change_ok() {
    ok(r"
union U { int i; float f; };
constexpr float f() {
  U u = {1};
  u.f = 2.5f;
  return u.f;
}
static_assert(f() == 2.5f);
");
}

/// [expr.const]/5.16: objects whose lifetime began within the evaluation may be modified.
#[test]
fn expr_const_5_16_modify_within_ok() {
    ok(r"
struct S { int a; int b[2]; };
constexpr int f() {
  S s = {1, {2, 3}};
  s.a += 10;
  ++s.b[1];
  s.b[0] = s.a;
  return s.a + s.b[0] + s.b[1];
}
static_assert(f() == 26);
");
}

/// [expr.const]/5.24, [expr.eq]/3, [expr.rel]/4: comparisons with specified results.
#[test]
fn expr_const_5_24_specified_comparisons_ok() {
    ok(r"
constexpr int x = 1, y = 2;
constexpr int a[3] = {1, 2, 3};
struct S { int m; int n; };
constexpr S s = {1, 2};
static_assert(&x != &y);
static_assert(&x != nullptr);
static_assert(a < a + 2 && a + 3 > a + 1);
static_assert(&s.m < &s.n);
static_assert(a + 3 == a + 3);
");
}

/// [expr.const]/5.14: a `void*` conversion back to the object's type is allowed (C++26).
#[test]
fn expr_const_5_14_void_ptr_round_trip_ok() {
    ok(r"
constexpr int f() {
  int x = 7;
  void *v = &x;
  return *static_cast<int *>(v);
}
static_assert(f() == 7);
");
}

/// [lex.string]/9: a pointer frozen into a `constexpr` variable keeps the
/// identity of the literal evaluation that created it, so pointers into
/// two equal literals compare unspecified (not equal through a shared
/// object), and a pointer compares equal to itself.
#[test]
fn expr_const_5_24_frozen_string_literals() {
    ok(r#"constexpr const char *p = "abc";
constexpr const char *r = p;
static_assert(p == r);
static_assert(p + 1 == r + 1);
static_assert(*(p + 1) == 'b');
"#);
    fails_exactly(
        r#"constexpr const char *p = "abc";
constexpr const char *q = "abc";
static_assert(p + 1 != q);
"#,
        r"
3:15: static assertion expression is not an integral constant expression
3:21: comparison of addresses of literals has unspecified value
",
    );
}
