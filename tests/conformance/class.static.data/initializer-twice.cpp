// [class.static.data]/4: a const static data member with an in-class
// initializer shall not be given an initializer again at its definition.
// Found by the clang negative-test run (WP-912).
//@ std: c++20
//@ expect: compile-fail
//@ diag: 11:18 error "static data member 'i' already has an initializer"
//@ diag: 9:24 note "previous initialization is here"
struct S {
  static const int i = 1;
};
const int S::i = 2;
