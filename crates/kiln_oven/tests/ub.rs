//! The undefined-behaviour catalogue ([expr.const]/5.8, [defns.undefined]):
//! every core-UB operation reachable in M2 is diagnosed with clang 19's
//! exact notes. Each golden was checked against `clang++-19 -std=c++2c
//! -fsyntax-only`; deviations are called out on the test. Golden lines are
//! `line:col: text` for the source exactly as written (line 1 starts right
//! after the opening quote). Bullet numbers of [expr.const]/5 are those of
//! N4950 (C++23).

mod common;
use common::*;

/// [expr.const]/5.8, [basic.compound]/3: reading through a one-past-the-end pointer.
#[test]
fn ub_oob_read_past_end() {
    fails_exactly(
        r"constexpr int f() {
  int a[3] = {1, 2, 3};
  int *p = a + 3;
  return *p;
}
static_assert(f() == 0);
",
        r"
6:15: static assertion expression is not an integral constant expression
4:10: read of dereferenced one-past-the-end pointer is not allowed in a constant expression
6:15: in call to 'f()'
",
    );
}

/// [expr.add]/4.3: forming a pointer two past the end is undefined.
#[test]
fn ub_oob_subscript() {
    fails_exactly(
        r"constexpr int f(int i) {
  int a[3] = {1, 2, 3};
  return a[i];
}
static_assert(f(4) == 0);
",
        r"
5:15: static assertion expression is not an integral constant expression
3:10: cannot refer to element 4 of array of 3 elements in a constant expression
5:15: in call to 'f(4)'
",
    );
}

/// [expr.add]/4.3: a negative index leaves the array.
#[test]
fn ub_oob_negative_index() {
    fails_exactly(
        r"constexpr int f(int i) {
  int a[3] = {1, 2, 3};
  return *(a + i);
}
static_assert(f(-1) == 0);
",
        r"
5:15: static assertion expression is not an integral constant expression
3:14: cannot refer to element -1 of array of 3 elements in a constant expression
5:15: in call to 'f(-1)'
",
    );
}

/// [expr.add]/4 with [basic.compound]/3: a non-array object is an array of one element.
#[test]
fn ub_oob_non_array() {
    fails_exactly(
        r"constexpr int f() {
  int x = 1;
  int *p = &x + 2;
  return 0;
}
static_assert(f() == 0);
",
        r"
6:15: static assertion expression is not an integral constant expression
3:15: cannot refer to element 2 of non-array object in a constant expression
6:15: in call to 'f()'
",
    );
}

/// [expr.const]/5.11, [basic.indet]/2: lvalue-to-rvalue conversion of an indeterminate value.
#[test]
fn ub_uninit_read() {
    fails_exactly(
        r"constexpr int f() {
  int x;
  return x;
}
static_assert(f() == 0);
",
        r"
5:15: static assertion expression is not an integral constant expression
3:10: read of uninitialized object is not allowed in a constant expression
5:15: in call to 'f()'
",
    );
}

/// [basic.indet]/2: incrementing an indeterminate value reads it.
#[test]
fn ub_uninit_increment() {
    fails_exactly(
        r"constexpr int f() {
  int x;
  x++;
  return x;
}
static_assert(f() == 0);
",
        r"
6:15: static assertion expression is not an integral constant expression
3:4: increment of uninitialized object is not allowed in a constant expression
6:15: in call to 'f()'
",
    );
}

/// [dcl.init.general]/7, [expr.const]/5.11: an uninitialized member read.
#[test]
fn ub_uninit_member() {
    fails_exactly(
        r"struct S { int a; int b; };
constexpr int f() {
  S s;
  s.a = 1;
  return s.b;
}
static_assert(f() == 0);
",
        r"
7:15: static assertion expression is not an integral constant expression
5:10: read of uninitialized object is not allowed in a constant expression
7:15: in call to 'f()'
",
    );
}

