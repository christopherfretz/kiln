// [over.over]/1, [expr.ref]/7.3.2: an overload set names a function only
// where a target type selects one, and a non-static member function named
// through an object can only be called. A for-statement's increment is a
// discarded-value expression like an expression statement, and neither
// operand of a conditional expression is a target, so both are errors
// there, also in a template and at instantiation.
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ args: -std=c++20
//@ diag: 39:12 error "reference to non-static member function must be called; did you mean to call it with no arguments?"
//@ diag: 40:10 error "reference to overloaded function could not be resolved; did you mean to call it?"
//@ diag: 37:6 note "possible target for call"
//@ diag: 36:6 note "possible target for call"
//@ diag: 41:12 error "reference to non-static member function must be called"
//@ diag: 35:27 note "possible target for call"
//@ diag: 35:40 note "possible target for call"
//@ diag: 42:7 error "reference to overloaded function could not be resolved; did you mean to call it?"
//@ diag: 37:6 note "possible target for call"
//@ diag: 36:6 note "possible target for call"
//@ diag: 43:9 error "reference to non-static member function must be called"
//@ diag: 35:27 note "possible target for call"
//@ diag: 35:40 note "possible target for call"
//@ diag: 44:25 error "reference to overloaded function could not be resolved; did you mean to call it?"
//@ diag: 37:6 note "possible target for call"
//@ diag: 36:6 note "possible target for call"
//@ diag: 47:10 error "reference to overloaded function could not be resolved; did you mean to call it?"
//@ diag: 37:6 note "possible target for call"
//@ diag: 36:6 note "possible target for call"
//@ diag: 49:7 error "reference to overloaded function could not be resolved; did you mean to call it?"
//@ diag: 37:6 note "possible target for call"
//@ diag: 36:6 note "possible target for call"
//@ diag: 48:12 error "reference to non-static member function must be called; did you mean to call it with no arguments?"
//@ diag: 51:15 note "in instantiation of function template specialization 't<S>' requested here"
struct S { void f(); void o(int); void o(double); };
void g(int);
void g(double);
void h(bool b, S s) {
  for (;;s.f) break;
  for (;;g) break;
  for (;;s.o) break;
  b ? g : g;
  b ? s.o : s.o;
  void (*fp)(int) = b ? g : g;
}
template <class T> void t(bool b, T x) {
  for (;;g) break;
  for (;;x.f) break;
  b ? g : g;
}
template void t(bool, S);
