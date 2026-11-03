// [dcl.spec.auto.general]/11: a variable template specialization that
// names itself in its initializer is never deduced. clang 19 gives the
// use a reference of type 'auto' and accepts a program whose only uses
// convert nothing, then crashes generating code for it; kiln reports
// clang 22's "depends on itself" at the end of the translation unit, at
// the point of instantiation, when no other error was reported
// (own-init-var-template-spec-19.cpp has the uses clang 19 rejects).
//@ std: c++26
//@ requires: clang<22
//@ skip(clang): clang 19 accepts the program and crashes in code generation
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:18 error "the type of variable template specialization 'v<int>' declared with deduced type 'auto' depends on itself"
template<class T> auto v = v<T>;
void g() { (void)v<int>; }
