// [over.match.class.deduct]/3 (P1814) is C++20: in C++17 deduction for
// an alias template is accepted with clang's C++20-extension warning;
// in C++20 it is silent (p3-alias-undeduced-default-argument.cpp).
// Found running GCC's C++ testsuite.
//@ expect: compile-ok
//@ std: c++17
//@ strict-diags
//@ requires: clang>=22
//@ diag: 14:10 warning "class template argument deduction for alias templates is a C++20 extension [-Wc++20-extensions]"
//@ diag: 15:3 warning "class template argument deduction for alias templates is a C++20 extension [-Wc++20-extensions]"

template <class> struct A { A(int = 0); };
template <class = void> using B = A<int>;
auto x = B{};
B y(1);
