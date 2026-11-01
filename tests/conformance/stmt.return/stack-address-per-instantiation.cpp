// [stmt.return]/2, [temp.inst]/4 (WP-231): clang re-checks the return it
// rebuilds in each instantiation, so a non-dependent return of a local's
// address warns in the template definition and once per instantiation;
// a dependent one warns only in instantiations, and an instantiation
// requested twice warns once.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 18:50 warning "address of stack memory associated with local variable 'x' returned"
//@ diag: 20:61 warning "address of stack memory associated with local variable 'y' returned"
//@ diag: 18:50 warning "address of stack memory associated with local variable 'x' returned"
//@ diag: 22:3 note "in instantiation of function template specialization 'f<int>' requested here"
//@ diag: 18:50 warning "address of stack memory associated with local variable 'x' returned"
//@ diag: 22:13 note "in instantiation of function template specialization 'f<long>' requested here"
//@ diag: 19:44 warning "address of stack memory associated with local variable 'x' returned"
//@ diag: 23:3 note "in instantiation of function template specialization 'g<int>' requested here"
//@ diag: 20:61 warning "address of stack memory associated with local variable 'y' returned"
//@ diag: 24:12 note "in instantiation of member function 'S<int>::m' requested here"
template <class T> int* f() { int x = 0; return &x; }
template <class T> T* g() { T x{}; return &x; }
template <class T> struct S { int* m() { int y = 0; return &y; } };
void use() {
  f<int>(); f<long>(); f<int>();
  g<int>();
  S<int>().m();
}
