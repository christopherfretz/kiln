//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:59 error "invalid index 1 for pack ts of size 1"
//@ diag: 8:9 note "in instantiation of function template specialization 'f<int>' requested here"
// [expr.prim.pack.index]/2: the index must be less than the pack's size;
// clang reports it at the pack's name, naming the expression unquoted.
template <class... Ts> constexpr int f(Ts... ts) { return ts...[1]; }
int x = f(1);
