// [meta.reflection.names]/1 (P3096R12): a function parameter has an
// identifier when the declarations naming it agree. kiln gives GCC 16.1's
// answer for instantiations (spec/reflection.md §3.9): a member of a
// class template specialization takes the names of every declaration of
// the primary template's member, which must agree (an in-class `value`
// and an out-of-class `data` give none, instantiated or not); a function
// template specialization takes the names of the template's last
// declaration (GCC forgets a template's older parameter names). BDE-style
// headers name parameters differently in the declaration and the
// out-of-line definition; a binding generator then names the Python
// arguments arg0, arg1, ... as GCC does.
//@ expect: compile-ok
//@ oracle: gcc
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#ifdef __KILN__
#include "kiln_meta.h"
#else
#include <meta>
#endif
using namespace std::meta;

template <class T> struct TT {
  void f(int value, int same);
  void g(int a);
  void h(int x);
};
template <class T> void TT<T>::f(int data, int same) {}
template <class T> void TT<T>::g(int a) {}
template <class T> void ft(int p, int q);
template <class T> void ft(int r, int q) {}
void nf(int a);
void nf(int b) {}
void use() { TT<char>{}.f(1, 2); }

consteval bool named(info p, const char* n) {
  if (!has_identifier(p)) return false;
  std::string_view s = identifier_of(p);
  decltype(sizeof 0) i = 0;
  for (; n[i]; ++i)
    if (i >= s.size() || s.data()[i] != n[i]) return false;
  return i == s.size();
}
static_assert(!has_identifier(parameters_of(^^TT<int>::f)[0]));
static_assert(named(parameters_of(^^TT<int>::f)[1], "same"));
static_assert(named(parameters_of(^^TT<int>::g)[0], "a"));
static_assert(named(parameters_of(^^TT<int>::h)[0], "x"));
static_assert(!has_identifier(parameters_of(^^TT<char>::f)[0]));
static_assert(named(parameters_of(^^ft<int>)[0], "r"));
static_assert(named(parameters_of(^^ft<int>)[1], "q"));
static_assert(!has_identifier(parameters_of(^^nf)[0]));

int main() {}
