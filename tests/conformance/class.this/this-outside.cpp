// [expr.prim.this]/3: 'this' is usable only in a non-static member function
// or a default member initializer.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:27 error "invalid use of 'this' outside of a non-static member function"
//@ diag: 13:11 error "invalid use of 'this' outside of a non-static member function"
//@ xfail(kiln): needs WP-065 parser classes
struct S {
  int x;
  static int f() { return this->x; }
  int y = this->x + 1;
};
void *p = this;
