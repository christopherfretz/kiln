// [stmt.expand]/5.3: a type that is neither iterable nor decomposable is
// diagnosed as by a structured binding declaration.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=23
//@ diag: 11:24 error "cannot bind non-class, non-array type 'int'"
//@ diag: 13:24 error "cannot bind private member 'b' of 'E'"
//@ diag: 12:34 note "declared private here"
void f() {
  int s = 0;
  template for (auto x : 5) { s += x; }
  struct E { int a; private: int b; };
  template for (auto x : E{}) { s += x; }
}
