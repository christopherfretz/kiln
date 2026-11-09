// [dcl.fct.def.default]/2, [class.copy.assign]/1: a copy assignment operator
// with an explicit object parameter may be defaulted (clang 22, GH142835;
// clang 19 crashes). From clang's SemaCXX/cxx2b-deducing-this-constexpr.cpp.
// Found by the clang negative-test run (WP-912).
//@ std: c++23
//@ requires: clang>=22
//@ xfail(kiln): WP-912 follow-up, a defaulted explicit-object copy assignment is not a special member
//@ expect: compile-ok
struct S {
  S &operator=(this S &, const S &) = default;
  int v = 1;
};
void g(S &a, const S &b) { a = b; }