/// [basic.life]/7, [expr.const]/5.8: reading a variable after its lifetime ended.
#[test]
fn ub_lifetime_ended() {
    fails_exactly(
        r"constexpr int f() {
  int *p = nullptr;
  {
    int x = 1;
    p = &x;
  }
  return *p;
}
static_assert(f() == 1);
",
        r"
9:15: static assertion expression is not an integral constant expression
7:10: read of object outside its lifetime is not allowed in a constant expression
9:15: in call to 'f()'
",
    );
}

/// [basic.life]/7: a dangling pointer to a callee's local.
#[test]
fn ub_lifetime_returned_local() {
    fails_exactly(
        r"constexpr int *g() {
  int x = 1;
  return &x;
}
constexpr int f() { return *g(); }
static_assert(f() == 1);
",
        r"
6:15: static assertion expression is not an integral constant expression
5:28: read of variable whose lifetime has ended
6:15: in call to 'f()'
2:7: declared here
",
    );
}

/// [expr.pre]/4: signed int overflow.
#[test]
fn ub_overflow_int_add() {
    fails_exactly(
        r"constexpr int f(int a) { return a + 1; }
static_assert(f(2147483647) == 0);
",
        r"
2:15: static assertion expression is not an integral constant expression
1:35: value 2147483648 is outside the range of representable values of type 'int'
2:15: in call to 'f(2147483647)'
",
    );
}

/// [expr.pre]/4: signed int overflow in multiplication.
#[test]
fn ub_overflow_int_mul() {
    fails_exactly(
        r"constexpr int f(int a) { return a * a; }
static_assert(f(65536) == 0);
",
        r"
2:15: static assertion expression is not an integral constant expression
1:35: value 4294967296 is outside the range of representable values of type 'int'
2:15: in call to 'f(65536)'
",
    );
}

/// [expr.pre]/4: signed long overflow.
#[test]
fn ub_overflow_long_sub() {
    fails_exactly(
        r"constexpr long f(long a) { return a - 1; }
static_assert(f(-9223372036854775807L - 1) == 0);
",
        r"
2:15: static assertion expression is not an integral constant expression
1:37: value -9223372036854775809 is outside the range of representable values of type 'long'
2:15: in call to 'f(-9223372036854775808)'
",
    );
}

/// [expr.pre]/4: signed long long overflow.
#[test]
fn ub_overflow_long_long_mul() {
    fails_exactly(
        r"constexpr long long f(long long a) { return a * 3; }
static_assert(f(4611686018427387904LL) == 0);
",
        r"
2:15: static assertion expression is not an integral constant expression
1:47: value 13835058055282163712 is outside the range of representable values of type 'long long'
2:15: in call to 'f(4611686018427387904)'
",
    );
}

/// [expr.pre]/4: signed __int128 overflow.
#[test]
fn ub_overflow_int128() {
    fails_exactly(
        r"constexpr __int128 f(__int128 a) { return a + a; }
static_assert(f((__int128)1 << 126) == 0);
",
        r"
2:15: static assertion expression is not an integral constant expression
1:45: value 170141183460469231731687303715884105728 is outside the range of representable values of type '__int128'
2:15: in call to 'f(85070591730234615865843651857942052864)'
",
    );
}

/// [expr.pre]/4: negating `INT_MIN`.
#[test]
fn ub_overflow_negate() {
    fails_exactly(
        r"constexpr int f(int a) { return -a; }
static_assert(f(-2147483647 - 1) == 0);
",
        r"
2:15: static assertion expression is not an integral constant expression
1:33: value 2147483648 is outside the range of representable values of type 'int'
2:15: in call to 'f(-2147483648)'
",
    );
}

/// [expr.mul]/4: `INT_MIN` / -1 is not representable.
#[test]
fn ub_overflow_div() {
    fails_exactly(
        r"constexpr int f(int a, int b) { return a / b; }
static_assert(f(-2147483647 - 1, -1) == 0);
",
        r"
2:15: static assertion expression is not an integral constant expression
1:42: value 2147483648 is outside the range of representable values of type 'int'
2:15: in call to 'f(-2147483648, -1)'
",
    );
}

