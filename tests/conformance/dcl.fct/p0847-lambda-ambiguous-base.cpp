// [expr.prim.lambda.closure]/5 (C++23, P0847R7): the explicit object
// parameter of a lambda with a lambda-capture must be of the closure
// type or of a class derived from it unambiguously; a call whose deduced
// object type has the closure as an ambiguous base is diagnosed with the
// paths (the call operator is found unambiguously through a
// using-declaration).
//@ expect: compile-fail
//@ std: c++23
//@ strict-diags
//@ diag: 20:3 error "is inaccessible due to ambiguity:"
//@ diag: 27:3 error "is inaccessible due to ambiguity:"
void f() {
  int x = 0;
  auto l = [x](this auto self) { return x; };
  using L = decltype(l);
  struct A : L {};
  struct B : L {};
  struct D : A, B { using A::operator(); };
  D d{{l}, {l}};
  d();
  auto m = [x](this auto&& self) { return x; };
  using M = decltype(m);
  struct E : M {};
  struct F : M {};
  struct G : E, F { using E::operator(); };
  G g{{m}, {m}};
  g();
  l();
  m();
}
