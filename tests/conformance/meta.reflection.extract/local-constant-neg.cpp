// [meta.reflection.extract]: by value, a block-scope variable not usable
// in constant expressions (a non-const local; a const one whose
// initializer is not a constant) has no constant; a reference extracted
// from an automatic constexpr local is valid but not a permitted result
// of the initializer of a constexpr reference or pointer outside its
// function ([expr.const]); the type must match; and a
// local whose lifetime began within the evaluation and ended is read
// outside its lifetime ([meta.reflection.queries] `constant_of`'s
// example). GCC 16.1 rejects all of these too. Reported by a user.
//@ expect: compile-fail
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ strict-diags
//@ diag: 38:17 error "static assertion expression is not an integral constant expression"
//@ diag: 38:17 note "'^^n' is not a variable of static storage duration, nor one whose lifetime began within the evaluation"
//@ diag: 37:7 note "declared here"
//@ diag: 41:51 error "constexpr variable 'gr<^^b>' must be initialized by a constant expression"
//@ diag: 44:10 note "in instantiation of variable template specialization 'gr<^^b>' requested here"
//@ diag: 41:51 note "reference to 'b' is not a constant expression"
//@ diag: 43:17 note "address of non-static constexpr variable 'b' may differ on each invocation of the enclosing function; add 'static' to give it a constant address"
//@ diag: 41:56 error "call to consteval function 'std::meta::extract<const int &>' is not a constant expression"
//@ diag: 41:56 note "reference to 'b' is not a constant expression"
//@ diag: 43:17 note "address of non-static constexpr variable 'b' may differ on each invocation of the enclosing function; add 'static' to give it a constant address"
//@ diag: 48:17 error "static assertion expression is not an integral constant expression"
//@ diag: 48:17 note "'^^b' cannot be extracted as a 'long'"
//@ diag: 53:17 error "static assertion expression is not an integral constant expression"
//@ diag: 53:17 note "'^^c' is not a variable of static storage duration, nor one whose lifetime began within the evaluation"
//@ diag: 52:13 note "declared here"
//@ diag: 60:15 error "static assertion expression is not an integral constant expression"
//@ diag: 60:15 note "'^^x' is not a variable of static storage duration, nor one whose lifetime began within the evaluation"
//@ diag: 57:17 note "declared here"
//@ diag: 65:17 error "splice operand must be a constant expression"
//@ diag: 65:17 note "read of variable 'y' outside its lifetime is not allowed in a constant expression"
#include <meta>

int h() {
  int n = 1;
  static_assert(std::meta::extract<int>(^^n) == 1);
  return n;
}
template <std::meta::info R> constexpr const int &gr = std::meta::extract<const int &>(R);
int i() {
  constexpr int b = 2;
  return gr<^^b>;
}
int j() {
  constexpr int b = 2;
  static_assert(std::meta::extract<long>(^^b) == 2);
  return b;
}
int k(int m) {
  const int c = m;
  static_assert(std::meta::extract<int>(^^c) == 1);
  return c;
}
consteval std::meta::info fn() {
  constexpr int x = 42;
  return ^^x;
}
static_assert(std::meta::extract<int>(fn()) == 42);
consteval std::meta::info fn2() {
  constexpr int y = 43;
  return ^^y;
}
static_assert([:std::meta::constant_of(fn2()):] == 43);