/// [expr.mul]/4: `INT_MIN` % -1 is undefined because the quotient is not representable.
#[test]
fn ub_overflow_rem() {
    fails_exactly(
        r"constexpr int f(int a, int b) { return a % b; }
static_assert(f(-2147483647 - 1, -1) == 0);
",
        r"
2:15: static assertion expression is not an integral constant expression
1:42: value 2147483648 is outside the range of representable values of type 'int'
2:15: in call to 'f(-2147483648, -1)'
",
    );
}

/// [expr.assign]/6: E1 op= E2 overflows like E1 = E1 op E2.
#[test]
fn ub_overflow_compound() {
    fails_exactly(
        r"constexpr int f() {
  int x = 2147483647;
  x += 1;
  return x;
}
static_assert(f() == 0);
",
        r"
6:15: static assertion expression is not an integral constant expression
3:5: value 2147483648 is outside the range of representable values of type 'int'
6:15: in call to 'f()'
",
    );
}

/// [expr.pre.incr]/1: ++ on `INT_MAX` overflows.
#[test]
fn ub_overflow_increment() {
    fails_exactly(
        r"constexpr int f() {
  int x = 2147483647;
  ++x;
  return x;
}
static_assert(f() == 0);
",
        r"
6:15: static assertion expression is not an integral constant expression
3:3: value 2147483648 is outside the range of representable values of type 'int'
6:15: in call to 'f()'
",
    );
}

/// [expr.shift]/1: shift count >= width.
#[test]
fn ub_shift_too_wide() {
    fails_exactly(
        r"constexpr int f(int n) { return 1 << n; }
static_assert(f(32) == 0);
",
        r"
2:15: static assertion expression is not an integral constant expression
1:35: shift count 32 >= width of type 'int' (32 bits)
2:15: in call to 'f(32)'
",
    );
}

/// [expr.shift]/1: negative shift count.
#[test]
fn ub_shift_negative() {
    fails_exactly(
        r"constexpr int f(int n) { return 1 >> n; }
static_assert(f(-1) == 0);
",
        r"
2:15: static assertion expression is not an integral constant expression
1:35: negative shift count -1
2:15: in call to 'f(-1)'
",
    );
}

/// [expr.shift]/1: shift count >= width of long.
#[test]
fn ub_shift_long_too_wide() {
    fails_exactly(
        r"constexpr long f(int n) { return 1L << n; }
static_assert(f(64) == 0);
",
        r"
2:15: static assertion expression is not an integral constant expression
1:37: shift count 64 >= width of type 'long' (64 bits)
2:15: in call to 'f(64)'
",
    );
}

/// [expr.mul]/4: integer division by zero.
#[test]
fn ub_div_zero() {
    fails_exactly(
        r"constexpr int f(int a, int b) { return a / b; }
static_assert(f(1, 0) == 0);
",
        r"
2:15: static assertion expression is not an integral constant expression
1:42: division by zero
2:15: in call to 'f(1, 0)'
",
    );
}

/// [expr.mul]/4: integer remainder by zero.
#[test]
fn ub_rem_zero() {
    fails_exactly(
        r"constexpr unsigned f(unsigned a, unsigned b) { return a % b; }
static_assert(f(1u, 0u) == 0);
",
        r"
2:15: static assertion expression is not an integral constant expression
1:57: division by zero
2:15: in call to 'f(1, 0)'
",
    );
}

/// [expr.mul]/4: floating division by zero.
#[test]
fn ub_float_div_zero() {
    fails_exactly(
        r"constexpr double f(double a, double b) { return a / b; }
static_assert(f(1.0, 0.0) == 0);
",
        r"
2:15: static assertion expression is not an integral constant expression
1:51: division by zero
2:15: in call to 'f(1.000000e+00, 0.000000e+00)'
",
    );
}

