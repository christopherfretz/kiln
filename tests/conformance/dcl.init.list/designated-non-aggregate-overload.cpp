// [over.ics.list]/2: a designated-initializer-list argument converts only
// to an aggregate parameter; a non-aggregate class parameter is not viable.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:3 error "no matching function for call to 'f'"
//@ diag: 9:6 note "candidate function not viable: cannot convert initializer list argument to 'C'"
struct C { C(int); int x; };
struct A { int x; };
void f(C);
void f2(C);
void f2(A);
void g() {
  f({.x = 1});
  f2({.x = 1});
}
