// [expr.pre.incr]/1, [expr.post.incr]/1 (C++17, P0002R1): the operand of
// `++` may not be of type bool. WP-646 feature probe.
//@ expect: compile-fail
//@ std: c++17
//@ strict-diags
//@ diag: 10:12 error "ISO C++17 does not allow incrementing expression of type bool"
//@ diag: 10:18 error "ISO C++17 does not allow incrementing expression of type bool"
//@ diag: 10:22 error "cannot decrement expression of type bool"
bool b = false;
void f() { ++b; b++; --b; }
