// C11 6.5.1.1p3 / [expr.prim] (_Generic in C++, a clang extension): the
// result is the selected association itself, so a literal 0 selected is a
// null pointer constant, a selected bit-field cannot bind a non-const
// reference, an overload set stays unresolved, and an unused result draws
// -Wunused-value / -Wunused-comparison; the controlling expression is
// unevaluated (no -Wuninitialized), and a type operand T() is the
// function type 'int ()' at instantiation.
//@ expect: compile-fail
//@ args: -Wall
//@ strict-diags
//@ diag: 28:8 error "cannot initialize a variable of type 'int *' with an rvalue of type 'void *'"
//@ diag: 29:8 error "non-const reference cannot bind to bit-field 'b'"
//@ diag: 22:16 note "bit-field is declared here"
//@ diag: 30:8 error "variable 'pf' with type 'auto' has incompatible initializer of type '<overloaded function type>'"
//@ diag: 36:23 warning "equality comparison result unused"
//@ diag: 36:23 note "use '=' to turn this equality comparison into an assignment"
//@ diag: 25:20 warning "expression result unused"
//@ diag: 26:22 warning "expression result unused"
//@ diag: 38:40 error "controlling expression type 'int ()' not compatible with any generic association type"
//@ diag: 39:15 note "in instantiation of function template specialization 't<int>' requested here"
int x;
struct B { int b : 3; } bs;
void ovl(int); void ovl(long);
void f() {
  _Generic(x, int: 1);
  _Generic(x, int: x + 1, default: 0);
  int *p = _Generic(x, int: 0, default: nullptr);
  int *q = _Generic(x, int: (void*)0, default: nullptr);
  int &r = _Generic(x, int: bs.b);
  auto pf = _Generic(1, int: ovl, default: 0);
  (void)p; (void)q; (void)pf;
  int u;
  int v = _Generic(u, int: 1);
  int w = _Generic(x, int: u);
  (void)v; (void)w;
  _Generic(x, int: x) == 1;
}
template <class T> void t() { _Generic(T(), int: 1); }
template void t<int>();
