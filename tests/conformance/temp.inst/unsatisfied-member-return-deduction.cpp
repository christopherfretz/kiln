// [expr.prim.id.general]/4, [temp.inst]/5: a static member function with
// a placeholder return type whose requires-clause is not satisfied is
// not viable, so `A<T>::r()` in a requires-expression is an unsatisfied
// requirement and the return type is never deduced (its body is not
// instantiated). Found running GCC's C++ testsuite (WP-1472).
//@ expect: compile-ok
//@ std: c++20
//@ xfail(kiln): WP-1497, the qualified call deduces the return type before checking the constraints
template <class T> struct A {
  static auto r() requires false { return T::fail; }
};
template <class T> concept C = requires { A<T>::r(); };
static_assert(!C<int>);
