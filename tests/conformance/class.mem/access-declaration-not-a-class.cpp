// [namespace.udecl]/3, [diff.cpp03.dcl.dcl]: an access declaration
// (`R::g;`, removed in C++11) is diagnosed as clang does, and one naming a
// member of a namespace is also a using-declaration in a class that does
// not refer into a class; later members are unaffected.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 12:6 error "ISO C++11 does not allow access declarations; use using declarations instead"
//@ diag: 12:3 error "using declaration in class refers into 'R', which is not a class"
namespace R { int f(int); int g; }
struct T {
  R::g;
  int ok;
};
int after(T t) { return t.ok; }
