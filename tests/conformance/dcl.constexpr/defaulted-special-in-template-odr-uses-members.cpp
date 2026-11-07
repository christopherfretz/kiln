// [basic.def.odr]/8, [class.default.ctor]/4: a constexpr defaulted special
// member defined at a use in a template definition (its constant
// evaluation may need it) still odr-uses the constructors and destructors
// of its subobjects, which are therefore defined: the program links. The
// shape of libstdc++'s `vector() = default` over `_Vector_impl` in a
// function template. WP-726.
//@ std: c++20
//@ expect: run
//@ stdout: 7
#include <cstdio>
template <class T> struct Alloc { constexpr Alloc() {} constexpr ~Alloc() {} };
template <class T> struct Impl : Alloc<T> { constexpr Impl() : p(nullptr) {} T* p; };
template <class T> struct Vec { Vec() = default; Impl<T> i; };
template <class C> int g(C c) { Vec<int> v; return v.i.p == nullptr ? c : 0; }
int main() { std::printf("%d\n", g(7)); }
