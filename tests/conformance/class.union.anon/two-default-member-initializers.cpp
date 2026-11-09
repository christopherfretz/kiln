// [class.union.anon]/1, [class.base.init]/10: at most one variant member of
// an anonymous union may have a default member initializer. Found by the
// clang negative-test run (WP-912).
//@ std: c++20
//@ expect: compile-fail
//@ diag: 11:9 error "initializing multiple members of union"
//@ diag: 10:9 note "previous initialization is here"
struct S {
  union {
    int a = 1;
    int b = 2;
  };
};
