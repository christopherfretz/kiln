// [stmt.if]/2 (WP-234): the constexpr form of the if statement is C++17;
// in earlier modes it is accepted as an extension and reported (by default,
// -Wc++17-extensions) at the 'constexpr' keyword, including in an
// 'else if constexpr' and in templates.
//@ std: c++14
//@ strict-diags
//@ diag: 13:6 warning "constexpr if is a C++17 extension"
//@ diag: 16:6 warning "constexpr if is a C++17 extension"
//@ diag: 17:6 warning "constexpr if is a C++17 extension"
//@ diag: 18:21 warning "constexpr if is a C++17 extension"

template <int N> int f() {
  if constexpr (N > 0) { return 1; } else { return 0; }
}
int g(int x) {
  if constexpr (true) {}
  if constexpr (sizeof(int) == 4) { x++; }
  if (x) {} else if constexpr (false) {}
  return f<1>() + f<0>() + x;
}
