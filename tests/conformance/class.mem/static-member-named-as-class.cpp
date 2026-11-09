// [class.mem.general]/14: every static data member shall have a name
// different from its class. Found by the clang negative-test run (WP-912).
//@ std: c++20
//@ xfail(kiln): WP-912 follow-up, a member with the class's name is not diagnosed
//@ expect: compile-fail
//@ diag: 8:14 error "member 'X' has the same name as its class"
struct X {
  static int X;
};
