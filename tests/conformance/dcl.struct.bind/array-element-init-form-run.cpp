// [dcl.struct.bind]/1: `auto [a, b] = arr;` copy-initializes each element of
// the unnamed variable from the corresponding element of `arr`, and
// `auto [a, b](arr);` direct-initializes it, "as specified by the form of
// the initializer". So an explicit copy constructor is not a candidate for
// `= arr` ([over.match.copy]/1.2) and the converting constructor template is
// used, while `(arr)` uses the copy constructor. kiln used the copy
// constructor for every form (found running GCC's C++ testsuite). Counts per
// standard mode (structured bindings are an extension before C++17).
//@ expect: run
//@ std: c++17
//@ stdout: copy-init: ctor 2 copy 0 template 2
//@ stdout: direct-init: ctor 2 copy 2 template 0
//@ stdout: reference: ctor 2 copy 0 template 0
//@ stdout: nested: ctor 4 copy 0 template 4
//@ stdout: values: 7 8
extern "C" int printf(const char *, ...);
int ctor, copy, tmpl;
struct A {
  A() : a(6) { ++ctor; }
  explicit A(const A &x) : a(x.a) { ++copy; }
  template <class T>
  A(const T &x) : a(x.a + 1) { ++tmpl; }
  int a;
};
static void report(const char *what) {
  printf("%s: ctor %d copy %d template %d\n", what, ctor, copy, tmpl);
  ctor = copy = tmpl = 0;
}
int main() {
  int x = 0, y = 0;
  {
    A arr[2];
    auto [a, b] = arr;
    x = a.a;
    report("copy-init");
  }
  {
    A arr[2];
    auto [a, b](arr);
    y = b.a + 2;
    report("direct-init");
  }
  {
    A arr[2];
    auto &[a, b] = arr;
    report("reference");
    (void)a, (void)b;
  }
  {
    A m[2][2];
    auto [r0, r1] = m;
    report("nested");
    (void)r0, (void)r1;
  }
  printf("values: %d %d\n", x, y);
}
