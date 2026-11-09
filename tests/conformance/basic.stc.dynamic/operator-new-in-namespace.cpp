// [basic.stc.dynamic.general]/2: an allocation function shall be a class
// member or global; one declared in a namespace scope other than the global
// one is ill-formed. Found by the clang negative-test run (WP-912).
//@ std: c++20
//@ xfail(kiln): WP-912 follow-up, allocation-function scope is not checked
//@ expect: compile-fail
//@ diag: 8:21 error "'operator new' cannot be declared inside a namespace"
namespace N { void *operator new(decltype(sizeof 0)); }
