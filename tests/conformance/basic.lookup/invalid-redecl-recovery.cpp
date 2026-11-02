// [basic.lookup.general], [stmt.ambig]: `R::f(x);` with no type `R::f` is
// a declaration of `x` with an unknown type, which clang reports and makes
// invalid. Lookup ignores an invalid declaration unless it is the only one
// found (clang `LookupResult::resolveKind`), so it does not hide the valid
// `x` of the same block scope and the later errors about that `x` are
// still reported. A name with no other declaration (`z`) finds the invalid
// declaration, and its uses are silent.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:6 error "no type named 'f' in namespace 'R'"
//@ diag: 20:3 error "no matching function for call to 'g'"
//@ diag: 16:33 note "candidate function not viable: requires 1 argument, but 2 were provided"
//@ diag: 21:7 error "no viable conversion from 'R::X' to 'int'"
//@ diag: 24:6 error "no type named 'f' in namespace 'R'"
//@ diag: 28:6 error "no type named 'f' in namespace 'R'"
namespace R { struct X {}; void g(X); }
void test() {
  R::X x;
  R::f(x);
  g(x, 1);
  int y = x;
}
void test2() {
  R::f(z);
  z = 1;
}
void test3() {
  R::f(w) = 3;
  w = 4;
}