/// [conv.fpint]/1: a truncated value that does not fit is undefined.
#[test]
fn ub_float_to_int_range() {
    fails_exactly(
        r"constexpr int f(double d) { return (int)d; }
static_assert(f(1e10) == 0);
",
        r"
2:15: static assertion expression is not an integral constant expression
1:41: value 1.0E+10 is outside the range of representable values of type 'int'
2:15: in call to 'f(1.000000e+10)'
",
    );
}

/// [expr.unary.op]/1, [expr.const]/5.8: indirection through a null pointer.
#[test]
fn ub_null_deref() {
    fails_exactly(
        r"constexpr int f(int *p) { return *p; }
static_assert(f(nullptr) == 0);
",
        r"
2:15: static assertion expression is not an integral constant expression
1:34: read of dereferenced null pointer is not allowed in a constant expression
2:15: in call to 'f(nullptr)'
",
    );
}

/// [expr.ref]/1: member access through a null pointer.
#[test]
fn ub_null_arrow() {
    fails_exactly(
        r"struct S { int a; };
constexpr int f(S *p) { return p->a; }
static_assert(f(nullptr) == 0);
",
        r"
3:15: static assertion expression is not an integral constant expression
2:35: cannot access field of null pointer
3:15: in call to 'f(nullptr)'
",
    );
}

/// [expr.add]/4.1: only 0 may be added to a null pointer.
#[test]
fn ub_null_arith() {
    fails_exactly(
        r"constexpr bool f(int *p) { return p + 1 != nullptr; }
static_assert(f(nullptr));
",
        r"
2:15: static assertion expression is not an integral constant expression
1:37: cannot perform pointer arithmetic on null pointer
2:15: in call to 'f(nullptr)'
",
    );
}

/// [expr.call]/1: calling a null function pointer.
#[test]
fn ub_null_fn_ptr() {
    fails_exactly(
        r"constexpr int f() {
  int (*fp)(int) = nullptr;
  return fp(1);
}
static_assert(f() == 0);
",
        r"
5:15: static assertion expression is not an integral constant expression
3:10: 'fp' evaluates to a null function pointer
5:15: in call to 'f()'
",
    );
}

/// [expr.rel]/4.3, [expr.const]/5.24: relational comparison of unrelated objects is unspecified.
#[test]
fn ub_cmp_cross_alloc() {
    fails_exactly(
        r"constexpr bool f() {
  int a = 1;
  int b = 2;
  return &a < &b;
}
static_assert(f());
",
        r"
6:15: static assertion expression is not an integral constant expression
4:13: comparison between '&a' and '&b' has unspecified value
6:15: in call to 'f()'
",
    );
}

/// [expr.eq]/3.1, [expr.const]/5.24: equality with a past-the-end pointer of another object is unspecified.
#[test]
fn ub_cmp_past_end() {
    fails_exactly(
        r"constexpr bool f() {
  int a[2] = {1, 2};
  int b = 3;
  return &a[2] == &b;
}
static_assert(!f());
",
        r"
6:15: static assertion expression is not an integral constant expression
4:16: comparison against pointer '&a[2]' that points past the end of a complete object has unspecified value
6:16: in call to 'f()'
",
    );
}

/// [lex.string]/9, [expr.const]/5.24: distinct literal evaluations need not be distinct objects.
#[test]
fn ub_cmp_string_literals() {
    fails_exactly(
        r#"static_assert("a" == "a");
"#,
        r"
1:15: static assertion expression is not an integral constant expression
1:19: comparison of addresses of literals has unspecified value
",
    );
}

/// [expr.add]/5.3: subtracting pointers into different arrays.
#[test]
fn ub_ptr_sub_cross_array() {
    fails_exactly(
        r"constexpr long f() {
  int a[2] = {1, 2};
  int b[2] = {3, 4};
  return &b[1] - &a[0];
}
static_assert(f() == 0);
",
        r"
6:15: static assertion expression is not an integral constant expression
4:16: subexpression not valid in a constant expression
6:15: in call to 'f()'
",
    );
}

