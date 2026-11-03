// [meta.reflection.substitute] (P2996R13): template_arguments_of(r) is
// the specialization's template arguments, in order: types as types,
// templates as templates, and constant arguments as reflections of the
// constant (reflect_constant-shaped) or of the referenced entity.
// Defaulted arguments are present and packs are expanded. Compared with
// GCC 16.1 (spec/harness.md "GCC oracle").
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
template <class T> void f(T) {}
template <class T> constexpr int v = 0;
template <class... Ts> struct Pk {};
template <auto... V> struct P {};
template <class T> struct Y;
template <long N> struct LongT {};
template <char N> struct CharT {};
using I = int;

static_assert(template_arguments_of(^^Pair<I>).size() == 2);
static_assert(template_arguments_of(^^Pair<I>)[0] == ^^int);
static_assert(template_arguments_of(^^Pair<I>)[1] == ^^int);  // default
static_assert(template_arguments_of(^^Pair<int, char>)[1] == ^^char);
static_assert(template_arguments_of(^^PairPtr<I>).size() == 1);
static_assert(template_arguments_of(^^PairPtr<I>)[0] == ^^int);
static_assert(template_arguments_of(^^f<int>)[0] == ^^int);
static_assert(template_arguments_of(^^v<char>)[0] == ^^char);
static_assert(template_arguments_of(^^Y<long>)[0] == ^^long);
static_assert(template_arguments_of(^^Pk<int, char>).size() == 2);
static_assert(template_arguments_of(^^Pk<int, char>)[1] == ^^char);
static_assert(template_arguments_of(^^Pk<>).size() == 0);
static_assert(template_arguments_of(^^P<1, 2u>).size() == 2);

// The constant arguments round-trip through substitute.
static_assert(substitute(^^LongT, {template_arguments_of(^^P<1, 2u>)[0]}) ==
              ^^LongT<1>);
static_assert(substitute(^^CharT, {template_arguments_of(^^CharT<'a'>)[0]}) ==
              ^^CharT<97>);
// The constant's type is part of its reflection: int 1 is not long 1.
static_assert(template_arguments_of(^^P<1>)[0] ==
              template_arguments_of(^^P<1>)[0]);
static_assert(template_arguments_of(^^P<1>)[0] !=
              template_arguments_of(^^LongT<1>)[0]);

struct S {};
int i;
template <int, int&, S, template <class> class> struct X {};
constexpr auto T = ^^X<1, i, S{}, PairPtr>;
static_assert(template_arguments_of(T).size() == 4);
static_assert(template_arguments_of(T)[3] == ^^PairPtr);
static_assert(substitute(^^X, template_arguments_of(T)) == T);

int main() {}
