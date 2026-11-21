// [meta.reflection.queries] constant_of (P2996R13) throws unless [: R :]
// is a valid splice-expression: an implicit object member function
// (also a conversion function or operator) is one only in a class member
// access or as the operand of & ([expr.prim.id.general]/4), and a
// member function template (an explicit-object one here) names no
// function until it has template arguments ([expr.prim.splice]/2). The twin of
// constant-of-explicit-object.cpp (WP-1782). GCC 16.1 rejects every line
// ("reflect_function failed" for the member functions).
//@ expect: compile-fail
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ diag: 29:15 error "static assertion expression is not an integral constant expression"
//@ diag: 29:15 note "reflection does not represent an annotation or a valid argument to a splice-expression"
//@ diag: 30:15 error "static assertion expression is not an integral constant expression"
//@ diag: 30:15 note "reflection does not represent an annotation or a valid argument to a splice-expression"
//@ diag: 31:15 error "static assertion expression is not an integral constant expression"
//@ diag: 31:15 note "reflection does not represent an annotation or a valid argument to a splice-expression"
#include <meta>
using namespace std::meta;

struct S {
  int v;
  constexpr int h() const { return v; }
  constexpr operator int() const { return v; }
  template <class T> constexpr int t(this T&& s) { return s.v; }
};
static_assert(constant_of(^^S::h) != info());
static_assert(constant_of(^^S::operator int) != info());
static_assert(constant_of(^^S::t) != info());