/// [class.union]/2, [expr.const]/5.10: reading an inactive union member (type punning).
#[test]
fn ub_union_inactive() {
    fails_exactly(
        r"union U { int i; float f; };
constexpr float f() {
  U u = {1};
  return u.f;
}
static_assert(f() == 0);
",
        r"
6:15: static assertion expression is not an integral constant expression
4:10: read of member 'f' of union with active member 'i' is not allowed in a constant expression
6:15: in call to 'f()'
",
    );
}

/// [expr.const]/5.15: a `reinterpret_cast` (type punning through a pointer).
#[test]
fn ub_pun_reinterpret() {
    fails_exactly(
        r"constexpr long f() {
  double d = 1.0;
  return *(long *)&d;
}
static_assert(f() == 0);
",
        r"
5:15: static assertion expression is not an integral constant expression
3:11: cast that performs the conversions of a reinterpret_cast is not allowed in a constant expression
5:15: in call to 'f()'
",
    );
}

/// [expr.const]/5.14: a cast from void* to a dissimilar object type.
#[test]
fn ub_pun_void_cast() {
    fails_exactly(
        r"constexpr long f() {
  double d = 1.0;
  void *v = &d;
  return *static_cast<long *>(v);
}
static_assert(f() == 0);
",
        r"
6:15: static assertion expression is not an integral constant expression
4:11: cast from 'void *' is not allowed in a constant expression because the pointed object type 'double' is not similar to the target type 'long'
6:15: in call to 'f()'
",
    );
}

/// [dcl.type.cv]/4: modifying a const object.
#[test]
fn ub_modify_const_local() {
    fails_exactly(
        r"constexpr int f() {
  const int c = 1;
  *(int *)&c = 2;
  return c;
}
static_assert(f() == 2);
",
        r"
6:15: static assertion expression is not an integral constant expression
3:14: modification of object of const-qualified type 'const int' is not allowed in a constant expression
6:15: in call to 'f()'
",
    );
}

/// [expr.const]/5.16: modifying an object whose lifetime began outside the evaluation.
#[test]
fn ub_modify_constexpr_global() {
    fails_exactly(
        r"constexpr int g = 1;
constexpr int f() {
  *(int *)&g = 2;
  return g;
}
static_assert(f() == 2);
",
        r"
6:15: static assertion expression is not an integral constant expression
3:14: a constant expression cannot modify an object that is visible outside that expression
6:15: in call to 'f()'
",
    );
}

/// [expr.const]/5.16: modifying a non-const global.
#[test]
fn ub_modify_global() {
    fails_exactly(
        r"int g = 1;
constexpr int f() {
  g = 2;
  return 2;
}
static_assert(f() == 2);
",
        r"
6:15: static assertion expression is not an integral constant expression
3:5: a constant expression cannot modify an object that is visible outside that expression
6:15: in call to 'f()'
",
    );
}

/// [expr.const]/5.9: reading a non-const variable.
#[test]
fn ub_read_non_const() {
    fails_exactly(
        r"int g = 1;
static_assert(g == 1);
",
        r"
2:15: static assertion expression is not an integral constant expression
2:15: read of non-const variable 'g' is not allowed in a constant expression
1:5: declared here
",
    );
}

/// [expr.const]/5.9, [expr.const]/4: a const double is not usable in constant expressions.
#[test]
fn ub_read_non_constexpr_double() {
    fails_exactly(
        r"const double dd = 1.0;
static_assert(dd == 1.0);
",
        r"
2:15: static assertion expression is not an integral constant expression
2:15: read of non-constexpr variable 'dd' is not allowed in a constant expression
1:14: declared here
",
    );
}

