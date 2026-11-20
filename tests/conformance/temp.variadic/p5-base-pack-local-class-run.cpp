// [class.derived.general]/1, [temp.variadic]/5.3: a base-specifier pack
// expansion in a local class of a function template (`struct L : Ts...`),
// also inside a generic lambda instantiated with the template.
//@ expect: run
//@ std: c++26
//@ stdout: 4 2
#include <cstdio>
struct A { int a = 1; };
struct B { int b = 2; };
template <class... Ts> int g() {
  return [](auto y) { struct L : Ts... { int z; }; L v{}; return v.a + v.b + int(sizeof(y)); }(char());
}
template <class... Ts> int k() { struct L : Ts... {}; return sizeof...(Ts); }
int main() { std::printf("%d %d\n", g<A, B>(), k<A, B>()); }
