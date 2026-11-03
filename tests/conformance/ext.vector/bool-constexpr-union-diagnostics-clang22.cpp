// Clang ext vectors as union members, [clang 20+] ([class.union.general]
// p6, [expr.const]; clang VectorExprEvaluator, lvalue vector element
// access): a lane store does not change the active member (only member
// access and array subscript do, and a vector is neither), so assigning
// a lane of an inactive `bool` or `int` vector member is not a constant
// expression, by subscript or by name; a lane of the active member is
// assigned in place. WP-438.
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ requires: clang>=22
//@ diag: 31:15 error "static assertion expression is not an integral constant expression"
//@ diag: 28:10 note "assignment to member 'v' of union with active member 'c' is not allowed in a constant expression"
//@ diag: 31:15 note "in call to 'g()'"
//@ diag: 37:15 error "static assertion expression is not an integral constant expression"
//@ diag: 34:10 note "assignment to member 'v' of union with active member 'c' is not allowed in a constant expression"
//@ diag: 37:15 note "in call to 'h(2)'"
//@ diag: 43:15 error "static assertion expression is not an integral constant expression"
//@ diag: 40:9 note "assignment to member 'v' of union with active member 'c' is not allowed in a constant expression"
//@ diag: 43:15 note "in call to 'k()'"
typedef bool b8 __attribute__((ext_vector_type(8)));
typedef int i4 __attribute__((ext_vector_type(4)));
union U { b8 v; unsigned char c; };
union I { i4 v; int c; };

constexpr bool g() {
  U w{.c = 3};
  w.v[1] = true;
  return w.v[1];
}
static_assert(g());
constexpr int h(int i) {
  I w{.c = 3};
  w.v[i] = 4;
  return w.v[i];
}
static_assert(h(2) == 4);
constexpr int k() {
  I w{.c = 3};
  w.v.y = 4;
  return w.v.y;
}
static_assert(k() == 4);

constexpr unsigned char ok() {
  U w{.v = (b8){true}};
  w.v[3] = true;
  w.v[0] = !w.v[0];
  return __builtin_bit_cast(unsigned char, w.v);
}
static_assert(ok() == 8);
constexpr int ok2() {
  I w{.v = (i4){1, 2, 3, 4}};
  w.v[1] += 5;
  w.v.z = 9;
  return w.v[1] * 10 + w.v[2];
}
static_assert(ok2() == 79);