/// [expr.const]/4: a const int without a preceding initializer is not usable.
#[test]
fn ub_read_extern_const() {
    fails_exactly(
        r"extern const int e;
static_assert(e == 1);
",
        r"
2:15: static assertion expression is not an integral constant expression
2:15: initializer of 'e' is unknown
1:18: declared here
",
    );
}

/// [expr.static.cast]/9, [expr.const]/5.8: an unscoped enum value outside its range.
/// clang 19 behaviour (`-Wenum-constexpr-conversion`): the cast is not a
/// failure of the evaluation; where a constant expression proper is
/// required (a `constexpr` variable's initializer, a converted constant
/// expression) it is reported as an error diagnostic and the value is
/// kept; integral constant expressions (`static_assert`) do not check it.
#[test]
fn ub_enum_range() {
    let src = r"enum E { A, B };
constexpr E f(int v) { return (E)v; }
static_assert(f(5) == 5);
";
    let tu = ok(src);
    assert!(tu.diags.is_empty(), "{:?}", tu.diags);
    let src = r"enum E { A, B };
constexpr E f(int v) { return (E)v; }
constexpr E e = f(5);
";
    let tu = ok(src);
    assert_eq!(tu.diags.len(), 1, "{:?}", tu.diags);
    let d = &tu.diags[0];
    assert!(d.contains("oven_enum_constexpr_conversion"), "{d}");
    assert!(d.contains(r#"args: ["5", "0", "1", "E"]"#), "{d}");
    // Memoization does not hide it: an unchecked evaluation of `f(5)` is
    // not cached for a checked one, and clang reports every checked
    // evaluation (tests/conformance/expr.const/5.8/enum-range.cpp).
    let src = r"enum E { A, B };
constexpr E f(int v) { return (E)v; }
static_assert(f(5) == 5);
constexpr E e = f(5);
constexpr E e2 = f(5);
";
    let tu = ok(src);
    assert_eq!(tu.diags.len(), 2, "{:?}", tu.diags);
}

/// [expr.unary.op]/1: `(*p).m` with p null reads through the null pointer.
#[test]
fn ub_null_dot_deref() {
    fails_exactly(
        r"struct S { int a; int b; };
constexpr int f(S *p) { return (*p).b; }
static_assert(f(nullptr) == 0);
",
        r"
3:15: static assertion expression is not an integral constant expression
2:32: read of dereferenced null pointer is not allowed in a constant expression
3:15: in call to 'f(nullptr)'
",
    );
}

/// [expr.add]/4.1: `p[0]` on null is allowed, the member read is not.
#[test]
fn ub_null_subscript_member() {
    fails_exactly(
        r"struct S { int a; int b; };
constexpr int f(S *p) { return p[0].b; }
static_assert(f(nullptr) == 0);
",
        r"
3:15: static assertion expression is not an integral constant expression
2:32: read of dereferenced null pointer is not allowed in a constant expression
3:15: in call to 'f(nullptr)'
",
    );
}

/// [class.union]/2: `u.f += 1` reads the inactive member.
#[test]
fn ub_union_compound_assign() {
    fails_exactly(
        r"union U { int i; float f; };
constexpr int f() {
  U u = {1};
  u.f += 1;
  return 0;
}
static_assert(f() == 0);
",
        r"
7:15: static assertion expression is not an integral constant expression
4:7: assignment to member 'f' of union with active member 'i' is not allowed in a constant expression
7:15: in call to 'f()'
",
    );
}

/// [class.union]/2: reading an inactive member through a pointer.
#[test]
fn ub_union_via_pointer() {
    fails_exactly(
        r"union U { int i; float f; };
constexpr int f() {
  U u = {1};
  float *q = &u.f;
  return *q;
}
static_assert(f() == 0);
",
        r"
7:15: static assertion expression is not an integral constant expression
5:10: read of member 'f' of union with active member 'i' is not allowed in a constant expression
7:15: in call to 'f()'
",
    );
}

/// [conv.fpint]/1: an implicit floating-integral conversion out of range.
#[test]
fn ub_float_to_int_implicit() {
    fails_exactly(
        r"constexpr int f(double d) {
  int x = d + 1;
  return x;
}
static_assert(f(1e10) == 0);
",
        r"
5:15: static assertion expression is not an integral constant expression
2:11: value 10000000001 is outside the range of representable values of type 'int'
5:15: in call to 'f(1.000000e+10)'
",
    );
}

/// [expr.add]/5.3: pointers into different member arrays of one object.
#[test]
fn ub_ptr_sub_subarrays() {
    fails_exactly(
        r"struct S { int a[2]; int b[2]; };
constexpr long f() {
  S s = {{1, 2}, {3, 4}};
  return &s.b[0] - &s.a[0];
}
static_assert(f() == 0);
",
        r"
6:15: static assertion expression is not an integral constant expression
4:18: subtracted pointers are not elements of the same array
6:15: in call to 'f()'
",
    );
}

/// [expr.add]/5: subtracting a non-null pointer from null.
#[test]
fn ub_ptr_sub_null() {
    fails_exactly(
        r"constexpr long f() {
  int x[2] = {1, 2};
  int *p = nullptr;
  return p - x;
}
static_assert(f() == 0);
",
        r"
6:15: static assertion expression is not an integral constant expression
4:12: subexpression not valid in a constant expression
6:15: in call to 'f()'
",
    );
}

