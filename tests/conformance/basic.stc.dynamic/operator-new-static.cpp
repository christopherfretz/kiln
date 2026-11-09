// [basic.stc.dynamic.general]/2: a global allocation function shall not be
// declared static. Found by the clang negative-test run (WP-912).
//@ std: c++20
//@ xfail(kiln): WP-912 follow-up, a static redeclaration of the implicitly declared global operator new is accepted
//@ expect: compile-fail
//@ diag: 7:14 error "static declaration of 'operator new' follows non-static declaration"
static void *operator new(decltype(sizeof 0));
