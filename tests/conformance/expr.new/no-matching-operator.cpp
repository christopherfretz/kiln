// [expr.new]/19: overload resolution for the allocation function fails.
//@ expect: compile-fail
//@ diag: 4:12 error "no matching function for call to 'operator new'"
void f() { new (1, 2) int; }
