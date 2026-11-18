// C++11 [class.copy]/26: an implicitly-defined copy or move assignment
// operator is not constexpr before C++14 (N3652), so calling a trivial
// one makes a constant expression ill-formed; kiln evaluated it as a
// copy in every mode. Same error as clang 22 and GCC 16.1 (the note's
// column differs: kiln notes an overloaded operator call at its start).
// From C++14 on it is constexpr (class.copy.assign/trivial-assignment-constexpr-cxx14.cpp).
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ args: -std=c++11
//@ diag: 21:15 error "constexpr variable 't' must be initialized by a constant expression"
//@ diag: 22:15 error "constexpr variable 'u' must be initialized by a constant expression"

struct T {
  int x = 0;
  bool y = false;
  constexpr T() {}
};
union U { int a; float b; constexpr U() : a(1) {} };

void f() {
  constexpr T t = (T{} = T{});
  constexpr U u = (U{} = U{});
  T a, b;
  a = b;
}
