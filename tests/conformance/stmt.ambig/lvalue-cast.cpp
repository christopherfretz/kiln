// [stmt.ambig]: `int(a)++;` is an expression (`++` cannot follow a
// declarator); the functional cast is a prvalue, so the operand is not
// modifiable. clang names the GCC cast-as-lvalue extension it rejects.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:3 error "assignment to cast is illegal, lvalue casts are not supported"
//@ diag: 19:3 error "assignment to cast is illegal, lvalue casts are not supported"
//@ diag: 20:3 error "assignment to cast is illegal, lvalue casts are not supported"
//@ diag: 21:5 error "assignment to cast is illegal, lvalue casts are not supported"
//@ diag: 22:4 error "assignment to cast is illegal, lvalue casts are not supported"
//@ diag: 23:10 error "expression is not assignable"
//@ diag: 24:3 error "assignment to cast is illegal, lvalue casts are not supported"
//@ diag: 25:3 error "assignment to cast is illegal, lvalue casts are not supported"
//@ diag: 26:9 error "expression is not assignable"
//@ diag: 28:24 error "expression is not assignable"
int a; struct S { int m; } s;
void f() {
  int(a)++;
  (int)a = 1;
  static_cast<int>(a) = 2;
  ++(int)a;
  ((int)(a)) += 3;
  (int)5 = 1;
  int(s.m) = 4;
  (long)a = 5;
  a + 1 = 2;
  (int&)a = 6;
  const_cast<int&&>(a) = 7;
}
