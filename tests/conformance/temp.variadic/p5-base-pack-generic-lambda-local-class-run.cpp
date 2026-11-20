// [class.derived.general]/1, [temp.variadic]/5.3: a base-specifier pack
// expansion naming a generic lambda's own pack (`decltype(xs)...`) in a
// local class of the lambda, in a function template: the template's
// instantiation keeps the pack until the call operator is instantiated.
// clang 22 crashes on it (SubstBaseSpecifiers); clang 19 accepts it.
//@ expect: run
//@ std: c++26
//@ requires: clang<22
//@ stdout: 12 12
#include <cstdio>
struct A { int a = 1; };
struct B { int b = 2; };
template <class T> int f() {
  auto l = [](auto... xs) { struct L : decltype(xs)... { T t; }; return int(sizeof(L)); };
  return l(A{}, B{});
}
int main() { std::printf("%d %d\n", f<int>(), f<char>()); }
