// [conv.ptr]/1 (CWG903, C++11 on): only an integer literal with value zero
// or a prvalue of type std::nullptr_t is a null pointer constant, so the
// zero-valued constant expressions C++98 accepted
// (p1-cxx98-zero-constant.cpp) no longer convert to a pointer. Found
// running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ std: c++11
//@ diag: 21:8 error "cannot initialize a variable of type 'int *' with an rvalue of type 'int'"
//@ diag: 22:8 error "cannot initialize a variable of type 'int *' with an lvalue of type 'const int'"
//@ diag: 23:8 error "cannot initialize a variable of type 'int *' with an rvalue of type 'bool'"
//@ diag: 24:3 error "no matching function for call to 'take'"
//@ diag: 16:6 note "candidate function not viable: no known conversion from 'bool' to 'int *' for 1st argument"
//@ diag: 25:7 error "incompatible integer to pointer conversion assigning to 'int *' from 'bool'"
//@ diag: 26:15 error "comparison between pointer and integer ('int *' and 'int')"
void take(int *);
const int kz = 0;
void f(int *a) {
  int *p0 = 0;
  int *q0 = (0);
  int *p2 = 1 - 1;
  int *p4 = kz;
  int *b1 = false;
  take(false);
  a = false;
  bool k1 = a == kz;
}
