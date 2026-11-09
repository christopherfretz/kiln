// [class.copy.assign]/2: the implicit copy assignment of F has the form F&
// operator=(const F&) because M's (implicitly deleted) copy assignment takes
// const M&. From clang's CXX/special/class.copy/p18-cxx11.cpp. Found by the
// clang negative-test run (WP-912).
//@ std: c++11
//@ xfail(kiln): WP-912 follow-up, the implicit copy assignment's parameter form ignores a deleted subobject operator
//@ expect: compile-ok
struct M {
  M &operator=(M &&);
};
struct F {
  M a;
};
struct G {
  friend F &F::operator=(const F &) noexcept;
};
