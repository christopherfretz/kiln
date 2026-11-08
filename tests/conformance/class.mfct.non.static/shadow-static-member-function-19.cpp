// [class.mfct.non.static]: clang 19 checks only whether the declaration's
// own context is a static member function, so an explicit object member
// function parameter and a lambda local in a static member function still
// warn (clang 22 does not, see shadow-static-member-function.cpp).
//@ verify
//@ args: -Wshadow-all
//@ requires: clang<22

struct X {
  int x; // expected-note {{previous declaration is here}} expected-note {{previous declaration is here}}
  void self(this X s, int x) { (void)s; (void)x; } // expected-warning {{declaration shadows a field of 'X'}}
  static void st() { auto l = [] { int x = 0; return x; }; (void)l; } // expected-warning {{declaration shadows a field of 'X'}}
};
