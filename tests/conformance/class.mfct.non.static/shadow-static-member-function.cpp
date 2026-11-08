// [class.mfct.non.static]: a field is not hidden inside an explicit object
// member function or a lambda in a static member function (clang 22's
// rule; clang 19 warns, see shadow-static-member-function-19.cpp).
//@ verify
//@ args: -Wshadow-all
//@ requires: clang>=22

// expected-no-diagnostics
struct X {
  int x;
  void self(this X s, int x) { (void)s; (void)x; }
  static void st() { auto l = [] { int x = 0; return x; }; (void)l; }
};
