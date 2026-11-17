// Before C++23 a static `operator()` or `operator[]` is an extension
// (P1169, P2589), and `operator[]` takes exactly one parameter with no
// default argument (P2128).
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ args: -std=c++20
//@ strict-diags
//@ diag: 14:14 warning "declaring overloaded 'operator()' as 'static' is a C++23 extension [-Wc++23-extensions]"
//@ diag: 15:14 warning "declaring overloaded 'operator[]' as 'static' is a C++23 extension [-Wc++23-extensions]"
//@ diag: 19:7 error "overloaded 'operator[]' cannot have no parameter before C++23"
//@ diag: 20:7 error "overloaded 'operator[]' cannot have more than one parameter before C++23"
//@ diag: 21:7 error "overloaded 'operator[]' cannot have a defaulted parameter before C++23"
struct Static {
  static int operator()(int);
  static int operator[](int);
};

struct Subscript {
  int operator[]();
  int operator[](int, int);
  int operator[](int = 0);
  int operator[](long);
};
