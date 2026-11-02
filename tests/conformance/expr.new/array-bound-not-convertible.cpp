// [expr.new]/8: later bounds are converted constant expressions of type std::size_t.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:23 error "value of type 'S' is not implicitly convertible to '__size_t' (aka 'unsigned long')"
//@ requires: clang>=22
struct S {};
extern S s;
void f() { new int[2][s]; }
