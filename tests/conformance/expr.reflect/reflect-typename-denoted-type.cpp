// [expr.reflect]/5-6 (P2996R13): `^^typename-specifier` is a type-id that
// is not a reflection-name, so it represents the type it denotes, not the
// type alias its terminal name names. Only `^^reflection-name` (an
// optionally qualified identifier) and a simple-template-id naming an
// alias template represent the alias. Siblings: the plain and qualified
// alias, the alias template specialization, a typename-specifier naming a
// class, `decltype`. GCC 16.1 agrees on every line.
//@ expect: compile-ok
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang 22 does not implement P2996 reflection
//@ oracle: gcc
#include <meta>
using namespace std::meta;

struct S {
  using T = int;
  struct C {};
  using CA = C;
};
namespace N {
using A = int;
}
using A = int;
template <class U> using AT = U*;
int a;

// The typename-specifier: the type denoted.
static_assert(^^typename S::T == ^^int);
static_assert(!is_type_alias(^^typename S::T));
static_assert(^^typename S::C == ^^S::C);
static_assert(^^typename S::CA == ^^S::C);
static_assert(^^const typename S::T == ^^const int);
static_assert(^^typename S::T* == ^^int*);

// A reflection-name: the alias.
static_assert(^^S::T != ^^int);
static_assert(is_type_alias(^^S::T));
static_assert(dealias(^^S::T) == ^^int);
static_assert(is_type_alias(^^A) && is_type_alias(^^N::A));
static_assert(is_type_alias(^^S::CA));

// A simple-template-id naming an alias template: the alias.
static_assert(is_type_alias(^^AT<int>));
static_assert(dealias(^^AT<int>) == ^^int*);

// Other type-ids: the type.
static_assert(^^decltype(a) == ^^int);

// In a template, a non-dependent typename-specifier is resolved at once.
template <class U> consteval info g() { return ^^typename S::T; }
static_assert(g<long>() == ^^int);
