// [class.access]: -faccess-control after -fno-access-control turns access
// checking back on (the last one wins) (WP-1467).
//@ args: -fno-access-control -faccess-control
//@ expect: compile-fail
//@ diag: 8:23 error "'x' is a private member of 'A'"
//@ diag: 7:15 note "implicitly declared private here"
class A { int x = 1; };
int g(A a) { return a.x; }
