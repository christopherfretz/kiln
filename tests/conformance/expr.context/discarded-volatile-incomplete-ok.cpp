// [expr.context]/2, the accepted side: a discarded glvalue of an incomplete
// class type that is not volatile, or is not one of the listed forms (a
// call returning a reference), undergoes no lvalue-to-rvalue conversion, so
// no complete type is needed; a volatile complete class is fine. Found
// running GCC's C++ testsuite.
//@ expect: compile-ok
//@ args: -Wno-unused-value -Wno-unused-volatile-lvalue
struct I;
struct C { int i; };
extern I *q;
extern volatile I &h();
extern volatile C vc;
template <class T, int> struct H { T t; };
extern H<I, 0> &a;
void t() {
  *q;
  (void)*q;
  h();
  vc;
  a;
}
