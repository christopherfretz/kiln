// [class.union.general]/4 note, [class.base.init]/10: at most one variant
// member of a union may have a default member initializer. Found by the
// clang negative-test run (WP-912).
//@ std: c++20
//@ expect: compile-fail
//@ diag: 10:8 error "initializing multiple members of union"
//@ diag: 9:7 note "previous initialization is here"
union U {
  int a = 0;
  char b = 'x';
};
