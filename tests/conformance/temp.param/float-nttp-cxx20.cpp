// [temp.param]/6 (C++20): floating-point and literal class types with
// public, non-mutable members are structural and may be the type of a
// non-type template parameter (the C++20 counterpart of float-nttp-cxx17.cpp,
// WP-1017).
//@ std: c++20
//@ expect: compile-ok
//@ strict-diags
template <float F> struct S {};
S<1.5f> s;
using FT = double;
template <FT D> constexpr double f() { return D; }
static_assert(f<2.0>() == 2.0);
struct L {
  int i;
};
template <L l> struct T {};
T<L{1}> t;
template <auto A> struct V {};
V<1.0> v;
