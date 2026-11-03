// [meta.reflection.substitute] (P2996R13): template_of(r) is the template
// of the specialization r reflects, for class, variable, function and
// alias template specializations; a specialization named but never
// instantiated (an incomplete Y<int>) has a template too, and asking does
// not instantiate it. Compared with GCC 16.1 (spec/harness.md "GCC oracle").
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
template <class T> struct Pair<char, T> {};
template <class T> using PairPtr = Pair<T*>;
template <class T> constexpr int v = 0;
template <class T> void f(T) {}
template <class T> struct Y;
template <class T> struct Z { static_assert(sizeof(T) == 100); };
template <auto... V> struct P {};

static_assert(template_of(^^Pair<int>) == ^^Pair);
static_assert(template_of(^^Pair<char, int>) == ^^Pair);  // partial spec
static_assert(template_of(^^PairPtr<int>) == ^^PairPtr);  // alias kept
static_assert(template_of(^^v<int>) == ^^v);
static_assert(template_of(^^f<int>) == ^^f);
static_assert(template_of(^^Y<int>) == ^^Y);  // never instantiated
static_assert(template_of(^^Z<int>) == ^^Z);  // not instantiated by asking
static_assert(template_of(^^P<1, 'c'>) == ^^P);
static_assert(template_of(^^P<>) == ^^P);

int main() {}
