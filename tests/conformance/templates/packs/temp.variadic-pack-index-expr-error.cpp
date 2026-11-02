//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:59 error "invalid index 1 for pack ts of size 1"
//@ diag: 10:9 note "in instantiation of function template specialization 'f<int>' requested here"
//@ requires: clang<22
// clang 22 emits error: invalid index 1 for pack 'ts' of size 1; WP-355 residual
// [expr.prim.pack.index]/2: the index must be less than the pack's size;
// clang reports it at the pack's name, naming the expression unquoted.
template <class... Ts> constexpr int f(Ts... ts) { return ts...[1]; }
int x = f(1);
