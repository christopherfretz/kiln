// [expr.prim.id.unqual]/7, [basic.contract.general]/4 (P2900R14): inside a
// contract predicate an id-expression naming a variable declared outside
// it (a parameter, a local, a static local, a namespace-scope variable,
// including one of reference type) is a const lvalue, so modifying it is
// ill-formed; a pointer's pointee is not const. GCC 16.1 rejects the same
// expressions.
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
//@ diag: 19:20 error "cannot modify 'x' in a contract predicate: it is implicitly const"
//@ diag: 20:24 error "cannot modify 'x' in a contract predicate: it is implicitly const"
//@ diag: 21:34 error "cannot modify 'p' in a contract predicate: it is implicitly const"
//@ diag: 22:19 error "cannot modify 'glob' in a contract predicate: it is implicitly const"
//@ diag: 26:19 error "cannot modify 'ra' in a contract predicate: it is implicitly const"
//@ diag: 27:19 error "cannot modify 's' in a contract predicate: it is implicitly const"
//@ diag: 28:19 error "cannot modify 'a' in a contract predicate: it is implicitly const"
int glob;
void m1(int x) pre(++x);
void m2(int& x) pre((x = 1) != 0);
void m3(int* p) pre(++*p) pre((p = nullptr) == nullptr);
void m4() pre(glob++ > 0);
void m5(int a) {
  int& ra = a;
  static int s;
  contract_assert(++ra);
  contract_assert(++s);
  contract_assert(--a);
}
