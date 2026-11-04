// [expr.reflect]/7 (P2996R13), [temp.res.general]/4-5,
// [meta.reflection.queries]: the operator, conversion, destructor and
// dependent qualified names `^^` takes are still subject to the
// single-entity rule: an operator-function-id naming several functions, a
// template-id that selects several specializations (`^^tf<int>` with two
// viable templates) and a dependent `^^T::g` that instantiates to an
// overload set are "overload set" errors. A dependent `^^T::m` naming a
// type, or a class template without `template`, needs `typename`
// (instantiation yields a type where a non-type was parsed); a missing
// member is the usual lookup error. A destructor has no type.
// Each operand is rejected by GCC 16.1 too (spec/harness.md "GCC oracle").
//@ expect: compile-fail
//@ oracle: gcc
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 51:25 error "cannot take the reflection of an overload set"
//@ diag: 52:22 error "cannot take the reflection of an overload set"
//@ diag: 53:22 error "cannot take the reflection of an overload set"
//@ diag: 54:54 error "cannot take the reflection of an overload set"
//@ diag: 55:15 note "in instantiation of function template specialization 'rg<S>' requested here"
//@ diag: 56:54 error "missing 'typename' prior to dependent type name 'S::type'"
//@ diag: 57:15 note "in instantiation of function template specialization 'rt<S>' requested here"
//@ diag: 58:54 error "missing 'typename' prior to dependent type name 'S::inner'"
//@ diag: 59:15 note "in instantiation of function template specialization 'ri<S>' requested here"
//@ diag: 60:54 error "no member named 'nomember' in 'S'"
//@ diag: 61:15 note "in instantiation of function template specialization 'rn<S>' requested here"
//@ diag: 62:20 error "call to consteval function 'std::meta::type_of' is not a constant expression"
//@ diag: 62:20 note "'^^S::~S' has no type"
//@ diag: 62:16 error "constexpr variable 'h' must be initialized by a constant expression"
//@ diag: 62:20 note "'^^S::~S' has no type"
#if __has_include(<meta>)
#include <meta>
#else
#include "kiln_meta_queries.h"
#endif
using std::meta::info;
struct S {
  S& operator=(const S&);
  S& operator=(S&&);
  void g();
  void g(int);
  using type = int;
  template <class U> struct inner {};
  ~S();
};
int operator+(S, S);
int operator+(S, int);
template <class T> void tf(T);
template <class T> void tf(T, int = 0);
constexpr info a = ^^S::operator=;
constexpr info b = ^^operator+;
constexpr info c = ^^tf<int>;
template <class T> consteval info rg() { return ^^T::g; }
template info rg<S>();
template <class T> consteval info rt() { return ^^T::type; }
template info rt<S>();
template <class T> consteval info ri() { return ^^T::inner; }
template info ri<S>();
template <class T> consteval info rn() { return ^^T::nomember; }
template info rn<S>();
constexpr info h = std::meta::type_of(^^S::~S);
