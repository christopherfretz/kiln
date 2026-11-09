// [basic.stc.dynamic.general]/2: a global allocation function shall not be
// declared static. Found by the clang negative-test run (WP-912).
//@ std: c++20
//@ expect: compile-fail
//@ diag: 7:14 error "static declaration of 'operator new' follows non-static declaration"
//@ diag: 7:14 note "previous implicit declaration is here"
static void *operator new(decltype(sizeof 0));
