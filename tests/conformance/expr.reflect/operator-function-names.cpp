// [expr.reflect]/7 (P2996R13), [over.oper], [class.conv.fct]: an
// operator-function-id or conversion-function-id naming exactly one
// function reflects that function, qualified (`^^S::operator=`) or not
// (`^^operator+`); the name is not an identifier, so `has_identifier` is
// false ([meta.reflection.names]). The same names are reflectable as
// dependent qualified names (`^^T::operator()`).
// Compared with GCC 16.1 (spec/harness.md "GCC oracle").
//@ expect: compile-ok
//@ oracle: gcc
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#if __has_include(<meta>)
#include <compare>
#include <meta>
#else
#include "kiln_meta_queries.h"
#endif
using std::meta::info;
struct S {
  S& operator=(const S&);
  int operator()(int);
  operator int() const;
  int operator<=>(const S&) const;
  int x;
};
int operator+(S, S);

static_assert(std::meta::is_function(^^S::operator=));
static_assert(std::meta::is_operator_function(^^S::operator=));
static_assert(std::meta::is_operator_function(^^S::operator()));
static_assert(std::meta::is_operator_function(^^S::operator<=>));
static_assert(std::meta::is_conversion_function(^^S::operator int));
static_assert(!std::meta::is_operator_function(^^S::operator int));
static_assert(std::meta::is_operator_function(^^operator+));
static_assert(std::meta::operator_of(^^S::operator=) == std::meta::operators::op_equals);
static_assert(std::meta::operator_of(^^S::operator()) == std::meta::operators::op_parentheses);
static_assert(std::meta::operator_of(^^operator+) == std::meta::operators::op_plus);
static_assert(std::meta::operator_of(^^S::operator<=>) == std::meta::operators::op_spaceship);

static_assert(!std::meta::has_identifier(^^S::operator=));
static_assert(!std::meta::has_identifier(^^S::operator()));
static_assert(!std::meta::has_identifier(^^S::operator int));
static_assert(!std::meta::has_identifier(^^operator+));
static_assert(std::meta::has_identifier(^^S::x));

static_assert(std::meta::parent_of(^^S::operator=) == ^^S);
static_assert(std::meta::parent_of(^^S::operator int) == ^^S);
static_assert(std::meta::type_of(^^S::operator()) == ^^int(int));
static_assert(std::meta::type_of(^^S::operator int) == ^^int() const);
static_assert(^^S::operator= != ^^S::operator());
static_assert(^^operator+ == ^^::operator+);

template <class T> consteval info r_assign() { return ^^T::operator=; }
template <class T> consteval info r_call() { return ^^T::operator(); }
template <class T> consteval info r_conv() { return ^^T::operator int; }
static_assert(r_assign<S>() == ^^S::operator=);
static_assert(r_call<S>() == ^^S::operator());
static_assert(r_conv<S>() == ^^S::operator int);
