// [conv.ptr]/1 as C++98/03 worded it (before CWG903): any integral constant
// expression of integer type that evaluates to zero is a null pointer
// constant, not only a literal; an enumerator is not one (its type is not
// an integer type). The conversion warns where clang 22 warns: in an
// initialization, argument, return or assignment, not in a comparison or
// a conditional. p1-cxx11-zero-constant.cpp is the other side of the
// boundary. Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ std: c++98
//@ diag: 31:15 warning "expression which evaluates to zero treated as a null pointer constant of type 'int *'"
//@ diag: 32:19 warning "expression which evaluates to zero treated as a null pointer constant of type 'int *const'"
//@ diag: 33:13 warning "expression which evaluates to zero treated as a null pointer constant of type 'int *'"
//@ diag: 34:8 error "cannot initialize a variable of type 'int *' with an rvalue of type 'E'"
//@ diag: 35:13 warning "expression which evaluates to zero treated as a null pointer constant of type 'int *'"
//@ diag: 37:13 warning "initialization of pointer of type 'int *' to null from a constant boolean expression"
//@ diag: 38:15 warning "initialization of pointer of type 'int *' to null from a constant boolean expression"
//@ diag: 39:8 warning "initialization of pointer of type 'int *' to null from a constant boolean expression"
//@ diag: 40:12 warning "initialization of pointer of type 'void *' to null from a constant boolean expression"
//@ diag: 41:7 warning "initialization of pointer of type 'int *' to null from a constant boolean expression"
//@ diag: 46:21 warning "expression which evaluates to zero treated as a null pointer constant of type 'int *'"
//@ diag: 48:49 warning "expression which evaluates to zero treated as a null pointer constant of type 'int *'"
//@ diag: 49:15 note "in instantiation of function template specialization 't<int>' requested here"
void take(int *);
void take3(int, void *, int);
const int kz = 0;
enum E { e0 };
struct S { int m; };
void mem(int S::*);
void f(int *a, bool c) {
  int *p2 = 1 - 1;
  int *const p3 = sizeof(*p3) ? 0 : 0;
  int *p4 = kz;
  int *p5 = e0;
  int *p6 = (char)0;
  int *p7 = 0L;
  int *b1 = false;
  int *b2 = 1 == 0;
  take(false);
  take3(0, false, 2);
  a = false;
  bool k1 = a == kz, k2 = kz != a;
  int *k3 = c ? a : kz;
  mem(1 - 1);
}
int *ret() { return kz; }
int *cast() { return (int *)(1 - 1); }
template <class T> void t(T *p) { p = sizeof(T) - sizeof(T); }
template void t<int>(int *);
