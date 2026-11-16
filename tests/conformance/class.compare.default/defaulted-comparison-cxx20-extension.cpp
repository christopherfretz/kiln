// [class.compare.default]: a defaulted comparison operator is a C++20
// feature; before C++20 kiln accepts it as an extension with clang's
// warning at `default` (found running GCC's C++ testsuite, where GCC
// rejects). Only `==` and `!=`: a defaulted relational operator is an
// error, as in clang (`<=>` does not exist before C++20).
//@ std: c++17
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:38 warning "defaulted comparison operators are a C++20 extension"
//@ diag: 18:37 error "only special member functions may be defaulted"
//@ diag: 19:38 warning "defaulted comparison operators are a C++20 extension"
struct S {
  bool operator==(const S &) const = default;
  int s;
};
struct T {
  int t;
  bool operator<(const T &) const = default;
  bool operator!=(const T &) const = default;
  bool operator==(const T &) const;
};
bool f(const S &a) { return a == a; }
