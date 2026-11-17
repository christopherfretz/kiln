//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ args: -ferror-limit=0
//@ diag: 22:1 error "'virtual' can only appear on non-static member functions"
//@ diag: 23:1 error "'explicit' can only appear on non-static member functions"
//@ diag: 27:3 error "'virtual' can only appear on non-static member functions"
//@ diag: 28:3 error "'explicit' can only appear on non-static member functions"
//@ diag: 29:3 error "'explicit' can only appear on non-static member functions"
//@ diag: 30:10 error "'virtual' can only appear on non-static member functions"
//@ diag: 31:8 error "'virtual' can only appear on non-static member functions"
//@ diag: 32:8 error "'explicit' can only appear on non-static member functions"
// [dcl.struct.bind]/1: the decl-specifier-seq of a structured binding
// declaration holds no function specifier ([dcl.fct.spec]/2, /3:
// `virtual` and `explicit` belong to member functions), at namespace
// scope, block scope and as a for-range-declaration. clang 22 and GCC 16.1
// reject each one at the specifier. The specifiers that are allowed
// stay accepted. Found running GCC's C++ testsuite.
struct S { int a, b; };
S s, arr[2];

virtual auto [n1, n2] = s;
explicit auto [n3, n4] = s;
static auto [n5, n6] = s;

void f() {
  virtual auto [a1, b1] = s;
  explicit auto [a2, b2] = s;
  explicit(true) auto [a3, b3] = s;
  static virtual const auto [a4, b4] = s;
  for (virtual auto [x, y] : arr) {}
  for (explicit auto &[x, y] : arr) {}
  for (const auto &[x, y] : arr) {}
  static thread_local auto [a5, b5] = s;
  auto &&[a6, b6] = s;
}
