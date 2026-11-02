//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:59 error "invalid index 1 for pack"
//@ diag: 10:9 note "in instantiation of function template specialization 'f<int>' requested here"
// clang 19 and 22 word these differently: the substrings above are common
// to both, and kiln follows its -fclang-compat level (WP-377).
// [expr.prim.pack.index]/2: the index must be less than the pack's size;
// clang reports it at the pack's name, naming the expression unquoted.
template <class... Ts> constexpr int f(Ts... ts) { return ts...[1]; }
int x = f(1);
