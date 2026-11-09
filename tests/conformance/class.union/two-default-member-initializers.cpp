// [class.union.general]/4 note, [class.base.init]/10: at most one variant
// member of a union may have a default member initializer. Found by the
// clang negative-test run (WP-912).
//@ std: c++20
//@ xfail(kiln): WP-912 follow-up, multiple initialized union members are not diagnosed
//@ expect: compile-fail
//@ diag: 10:8 error "initializing multiple members of union"
union U {
  int a = 0;
  char b = 'x';
};
