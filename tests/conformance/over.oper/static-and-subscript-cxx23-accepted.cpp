// In C++23 a static `operator()` or `operator[]` is valid, and
// `operator[]` may take any number of parameters, defaulted or not.
// Found running GCC's C++ testsuite.
//@ expect: compile-ok
//@ args: -std=c++23
//@ strict-diags
struct Static {
  static int operator()(int x) { return x; }
  static int operator[](int x) { return x; }
};

struct Subscript {
  int operator[]() { return 0; }
  int operator[](int a, int b) { return a + b; }
  int operator[](long = 0) { return 1; }
};

int use(Subscript s) { return Static{}(1) + Static{}[2] + s[1, 2] + s[3L]; }
