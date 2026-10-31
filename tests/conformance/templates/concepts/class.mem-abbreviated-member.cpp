//@ expect: compile-ok
// [dcl.fct]/22: an abbreviated member function template.
template <class T> concept Int = __is_same(T, int);
struct S {
  static constexpr int f(Int auto) { return 1; }
  static constexpr int f(auto) { return 2; }
};
static_assert(S::f(1) == 1);
static_assert(S::f('c') == 2);
