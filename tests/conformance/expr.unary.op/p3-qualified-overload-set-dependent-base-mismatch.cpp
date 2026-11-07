// [expr.unary.op]/3, [over.over]/1-2: `&N::f` naming an overload set
// through a class template with a dependent base is the qualified set,
// checked against its target type in the template definition (WP-715).
//@ std: c++17
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:10 error "variable 'f1' with type 'auto' has incompatible initializer of type '<overloaded function type>'"
//@ diag: 19:29 error "address of overloaded function 'f' does not match required type 'int (long)'"
//@ diag: 12:7 note "candidate function has type mismatch at 1st parameter (expected 'long' but has 'int')"
//@ diag: 13:7 note "candidate function has different number of parameters (expected 1 but has 2)"
struct Int {
  int f(int b) { return b; }
  int f(int b, int c) { return b * c; }
};
template <class P> struct Base {};
template <class P> struct D : Base<P> {
  void g() {
    auto f1 = &Int::f;
    int (Int::*f2)(long) = &Int::f;
  }
};
