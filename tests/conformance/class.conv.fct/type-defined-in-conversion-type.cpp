// [dcl.fct]/17, [class.conv.fct]/1: types shall not be defined in a
// conversion-type-id. Found by the clang negative-test run (WP-912).
//@ std: c++20
//@ xfail(kiln): WP-912 follow-up, types defined in a type specifier context are not diagnosed
//@ expect: compile-fail
//@ diag: 8:19 error "'K::S' cannot be defined in a type specifier"
struct K {
  operator struct S {}();
};
