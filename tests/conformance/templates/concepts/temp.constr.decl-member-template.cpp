//@ expect: compile-ok
// [temp.constr.decl]: a constrained member function template.
struct S {
  template <class T> requires (sizeof(T) == 1) static constexpr int f(T) { return 1; }
  template <class T> requires (sizeof(T) != 1) static constexpr int f(T) { return 2; }
};
static_assert(S::f('c') == 1);
static_assert(S::f(1) == 2);
