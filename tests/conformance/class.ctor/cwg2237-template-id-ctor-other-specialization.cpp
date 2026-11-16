// [class.ctor.general]/1: a constructor declarator-id written as a
// template-id must name the class itself; `X<int>();` in `X<T>` names
// another specialization. kiln rejects it as GCC 16.1 does; clang 22
// accepts it. Found running GCC's C++ testsuite (WP-1466).
//@ expect: compile-fail
//@ std: c++17
//@ strict-diags
//@ diag: 10:30 error "expected unqualified-id"
//@ xfail(clang): clang 22 accepts a template-id naming another specialization
template<class T> struct X { X<int>(); };
