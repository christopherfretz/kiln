// [meta.reflection.substitute] (P2996R13): can_substitute(templ, args) is
// whether templ<args...> is valid: false (no diagnostic) for a wrong
// number or kind of arguments, unsatisfied constraints and a substitution
// failure in an alias template; true for a class specialization whose
// definition would fail to instantiate, since substitution does not
// instantiate it. Compared with GCC 16.1 (spec/harness.md "GCC oracle").
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
using namespace std::meta;

template <class T, class U = T> struct Pair {};
template <class T> requires(sizeof(T) > 1) struct R {};
template <class T> using Bad = typename T::type;
template <class T> struct Z { static_assert(sizeof(T) == 100); };
template <template <class> class TT> struct TTa {};
template <auto... V> struct P {};
template <class T, class U> void h(T, U) {}
template <class T> auto a(T t) { return t; }
struct HasType { using type = long; };

static_assert(can_substitute(^^Pair, {^^int}));
static_assert(can_substitute(^^Pair, {^^int, ^^int}));
static_assert(!can_substitute(^^Pair, {}));
static_assert(!can_substitute(^^Pair, {^^int, ^^int, ^^int}));
static_assert(!can_substitute(^^TTa, {^^int}));
static_assert(!can_substitute(^^P, {^^int}));
static_assert(can_substitute(^^R, {^^int}));
static_assert(!can_substitute(^^R, {^^char}));
static_assert(!can_substitute(^^Bad, {^^int}));
static_assert(can_substitute(^^Bad, {^^HasType}));
static_assert(can_substitute(^^Z, {^^int}));
static_assert(can_substitute(^^h, {^^int, ^^long}));

consteval bool count() {
  int n = 0;
  info ts[] = {^^char, ^^short, ^^int, ^^long};
  for (info t : ts) n += can_substitute(^^R, {t});
  return n == 3;
}
static_assert(count());

int main() {}
