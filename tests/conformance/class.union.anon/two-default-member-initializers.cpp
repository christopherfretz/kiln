// [class.union.anon]/1, [class.base.init]/10: at most one variant member of
// an anonymous union may have a default member initializer. Found by the
// clang negative-test run (WP-912).
//@ std: c++20
//@ xfail(kiln): WP-912 follow-up, multiple initialized union members are not diagnosed
//@ expect: compile-fail
//@ diag: 11:9 error "initializing multiple members of union"
struct S {
  union {
    int a = 1;
    int b = 2;
  };
};
