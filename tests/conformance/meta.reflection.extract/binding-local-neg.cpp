// [meta.reflection.extract], [expr.const]/10: a structured binding or
// variable whose lifetime did not begin within the constant evaluation
// is not read by it: a non-constexpr global, the bindings and locals of
// a non-constexpr function seen from a consteval call in it (an
// immediate invocation is an evaluation of its own) or from a
// static_assert, and a binding whose frame has returned. Reduced from
// real code.
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ expect: compile-fail
//@ strict-diags
//@ diag: 33:15 error "static assertion expression is not an integral constant expression"
//@ diag: 32:33 note "read of non-const variable 'gv' is not allowed in a constant expression"
//@ diag: 33:15 note "in call to 'global()'"
//@ diag: 31:5 note "declared here"
//@ diag: 36:10 error "call to consteval function 'read' is not a constant expression"
//@ diag: 30:48 note "'^^x' is not a variable of static storage duration, nor one whose lifetime began within the evaluation"
//@ diag: 36:10 note "in call to 'read(^^x)'"
//@ diag: 35:7 note "declared here"
//@ diag: 40:10 error "call to consteval function 'read' is not a constant expression"
//@ diag: 30:48 note "the value of 'a' is not usable in a constant expression"
//@ diag: 40:10 note "in call to 'read(^^a)'"
//@ diag: 44:17 error "static assertion expression is not an integral constant expression"
//@ diag: 44:17 note "the value of 'a' is not usable in a constant expression"
//@ diag: 51:15 error "static assertion expression is not an integral constant expression"
//@ diag: 50:32 note "the value of 'a' is not usable in a constant expression"
//@ diag: 51:15 note "in call to 'stale()'"
#include <meta>
struct P { int a; int b; };
consteval int read(std::meta::info r) { return std::meta::extract<int>(r); }
int gv = 1;
consteval int global() { return std::meta::extract<int>(^^gv); }
static_assert(global() == 1);
int outer() {
  int x = 1;
  return read(^^x);
}
int outer_binding() {
  auto [a, b] = P{1, 2};
  return read(^^a);
}
void in_static_assert() {
  auto [a, b] = P{1, 2};
  static_assert(std::meta::extract<int>(^^a) == 1);
}
consteval std::meta::info leak() {
  auto [a, b] = P{1, 2};
  return ^^a;
}
consteval int stale() { return std::meta::extract<int>(leak()); }
static_assert(stale() == 1);
