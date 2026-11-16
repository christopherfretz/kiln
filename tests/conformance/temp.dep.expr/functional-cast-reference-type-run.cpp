// [temp.dep.expr]/3, [expr.type.conv]/2: a functional cast to a reference
// type with a type-dependent operand is not type-dependent; it is an lvalue
// (or xvalue) of the referenced type, so a call it is an argument of is
// resolved at the definition and deduces the referenced type.
//@ expect: run
//@ std: c++17
//@ stdout: 3 int
#include <cstdio>

typedef int& R;
template <class T> struct Name { static const char* get() { return "other"; } };
template <> struct Name<int> { static const char* get() { return "int"; } };

template <class T> int down(T x, int n) { return n == 0 ? 0 : 1 + down(R(x), n - 1); }
template <class T> const char* name_of(T) { return Name<T>::get(); }
template <class T> const char* via(T x) { return name_of(R(x)); }

int main() { std::printf("%d %s\n", down(0, 3), via(0)); }
