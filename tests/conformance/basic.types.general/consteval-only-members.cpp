// [expr.const.imm]/3, [expr.const.const] (P4101R1): no function is
// restricted by its type. P2996R13's rule (with CWG3115: a member of a
// class with an `info` member, a constructor, destructor, conversion
// function or explicit object parameter, or a type that holds `info`
// only after substitution, must be consteval) is gone: every function
// below is an ordinary function whose run-time `info` is the null
// reflection (WP-1361). A non-static data member of an object holding a
// non-null reflection, read outside an immediate function context, is
// still an immediate constant expression ([expr.const.const]/5.1). kiln
// only: GCC 16.1 rejects each function (docs/status/deviations.md).
//@ expect: compile-fail
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 30:19 error "consteval-only value outside an immediate function context"
#if __has_include(<meta>)
#include <meta>
#else
#include "kiln_meta.h"
#endif
using std::meta::info;
// Members of a class with an `info` member are ordinary functions.
struct A { int f(); info r; int g(); };
struct C { info r; C(); ~C(); operator int() const; };
struct D { info r; auto h(this D const&) { return 0; } };
struct E { info r; constexpr int k() const { return 1; } };
// Member access to a consteval-only member is a consteval-only leaf.
struct H { info m; consteval H() : m(^^int) {} };
constexpr H h;
void g1() { (void)h.m; }
// A type holding `info` only by substitution: likewise.
template <class T> void tf(T) {}
template <class T> struct W { T t; void m() {} void unused() {} };
consteval void u() { tf(^^int); W<info> w{^^int}; w.m(); }
void rt() { tf(info{}); W<info> w{}; w.m(); E e{}; (void)e.k(); }
