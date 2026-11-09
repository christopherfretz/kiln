// [dcl.fct]/17, [class.conv.fct]/1: types shall not be defined in a
// conversion-type-id. Found by the clang negative-test run (WP-912). clang
// names the tag with its class scope.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:19 error "'K::S' cannot be defined in a type specifier"
//@ diag: 15:17 error "'K2::E' cannot be defined in a type specifier"
//@ diag: 16:18 error "'K2::U' cannot be defined in a type specifier"
//@ diag: 17:28 error "'K2::T' cannot be defined in a type specifier"
struct K {
  operator struct S {}();
};
struct K2 {
  operator enum E { e }();
  operator union U { int a; } *();
  explicit operator struct T { int b; } &();
};
