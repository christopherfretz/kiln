// [basic.types.general]/12, [dcl.constexpr] (P2996R13, CWG3115): a
// function of consteval-only type is consteval unless it is
// immediate-escalating. A non-static member function's type includes its
// implicit object parameter, so a member of a consteval-only class counts,
// with the members declared before it (GCC 16.1: `A::f` is fine, `A::g`
// is not); constructors, destructors and conversion functions included,
// an explicit object parameter by its type. A function whose type becomes
// consteval-only by substitution is checked when its definition is
// instantiated (an unused member never is). P4101R1 drops this function
// rule (kiln keeps it until WP-1328 land 2). A non-static data member of
// an object holding a non-null reflection, read outside an immediate
// function context, is an immediate constant expression ([expr.const.const]
// /5.1, P4101R1).
// Compared with GCC 16.1 (spec/harness.md "GCC oracle").
//@ expect: compile-fail
//@ oracle: gcc
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 38:33 error "function whose type holds 'std::meta::info' must be declared 'consteval' (a run-time 'std::meta::info' is not supported yet)"
//@ diag: 39:20 error "function whose type holds 'std::meta::info' must be declared 'consteval' (a run-time 'std::meta::info' is not supported yet)"
//@ diag: 39:25 error "function whose type holds 'std::meta::info' must be declared 'consteval' (a run-time 'std::meta::info' is not supported yet)"
//@ diag: 39:31 error "function whose type holds 'std::meta::info' must be declared 'consteval' (a run-time 'std::meta::info' is not supported yet)"
//@ diag: 40:25 error "function whose type holds 'std::meta::info' must be declared 'consteval' (a run-time 'std::meta::info' is not supported yet)"
//@ diag: 41:34 error "function whose type holds 'std::meta::info' must be declared 'consteval' (a run-time 'std::meta::info' is not supported yet)"
//@ diag: 45:19 error "consteval-only value outside an immediate function context"
//@ diag: 47:25 error "function whose type holds 'std::meta::info' must be declared 'consteval' (a run-time 'std::meta::info' is not supported yet)"
//@ diag: 49:22 note "in instantiation of function template specialization 'tf<std::meta::info>' requested here"
//@ diag: 48:41 error "function whose type holds 'std::meta::info' must be declared 'consteval' (a run-time 'std::meta::info' is not supported yet)"
//@ diag: 49:53 note "in instantiation of member function 'W<std::meta::info>::m' requested here"
#if __has_include(<meta>)
#include <meta>
#else
#include "kiln_meta.h"
#endif
using std::meta::info;
// The implicit object parameter counts, with the members declared so far.
struct A { int f(); info r; int g(); };
struct C { info r; C(); ~C(); operator int() const; };
struct D { info r; auto h(this D const&) { return 0; } };
struct E { info r; constexpr int k() const { return 1; } };
// Member access to a consteval-only member is a consteval-only leaf.
struct H { info m; consteval H() : m(^^int) {} };
constexpr H h;
void g1() { (void)h.m; }
// A type consteval-only only by substitution: the instantiated definition.
template <class T> void tf(T) {}
template <class T> struct W { T t; void m() {} void unused() {} };
consteval void u() { tf(^^int); W<info> w{^^int}; w.m(); }
