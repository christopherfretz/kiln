// C++14 [except.spec]/1-2, [depr.except.spec] (C++11/14): a dynamic
// exception specification with a type-id-list is valid, deprecated,
// before C++17 (clang's -Wdeprecated-dynamic-exception-spec is off by
// default); only C++17 removed it ([diff.cpp14.except]). In a template
// its type-id-list is substituted ([temp.inst]). kiln reported the C++17
// error in every mode and could not instantiate a template carrying one
// (Eigen's tests in C++11 mode with EIGEN_MAX_CPP_VER=03 give a class
// template's destructor `throw(X)`).
//@ std: c++11
//@ expect: run
//@ strict-diags
//@ stdout: 3
#include <cstdio>
struct E {};
struct A {
  ~A() throw(E) {}
  int f() throw(E, int) { return 1; }
};
int g() throw(E);
int g() throw(E) { return 1; }
template <class T> T h(T t) throw(T) { return t; }
template <class T> struct C {
  ~C() throw(E) {}
  T get() throw(T, E) { return T(1); }
};
int main() {
  A a;
  C<int> c;
  std::printf("%d\n", a.f() + h(g()) + c.get() - 0);
}
