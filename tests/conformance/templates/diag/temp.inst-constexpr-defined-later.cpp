// [temp.inst]/8, [temp.point]/1, [expr.const]/10.3: a member function of a
// specialization that is used before its pattern is defined is not
// instantiated by the constant evaluator: inside a constant expression
// evaluated before the definition it is an undefined function.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:17 error "static assertion expression is not an integral constant expression"
//@ diag: 13:37 note "undefined function 'f' cannot be used in a constant expression"
//@ diag: 18:17 note "in call to 'g()'"
//@ diag: 11:52 note "declared here"
template <class T> struct S { static constexpr int f(); };
struct X : S<int> {
  static constexpr int g() { return f(); }
  static constexpr int h();
};
template <class T> constexpr int S<T>::f() { return 42; }
constexpr int X::h() {
  static_assert(g() == 42);
  static_assert(f() == 42);
  return 0;
}
