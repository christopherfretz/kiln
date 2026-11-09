// [class.mem.general]/5: under -pedantic-errors a member class redeclared
// in its member-specification is an error.
//@ std: c++20
//@ args: -pedantic-errors
//@ expect: compile-fail
//@ diag: 10:10 error "class member cannot be redeclared"
//@ diag: 9:10 note "previous declaration is here"
struct A {
  struct B {};
  struct B;
};
