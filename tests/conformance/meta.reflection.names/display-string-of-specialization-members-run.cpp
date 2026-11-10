// [meta.reflection.names]/6-7, [temp.inst] (P2996R13; spec/reflection.md
// §3.9, spec/reflection-repr.md §5, WP-1258): a member of a class template
// specialization names the specialization with its template arguments,
// at every level of its scope (`S<int>::m`, `ns::R<int, 2>::Q::y`), in
// display_string_of of the member, of a constant naming it (a pointer to
// member, an enumerator value, a reflection of it) and of a template
// specialization taking such a constant; a variable template
// specialization prints its arguments. identifier_of is the bare name.
// GCC 16.1 compiles and runs the same file; the output must agree.
//@ expect: run
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ stdout: m S<int>::m
//@ stdout: In S<int>::In
//@ stdout: E S<int>::E
//@ stdout: e1 S<int>::e1
//@ stdout: k S<int>::k
//@ stdout: y ns::R<char, 3>::Q::y
//@ stdout: &S<int>::m
//@ stdout: &S<int>::f
//@ stdout: S<int>::e2
//@ stdout: S<int>::In{4}
//@ stdout: X<&S<int>::m>
//@ stdout: X<S<int>::e2>
//@ stdout: X<^^S<int>::m>
//@ stdout: X<^^S<int>::In>
//@ stdout: ns::vt<int>
//@ stdout: X<^^ns::vt<int> >
#include <cstdio>
#include <meta>
template <class T> struct S {
  void f() {}
  int m;
  struct In { int z; };
  enum E { e1, e2 };
  static constexpr int k = 3;
};
namespace ns {
template <class T, int N> struct R { struct Q { int x; static inline int y; }; };
template <class T> constexpr int vt = 1;
}
template <auto V> struct X {};

#define P(...) std::puts(std::define_static_string(std::meta::display_string_of(__VA_ARGS__)))
#define N(...)                                                              \
  std::printf("%s %s\n", std::define_static_string(std::meta::identifier_of(__VA_ARGS__)), \
              std::define_static_string(std::meta::display_string_of(__VA_ARGS__)))

int main() {
  N(^^S<int>::m);
  N(^^S<int>::In);
  N(^^S<int>::E);
  N(^^S<int>::e1);
  N(^^S<int>::k);
  N(^^ns::R<char, 3>::Q::y);
  P(std::meta::reflect_constant(&S<int>::m));
  P(std::meta::reflect_constant(&S<int>::f));
  P(std::meta::reflect_constant(S<int>::e2));
  P(std::meta::reflect_constant(S<int>::In{4}));
  P(^^X<&S<int>::m>);
  P(^^X<S<int>::e2>);
  P(^^X<^^S<int>::m>);
  P(^^X<^^S<int>::In>);
  P(^^ns::vt<int>);
  P(^^X<^^ns::vt<int>>);
}
