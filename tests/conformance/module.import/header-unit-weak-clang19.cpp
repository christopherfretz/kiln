// [module.import]/6, clang 19: a `weak` or `selectany` definition with
// external linkage is diagnosed in a header unit like any other (clang 22
// exempts it, header-unit-weak.cpp). The test file is built as a header
// unit.
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ requires: clang<22
//@ as-header-unit
//@ diag: 13:27 error "non-inline external definitions are not permitted in C++ header units"
//@ diag: 14:27 error "non-inline external definitions are not permitted in C++ header units"
//@ diag: 15:32 error "non-inline external definitions are not permitted in C++ header units"
int __attribute__((weak)) wv = 1;
__attribute__((weak)) int wf() { return 1; }
int __attribute__((selectany)) sa = 1;
