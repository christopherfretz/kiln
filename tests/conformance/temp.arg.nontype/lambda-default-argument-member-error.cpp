// [temp.param]/15: an ill-formed body of a defaulted lambda in a member
// template's head is diagnosed when the default is used, with the
// instantiation note (WP-1497).
//@ expect: compile-fail
//@ std: c++20
//@ diag: 9:43 error "member reference base type 'int' is not a structure or union"
//@ diag: 11:17 note "in instantiation of default argument for"
template <class T> struct A {
  template <auto N, auto F = [] { return N.n; }> struct C { static constexpr int v = F(); };
};
int x = A<int>::C<1>::v;
