// [meta.reflection.substitute] (P2996R13): substitute(templ, args) is the
// reflection of templ<args...>: a class or variable template
// specialization, the unique function template specialization, the alias
// template specialization (the alias is kept, not replaced by what it
// names) or, for a concept, the bool value of the concept-id. Forming a
// class specialization does not instantiate it. Equal arguments give the
// same reflection. Compared with GCC 16.1 (spec/harness.md "GCC oracle").
//@ expect: compile-ok
//@ oracle: gcc
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
#if __has_include(<meta>)
#include <meta>
#else
#include "kiln_meta.h"
#endif
using namespace std::meta;

template <class T, class U = T> struct Pair {};
template <class T> using PairPtr = Pair<T*>;
template <class T> constexpr int v = sizeof(T);
template <class T> void f(T) {}
template <class... Ts> struct Pk {};
template <class T> concept C = true;
template <class T> concept C1 = sizeof(T) == 1;
template <bool B> struct BoolT {};
template <class T> struct Z { static_assert(sizeof(T) == 100); };
template <template <class> class TT> struct TTa {};
template <class T> struct One {};
using I = int;

static_assert(substitute(^^Pair, {^^I}) == ^^Pair<int>);
static_assert(substitute(^^Pair, {^^int, ^^char}) == ^^Pair<int, char>);
static_assert(substitute(^^PairPtr, {^^int}) == ^^PairPtr<int>);
static_assert(substitute(^^PairPtr, {^^int}) != ^^Pair<int*>);
static_assert(substitute(^^v, {^^int}) == ^^v<int>);
static_assert(substitute(^^f, {^^int}) == ^^f<int>);
static_assert(substitute(^^Pk, {}) == ^^Pk<>);
static_assert(substitute(^^Pk, {^^int, ^^char, ^^long}) ==
              ^^Pk<int, char, long>);
static_assert(substitute(^^TTa, {^^One}) == ^^TTa<One>);
static_assert(substitute(^^Z, {^^int}) == ^^Z<int>);  // Z<int> not instantiated

// A concept gives the value of the concept-id, usable as a template
// argument in its turn.
static_assert(substitute(^^BoolT, {substitute(^^C, {^^int})}) == ^^BoolT<true>);
static_assert(substitute(^^BoolT, {substitute(^^C1, {^^int})}) ==
              ^^BoolT<false>);
static_assert(substitute(^^C1, {^^int}) != substitute(^^C, {^^int}));
static_assert(substitute(^^C1, {^^int}) == substitute(^^C1, {^^long}));

// Arguments come from any range of reflections.
constexpr info args[] = {^^long, ^^char};
static_assert(substitute(^^Pair, args) == ^^Pair<long, char>);
consteval info twice(info t) {
  info a[] = {t, t};
  return substitute(^^Pair, a);
}
static_assert(twice(^^char) == ^^Pair<char, char>);

// The result is a reflection like any other: splice it.
typename [:substitute(^^Pair, {^^int}):] p;
static_assert(sizeof(p) == sizeof(Pair<int>));

int main() {}
