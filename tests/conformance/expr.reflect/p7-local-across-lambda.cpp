// [expr.reflect]/7.1 (P2996R13): `^^id-expression` is ill-formed if the
// id-expression denotes (7.1.1) a variable declared by an init-capture, or
// (7.1.4) a local entity for which a lambda scope intervenes between its
// introduction and the reflect-expression. The lambda scope starts after
// the lambda-introducer ([basic.scope.lambda]), so the declarator is in it
// and an init-capture's initializer is not; a capture (explicit or by
// default) does not help, and an unevaluated operand does not either. A
// local of the lambda itself, its own parameter, and a local named outside
// any lambda are fine. Also in a contract predicate (P2900R14). GCC 16.1
// rejects every line ("cannot be applied a local entity for which there is
// an intervening lambda expression"; the precondition for another
// reason).
//@ expect: compile-fail
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang 22 does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ diag: 34:26 error "cannot take the reflection of a local entity for which there is an intervening lambda expression"
//@ diag: 35:27 error "cannot take the reflection of a local entity for which there is an intervening lambda expression"
//@ diag: 36:27 error "cannot take the reflection of a local entity for which there is an intervening lambda expression"
//@ diag: 37:33 error "cannot take the reflection of a local entity for which there is an intervening lambda expression"
//@ diag: 38:38 error "cannot take the reflection of a local entity for which there is an intervening lambda expression"
//@ diag: 39:46 error "cannot take the reflection of a local entity for which there is an intervening lambda expression"
//@ diag: 40:26 error "cannot take the reflection of a local entity for which there is an intervening lambda expression"
//@ diag: 41:31 error "cannot take the reflection of a local entity for which there is an intervening lambda expression"
//@ diag: 42:47 error "cannot take the reflection of a local entity for which there is an intervening lambda expression"
//@ diag: 42:54 error "cannot take the reflection of a local entity for which there is an intervening lambda expression"
//@ diag: 43:31 error "cannot take the reflection of a local entity declared by init-capture"
//@ diag: 55:32 error "cannot take the reflection of a local entity for which there is an intervening lambda expression"
#include <meta>

void f(int p) {
  int x = 0;
  auto a = [] { return ^^p; };
  auto b = [&] { return ^^x; };
  auto c = [x] { return ^^x; };
  auto d = [] { return sizeof(^^x); };
  auto e = [] { return [] { return ^^x; }; };
  auto g = [](int q) { return [=] { return ^^q; }; };
  auto h = [](decltype(^^p) = {}) { return 0; };
  auto i = []() -> decltype(^^x) { return {}; };
  auto j = [] { struct L { int m() { return ^^x == ^^x; } }; };
  auto k = [y = 1] { return ^^y; };
  // Not ill-formed: its own local and parameter, outside any lambda.
  auto l = [](int q, decltype(^^q)* = nullptr) {
    int z = 0;
    constexpr auto rz = ^^z;
    constexpr auto rq = ^^q;
    return 0;
  };
  constexpr auto rp = ^^p;
  constexpr auto m = [] { return ^^int; };
}

int h(int p) pre([] { return ^^p; }() != ^^int);
