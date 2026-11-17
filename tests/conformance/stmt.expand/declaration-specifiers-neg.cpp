// [stmt.expand]/2: the declaration of an expansion statement is a
// for-range-declaration, so its decl-specifiers are a type-specifier or
// `constexpr` ([stmt.iter.general]); `virtual` and `explicit` are the
// function-specifier rule of any variable, also on a structured binding
// ([dcl.struct.bind]/1), and a storage class the range-for rule. GCC 16.1
// rejects the same lines ("'virtual' outside class declaration").
// Found running GCC's C++ testsuite.
//@ std: c++26
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=23
//@ args: -ferror-limit=0
//@ diag: 22:17 error "'virtual' can only appear on non-static member functions"
//@ diag: 23:17 error "'explicit' can only appear on non-static member functions"
//@ diag: 24:17 error "loop variable 'x' may not be declared 'static'"
//@ diag: 25:17 error "'virtual' can only appear on non-static member functions"
//@ diag: 26:17 error "'explicit' can only appear on non-static member functions"
struct S { int a, b; };
void f() {
  S arr[1]{};
  int v[2]{};
  template for (virtual int x : v) {}
  template for (explicit int x : v) {}
  template for (static int x : v) {}
  template for (virtual auto [a, b] : arr) {}
  template for (explicit(true) auto &[a, b] : arr) {}
  template for (constexpr auto x : {1, 2}) {}
  template for (const auto &x : v) {}
}
