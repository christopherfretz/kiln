// [basic.types.general]/12, [expr.const]/18 (P2996R13, CWG3115): what the
// consteval-only function rule does not reject. A member declared before
// the class's first consteval-only member; a static member function (no
// object parameter); a consteval member; a defaulted member and the
// members of a class template specialization declared `constexpr` (both
// immediate-escalating); a member of a class template whose type is
// consteval-only only after substitution, never defined or never used; a
// class whose only consteval-only member is static.
// Compared with GCC 16.1 (spec/harness.md "GCC oracle").
//@ expect: compile-ok
//@ oracle: gcc
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#if __has_include(<meta>)
#include <meta>
#else
#include "kiln_meta.h"
#endif
using std::meta::info;
struct A { int before(); info r; static int s(); consteval int c() const { return 1; } };
struct B { info r; B() = default; B(const B&) = default; ~B() = default; };
template <class T> struct F { T r; constexpr int k() const { return 1; } };
constexpr F<info> fi{^^int};
static_assert(fi.k() == 1);
template <class T> struct W { T t; void declared(); void unused() {} };
consteval int use_w() { W<info> w{^^int}; (void)w; return 0; }
static_assert(use_w() == 0);
struct G { int x; int f(); };
struct H { static constexpr info s = ^^int; int f(); };
static_assert(H::s == ^^int);
template <class T> constexpr T ident(T t) { return t; }
static_assert(ident(^^int) == ^^int);
