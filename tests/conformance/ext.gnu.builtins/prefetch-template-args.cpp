// __builtin_prefetch's `rw` and `locality` ranges are checked when a
// dependent argument is instantiated (clang `BuiltinPrefetch` on the
// instantiated call). WP-610.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:49 error "argument value 5 is outside the valid range [0, 3]"
//@ diag: 10:15 note "in instantiation of function template specialization 't<1, 5>' requested here"
template <int R, int L> void t(const void *p) { __builtin_prefetch(p, R, L); }
template void t<0, 3>(const void *);
template void t<1, 5>(const void *);
