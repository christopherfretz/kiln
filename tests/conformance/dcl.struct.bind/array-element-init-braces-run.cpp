// [dcl.struct.bind]/1: `auto [a, b]{arr};` direct-initializes each element
// (the form of the initializer is direct-list-initialization), so the
// explicit copy constructor is used, not the converting constructor
// template. GCC 16.1 and clang 22 agree; clang 19 copy-initializes (found
// running GCC's C++ testsuite).
//@ expect: run
//@ std: c++17
//@ requires: clang>=22
//@ stdout: ctor 2 copy 2 template 0
extern "C" int printf(const char *, ...);
int ctor, copy, tmpl;
struct A {
  A() { ++ctor; }
  explicit A(const A &) { ++copy; }
  template <class T>
  A(const T &) { ++tmpl; }
};
int main() {
  A arr[2];
  auto [a, b]{arr};
  (void)a, (void)b;
  printf("ctor %d copy %d template %d\n", ctor, copy, tmpl);
}
