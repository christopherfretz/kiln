// [temp.func.order]/3, /6.2.2, CWG2834 (open): what stays ambiguous when an
// implicit object member function template, which gets X(M), a reference
// to its class, is ordered against an explicit object parameter or a
// non-member operator's first parameter of another type (`this V` by
// value), and when a static member is ordered against `this auto` or
// `this Self &&` (the object parameter is not compared; the template
// parameter lists differ in length, so the constraints do not decide).
// clang 22 and GCC 16.1 report each call ambiguous. Accepted siblings:
// `p3-implicit-against-explicit-object-run.cpp`,
// `p3-static-against-explicit-object-run.cpp`. Found running GCC's C++
// testsuite (WP-1670). clang 19 picks the static member in the last two.
//@ std: c++23
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 58:7 error "conversion from 'CV' to 'int' is ambiguous"
//@ diag: 54:18 note "candidate function [with T = int]"
//@ diag: 55:22 note "candidate function [with T = int]"
//@ diag: 59:12 error "call to member function 'f' is ambiguous"
//@ diag: 36:7 note "candidate function [with auto:1 = int]"
//@ diag: 37:7 note "candidate function [with auto:1 = int]"
//@ diag: 59:21 error "call to member function 'g' is ambiguous"
//@ diag: 38:7 note "candidate function [with auto:1 = int]"
//@ diag: 39:7 note "candidate function [with auto:1 = int]"
//@ diag: 59:31 error "call to member function 'f' is ambiguous"
//@ diag: 42:14 note "candidate function [with auto:1 = int]"
//@ diag: 43:7 note "candidate function [with auto:1 = SA, auto:2 = int]"
//@ diag: 59:41 error "call to member function 'f' is ambiguous"
//@ diag: 46:14 note "candidate function [with auto:1 = int]"
//@ diag: 47:29 note "candidate function [with Self = SS &, auto:2 = int]"
//@ diag: 59:51 error "use of overloaded operator '+' is ambiguous (with operand types 'N' and 'int')"
//@ diag: 50:7 note "candidate function [with auto:1 = int]"
//@ diag: 52:5 note "candidate function [with auto:1 = int]"
template <class T> concept C = true;
struct V {
  int f(C auto) { return 1; }
  int f(this V, auto) { return 2; }
  int g(auto) { return 1; }
  int g(this V, C auto) { return 2; }
};
struct SA {
  static int f(C auto) { return 3; }
  int f(this auto, auto) { return 2; }
};
struct SS {
  static int f(auto) { return 3; }
  template <class Self> int f(this Self &&, C auto) { return 2; }
};
struct N {
  int operator+(C auto) { return 1; }
};
int operator+(N, auto) { return 4; }
struct CV {
  template <C T> operator T() { return 1; }
  template <class T> operator T(this CV) { return 2; }
};
int use(V v, SA sa, SS ss, N n, CV cv) {
  int r = cv;
  return v.f(0) + v.g(0) + sa.f(0) + ss.f(0) + (n + 0) + r;
}
