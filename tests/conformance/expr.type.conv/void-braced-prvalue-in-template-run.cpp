// CWG2351 ([expr.type.conv]/2) in a template: `void{}` stays a prvalue of
// type `void` when instantiated. clang 22 rejects it at instantiation
// ("cannot initialize non-class type 'void' with a parenthesized
// initializer list") where it accepts the same expression outside a
// template; kiln follows the standard (docs/status/deviations.md). clang
// 19 rejects `void{}` everywhere (void-braced-prvalue-19.cpp).
//@ expect: run
//@ requires: clang>=20
//@ skip(clang): clang 22 rejects void{} in an instantiated template (CWG2351)
//@ stdout: 3
#include <cstdio>
template<class T> T f(T t) { void{}; (void)void{}; return t; }
template<class T> void g() { return void{}; }
template<class T> constexpr bool c() { return (void{}, true); }
static_assert(c<int>());
int main() { g<int>(); std::printf("%d\n", f(3)); }