/// [basic.life]/7: a block-scope local escaping its scope and its call.
#[test]
fn ub_lifetime_scope_then_return() {
    fails_exactly(
        r"constexpr int *g() {
  int *p = nullptr;
  {
    int x = 1;
    p = &x;
  }
  return p;
}
constexpr int f() { return *g(); }
static_assert(f() == 1);
",
        r"
10:15: static assertion expression is not an integral constant expression
9:28: read of variable whose lifetime has ended
10:15: in call to 'f()'
4:9: declared here
",
    );
}

/// [expr.shift]/1: an unsigned shift count >= width is also undefined.
#[test]
fn ub_shift_unsigned_too_wide() {
    fails_exactly(
        r"constexpr unsigned f(int n) { return 1u >> n; }
static_assert(f(32) == 0);
",
        r"
2:15: static assertion expression is not an integral constant expression
1:41: shift count 32 >= width of type 'unsigned int' (32 bits)
2:15: in call to 'f(32)'
",
    );
}

/// [conv.prom]/1: unsigned short promotes to int, so the product can overflow.
#[test]
fn ub_overflow_unsigned_promoted() {
    fails_exactly(
        r"constexpr int f(unsigned short a) { return a * a; }
static_assert(f(65535) == 0);
",
        r"
2:15: static assertion expression is not an integral constant expression
1:46: value 4294836225 is outside the range of representable values of type 'int'
2:15: in call to 'f(65535)'
",
    );
}

/// [expr.const]/5.8: reading a member through a past-the-end struct pointer.
#[test]
fn ub_index_past_end_read_struct() {
    fails_exactly(
        r"struct S { int a; };
constexpr int f() {
  S s[1] = {{1}};
  S *p = s + 1;
  return p->a;
}
static_assert(f() == 1);
",
        r"
7:15: static assertion expression is not an integral constant expression
5:13: cannot access field of pointer past the end of object
7:15: in call to 'f()'
",
    );
}

/// [lex.string]/9: string literal objects are const; modification is undefined.
#[test]
fn ub_modify_string_literal() {
    fails_exactly(
        r#"constexpr char f() {
  char *p = (char *)"ab";
  p[0] = 'x';
  return p[0];
}
static_assert(f() == 'x');
"#,
        r#"
6:15: static assertion expression is not an integral constant expression
3:8: assignment to object '"ab"[0]' whose value is not known
6:15: in call to 'f()'
2:21: temporary created here
"#,
    );
}
