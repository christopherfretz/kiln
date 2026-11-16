//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ args: -std=c++20 -pedantic-errors -Wno-error=pedantic
//@ diag: 15:24 warning "flexible array members are a C99 feature"
//@ diag: 16:23 error "variable length arrays in C++ are a Clang extension"
//@ diag: 16:23 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 16:12 note "declared here"
// clang-tblgen puts every off-by-default extension in -Wpedantic whatever
// its own group, so -Wno-error=pedantic keeps -Wc99-extensions (the
// flexible array member) a warning under -pedantic-errors in clang 22;
// -Wvla-cxx-extension is on by default, not in -Wpedantic, and stays an
// error. clang 19 keeps both errors (the sibling no-error-pedantic-19.cpp).
// Found running GCC's C++ testsuite.
struct A { int n; char a[]; };
void f(int n) { int v[n]; (void)v; }
