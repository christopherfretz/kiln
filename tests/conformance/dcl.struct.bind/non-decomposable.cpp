//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:8 error "cannot decompose union type 'U'"
//@ diag: 22:8 error "cannot decompose non-class, non-array type 'int'"
//@ diag: 23:8 error "cannot decompose non-class, non-array type 'int *'"
//@ diag: 24:8 error "cannot decompose class type 'D': both it and its base class 'B' have non-static data members"
//@ diag: 26:8 error "cannot decompose class type 'AU' because it has an anonymous union member"
//@ diag: 19:13 note "declared here"
//@ requires: clang<22
// clang 22 emits error: cannot bind union type 'U'; WP-355 residual
// [dcl.struct.bind]/5 (C++26 /7): a union, a non-class non-array type, a
// class whose data members are not all direct members of one class, or a
// class with an anonymous union member cannot be decomposed.
union U { int a; };
struct B { int x; };
struct D : B { int y; };
struct E : B {};
struct F : E {};
struct AU { union { int u; }; int w; };
void f(U u, int i, int *p, D d, F ff, AU au) {
  auto [a] = u;
  auto [b] = i;
  auto [c] = p;
  auto [dx, dy] = d;
  auto [fx] = ff;
  auto [au1, au2] = au;
}
