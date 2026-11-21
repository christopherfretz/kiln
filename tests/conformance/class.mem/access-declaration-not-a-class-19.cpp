// The clang 19 form of access-declaration-not-a-class.cpp: clang 19 prints
// the qualifier with its `::` (`'R::'`).
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang<22
//@ diag: 10:6 error "ISO C++11 does not allow access declarations; use using declarations instead"
//@ diag: 10:3 error "using declaration in class refers into 'R::', which is not a class"
namespace R { int f(int); int g; }
struct T {
  R::g;
  int ok;
};
int after(T t) { return t.ok; }
