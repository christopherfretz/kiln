// [module.import]/6, clang 22: a `weak` or `selectany` entity is not
// checked in a header unit (clang 19 diagnoses it,
// header-unit-weak-clang19.cpp). The test file is built as a header unit.
//@ expect: compile-ok
//@ std: c++20
//@ strict-diags
//@ requires: clang>=22
//@ as-header-unit
int __attribute__((weak)) wv = 1;
__attribute__((weak)) int wf() { return 1; }
int __attribute__((selectany)) sa = 1;
