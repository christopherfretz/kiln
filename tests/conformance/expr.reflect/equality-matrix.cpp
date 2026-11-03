// [expr.eq]/7 [expr.reflect] P2996R13: two reflections compare equal iff
// they represent the same entity. One reflection of every operand kind,
// written twice (second spellings differ where the language allows), is
// compared all against all: equal exactly on the diagonal.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ oven: steps<=4000 bytes<=2048
#include "kiln_meta.h"
using namespace std::meta;
namespace N { int v; inline namespace In { int w; } }
namespace M = N;
using I = int;
template <class T> struct TC {};
template <class T> using TA = TC<T>;
template <class T> int tv = 0;
template <class T> void tf() {}
template <class T> concept C = true;
enum E { A };
struct S { int m; int b : 2; void f(); static int s; };
auto [p, q] = S{};
constexpr int cv = 1;
constexpr info row[] = {
    info{}, ^^::, ^^N, ^^M, ^^int, ^^I, ^^const int, ^^int*, ^^TC<int>, ^^TA<int>,
    ^^TC, ^^TA, ^^tv, ^^tf, ^^C, ^^tv<int>, ^^tf<int>, ^^A, ^^S::m, ^^S::b,
    ^^S::f, ^^S::s, ^^p, ^^q, ^^cv, ^^N::v, ^^N::w, ^^S, ^^E,
};
constexpr info col[] = {
    info{}, ^^::, ^^::N, ^^::M, ^^signed, ^^::I, ^^int const, ^^I*, ^^TC<I>, ^^TA<I>,
    ^^::TC, ^^::TA, ^^::tv, ^^::tf, ^^::C, ^^tv<I>, ^^tf<I>, ^^E::A, ^^::S::m, ^^S::b,
    ^^::S::f, ^^S::s, ^^::p, ^^::q, ^^::cv, ^^M::v, ^^N::In::w, ^^struct S, ^^enum E,
};
constexpr unsigned long n = sizeof(row) / sizeof(row[0]);
static_assert(n == sizeof(col) / sizeof(col[0]));
consteval bool diagonal() {
  for (unsigned long i = 0; i < n; ++i)
    for (unsigned long j = 0; j < n; ++j)
      if ((row[i] == col[j]) != (i == j) || (row[i] != col[j]) == (i == j)) return false;
  return true;
}
static_assert(diagonal());
