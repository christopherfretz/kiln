// [expr.context]/2 is C++11 wording: before it a discarded volatile
// glvalue undergoes no lvalue-to-rvalue conversion, so an incomplete class
// is not required to be complete (kiln and clang follow C++03 here; GCC 16
// instantiates the template anyway). Found running GCC's C++ testsuite.
//@ expect: compile-ok
//@ std: c++98
//@ args: -Wno-unused-value -Wno-unused-volatile-lvalue
struct I;
template <class T, int> struct H { T t; };
extern volatile H<I, 0> &a;
extern volatile I *p;
void t() {
  a;
  *p;
}
