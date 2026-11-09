// [class.mem.general]/14: every static data member shall have a name
// different from its class. Found by the clang negative-test run (WP-912).
//@ std: c++20
//@ expect: compile-fail
//@ diag: 7:14 error "member 'X' has the same name as its class"
struct X {
  static int X;
};
