// [expr.prim.splice]/2.1.4 (P2996R13): a potentially-evaluated
// splice-expression designating a local entity for which there is an
// intervening lambda scope is ill-formed; the effect of an enclosing
// `typeid` is disregarded. A capture (explicit or by default) does not
// make it valid: naming the local through a splice is not an odr-use, so
// it never captures. The reflection itself (`rx`) is a constexpr local
// usable in constant expressions, so naming it in the splice operand is
// fine ([basic.def.odr]/5). Also in a contract predicate (P2900R14).
// GCC 16.1 rejects every case ("use of local variable with automatic
// storage from containing function in a splice expression").
//@ expect: compile-fail
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang 22 does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ diag: 41:24 error "cannot splice local entity 'x' for which there is an intervening lambda expression"
//@ diag: 38:7 note "'x' declared here"
//@ diag: 42:25 error "cannot splice local entity 'x' for which there is an intervening lambda expression"
//@ diag: 38:7 note "'x' declared here"
//@ diag: 43:25 error "cannot splice local entity 'p' for which there is an intervening lambda expression"
//@ diag: 37:12 note "'p' declared here"
//@ diag: 44:32 error "cannot splice local entity 'x' for which there is an intervening lambda expression"
//@ diag: 38:7 note "'x' declared here"
//@ diag: 45:36 error "cannot splice local entity 'x' for which there is an intervening lambda expression"
//@ diag: 38:7 note "'x' declared here"
//@ diag: 48:24 error "cannot splice local entity 's' for which there is an intervening lambda expression"
//@ diag: 46:9 note "'s' declared here"
//@ diag: 59:31 error "cannot splice local entity 'p' for which there is an intervening lambda expression"
//@ diag: 57:11 note "'p' declared here"
//@ diag: 53:22 error "cannot splice local entity 'v' for which there is an intervening lambda expression"
//@ diag: 53:13 note "while substituting into a lambda expression here"
//@ diag: 55:9 note "in instantiation of function template specialization 'tf<int>' requested here"
//@ diag: 51:29 note "'v' declared here"
#include <meta>
#include <typeinfo>

void f(int p) {
  int x = 0;
  constexpr auto rx = ^^x;
  constexpr auto rp = ^^p;
  auto a = [] { return [: rx :]; };
  auto b = [&] { return [: rx :]; };
  auto c = [=] { return [: rp :]; };
  auto d = [] { return &typeid([: rx :]); };
  auto e = [] { return [] { return [: rx :]; }; };
  auto [s, t] = [] { struct P { int u, v; }; return P{1, 2}; }();
  constexpr auto rs = ^^s;
  auto g = [] { return [: rs :]; };
}

template <class T> int tf(T v) {
  constexpr auto rv = ^^v;
  return [] { return [: rv :]; }();
}
int i = tf(1);

int h(int p) {
  constexpr auto rp = ^^p;
  contract_assert([] { return [: rp :]; }() > 0);
  return p;
}
