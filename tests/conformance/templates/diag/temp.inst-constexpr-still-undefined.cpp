// [temp.inst]/8, [expr.const]/10.3: a member function of a specialization
// used before its pattern is defined is an undefined function to a
// constant expression evaluated before the definition, even though the
// Oven may instantiate it on demand once the pattern is defined
// (temp.inst-constexpr-defined-later.cpp; WP-484).
//@ expect: compile-fail
//@ oracle: gcc
//@ strict-diags
//@ diag: 17:15 error "static assertion expression is not an integral constant expression"
//@ diag: 15:37 note "undefined function 'f' cannot be used in a constant expression"
//@ diag: 17:15 note "in call to 'g()'"
//@ diag: 13:52 note "declared here"
template <class T> struct S { static constexpr int f(); };
struct X : S<int> {
  static constexpr int g() { return f(); }
};
static_assert(X::g() == 42);
template <class T> constexpr int S<T>::f() { return 42; }
int main() {}
