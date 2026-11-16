// [temp.inst]/4, /8: naming a constexpr static data member of a class
// template specialization in an unevaluated operand (`decltype`) needs
// only its declaration, so its initializer `T::nonexistent` is not
// instantiated. Found running GCC's C++ testsuite (WP-1472).
//@ expect: compile-ok
//@ std: c++20
template <class T> struct A { static constexpr int value = T::nonexistent; };
using type = const int;
using type = decltype(A<int>::value);
template <class T> constexpr int v = T::nonexistent;
using type = decltype(v<int>);
