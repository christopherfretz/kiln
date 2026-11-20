// [temp.variadic]/5, [dcl.fct]/16: a pack expansion inside a pattern
// whose packs are substituted first keeps its own pattern, its
// already-substituted packs expanded (clang's TransformPackExpansion
// rebuild). `T... x, decltype(x)... y` declares a function parameter pack
// whose expansion names the earlier parameter pack, in a function type
// of a member alias and of a function template; a template template
// argument pack expansion and a type-constraint naming the enclosing
// pack are kept the same way. Reduced from GCC's
// g++.dg/cpp0x/variadic-nested2.C.
//@ expect: run
//@ std: c++26
//@ stdout: 3 2 2 1
#include <cstdio>
template <class... T> struct S1 { using fptr = void (*)(T... x, decltype(x)... y); };
static_assert(__is_same(S1<int>::fptr, void (*)(int, int)));
static_assert(__is_same(S1<int, char>::fptr, void (*)(int, char, int, char)));
template <class... T> int f(T... x, decltype(x)... y) { return (0 + ... + x) + (0 + ... + y); }
template <class... T> struct S2 { using fn = int(T... x) noexcept(noexcept((x, ...))); };
static_assert(__is_same(S2<int>::fn, int(int) noexcept));
template <class... T> struct Outer {
  template <template <class> class... Ps> struct Inner { static constexpr int n = sizeof...(Ps); };
  template <T... Vs> static constexpr int count = sizeof...(Vs);
};
template <class> struct W {};
template <class... Ts> concept Many = sizeof...(Ts) > 1;
template <class T> int g() { Many<T> auto v = 1; return v + 1; }
int main() {
  std::printf("%d %d %d %d\n", f<int>(1, 2), Outer<int>::Inner<W, W>::n, g<char>(),
              Outer<int, char>::count<1, 'a'> - 1);
}
