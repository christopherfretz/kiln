// [meta.reflection.names]/6-7 (P2996R13): display_string_of as GCC 16.1's
// `%D`/`%T` (spec/reflection.md §3.9) for template instantiations and
// constants. GCC prints a member of a class template specialization, or
// a member template of one, through the primary template's member with
// the bindings: levels innermost first, a member template's own non-type
// parameters bound to themselves, then the typedefs the signature names
// with what they denote here (`[with T = int; long unsigned int N = 9;
// size_type = long unsigned int]`); the rest of the declarator after the
// name does not repeat the function's scope (`const_iterator`, `Inner`).
// A class-type template argument prints GCC's constant: a `char` array as
// a string literal, any other array with its type, an empty class as
// `T()`, a null pointer as `0`. A binding generator writes these strings
// into its manifest.
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

namespace B {
template <class T, unsigned long N> struct LV {
  using const_iterator = const T*;
  using size_type = decltype(sizeof 0);
  template <class I> T* insert(const_iterator, I, I);
  void push(const T&);
  size_type size() const;
  template <unsigned long S> int cmp(const LV<T, S>&) const;
};
struct S {
  struct Inner {};
  void g(Inner);
  static Inner make();
};
template <unsigned N> struct fs {
  char data[N];
  consteval fs(const char (&s)[N]) {
    for (unsigned i = 0; i < N; ++i) data[i] = s[i];
  }
};
struct Q {
  int a[2];
  char c;
};
struct E {};
struct NP {
  const int* p;
};
template <auto V> struct W {};
}  // namespace B

consteval bool eq(std::string_view a, const char* b) {
  decltype(sizeof 0) n = 0;
  while (b[n]) {
    if (n >= a.size() || a.data()[n] != b[n]) return false;
    ++n;
  }
  return n == a.size();
}
#define CHECK(r, s) static_assert(eq(display_string_of(r), s))

CHECK((^^B::LV<int, 9>::push), "void B::LV<T, N>::push(const T&) [with T = int; long unsigned int N = 9]");
CHECK((^^B::LV<int, 9>::size),
      "B::LV<T, N>::size_type B::LV<T, N>::size() const [with T = int; long unsigned int N = 9; "
      "size_type = long unsigned int]");
CHECK((^^B::LV<int, 9>::insert),
      "template<class I> T* B::LV<T, N>::insert(const_iterator, I, I) [with T = int; long "
      "unsigned int N = 9]");
CHECK((^^B::LV<int, 9>::cmp),
      "template<long unsigned int S> int B::LV<T, N>::cmp(const B::LV<T, S>&) const [with long "
      "unsigned int S = S; T = int; long unsigned int N = 9]");
CHECK(^^B::S::g, "void B::S::g(Inner)");
CHECK(^^B::S::make, "static B::S::Inner B::S::make()");
CHECK(^^B::W<B::fs{"operator[]"}>, "B::W<B::fs<11>{\"operator[]\"}>");
CHECK(^^B::W<B::fs{"a\nb"}>, "B::W<B::fs<4>{\"a\\012b\"}>");
CHECK((^^B::W<B::Q{{1, 2}, 'c'}>), "B::W<B::Q{int [2]{1, 2}, 'c'}>");
CHECK((^^B::W<B::Q{{0, 0}, 0}>), "B::W<B::Q{int [2]{0, 0}, '\\000'}>");
CHECK(^^B::W<B::E{}>, "B::W<B::E()>");
CHECK(^^B::W<B::NP{nullptr}>, "B::W<B::NP{0}>");

int main() {}
