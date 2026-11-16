// [meta.reflection.extract]/12: extract by value is
// `extract-value<U>(constant_of(r))`, so a block-scope variable usable in
// constant expressions ([expr.const]) gives its constant whatever its
// storage duration: a `constexpr` local, a `const int` with a constant
// initializer, a `constexpr` local of literal class type, one reached
// through a template argument, one of a consteval function or a lambda.
// extract of a reference (extract-ref, /5.3) of such a variable is valid
// too: usable in constant expressions suffices there; only its address
// is not a permitted result of a constant expression outside the
// function (local-constant-neg.cpp). Reported by a user; GCC 16.1 agrees.
//@ expect: compile-ok
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
#include <meta>

struct P { int a; int b; };

template <std::meta::info R> constexpr int by_targ = std::meta::extract<int>(R);

consteval int in_consteval() {
  constexpr int x = 7;
  constexpr int y = std::meta::extract<int>(^^x);
  return y + std::meta::extract<int>(^^x);
}
static_assert(in_consteval() == 14);

int f() {
  constexpr int b = 2;
  static_assert(std::meta::extract<int>(^^b) == 2);
  static_assert(std::meta::extract<const int>(^^b) == 2);
  static_assert(std::meta::extract<const int &>(^^b) == 2);
  static_assert(&std::meta::extract<const int &>(^^b) == &std::meta::extract<const int &>(^^b));
  constexpr int b2 = std::meta::extract<const int &>(^^b);
  static_assert(b2 == 2);
  const int c = 3;
  static_assert(std::meta::extract<int>(^^c) == 3);
  constexpr P p{4, 5};
  static_assert(std::meta::extract<P>(^^p).b == 5);
  constexpr int t = 6;
  static_assert(by_targ<^^t> == 6);
  constexpr auto rt = ^^t;
  static_assert(std::meta::extract<int>(rt) == 6);
  int v = by_targ<^^b>;
  auto l = [] {
    constexpr int z = 8;
    return std::meta::extract<int>(^^z);
  };
  static_assert(l() == 8);
  constexpr int w = [] {
    constexpr int z = 9;
    return std::meta::extract<int>(^^z);
  }();
  static_assert(w == 9);
  return v + c;
}

// A reflection that escaped its function in an EARLIER evaluation: the
// variable is still usable in constant expressions and its lifetime did
// not begin in this one.
consteval std::meta::info leak() {
  constexpr int k = 42;
  return ^^k;
}
constexpr std::meta::info leaked = leak();
static_assert(std::meta::extract<int>(leaked) == 42);

int main() { return f() == 5 ? 0 : 1; }
