// [stmt.expand]/5.3: a type that is neither iterable nor decomposable is
// diagnosed as by a structured binding declaration.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=23
//@ diag: 13:24 error "non-class, non-array type 'int'"
//@ diag: 15:24 error "private member 'b' of 'E'"
//@ diag: 14:34 note "declared private here"
// clang 22 says "cannot bind", clang 19 "cannot decompose"; kiln follows
// its -fclang-compat level, so the directives match the common suffix.
void f() {
  int s = 0;
  template for (auto x : 5) { s += x; }
  struct E { int a; private: int b; };
  template for (auto x : E{}) { s += x; }
}
