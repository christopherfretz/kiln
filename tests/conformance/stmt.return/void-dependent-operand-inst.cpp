// [stmt.return]/2: a type-dependent operand in a void function template
// is checked when the template is instantiated: a non-void operand is
// then ill-formed. WP-128.
//@ expect: compile-fail
//@ diag: 11:3 error "should not return a value"
//@ diag: 13:12 note "in instantiation of function template specialization"

template <class T> T g(T t) { return t; }
template <class T> void ok(T t) { return (void)g(t); }
template <class T> void f(T t) {
  return g(t);
}
void d() { f(1); ok(2); }
