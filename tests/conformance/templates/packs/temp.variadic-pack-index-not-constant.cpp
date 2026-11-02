//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:62 error "array size is not a constant expression"
//@ diag: 9:62 note "function parameter 'i' with unknown value cannot be used in a constant expression"
//@ diag: 9:34 note "declared here"
// [expr.prim.pack.index]/1: the index is a converted constant expression of
// type std::size_t; a non-dependent index that is not constant is diagnosed
// in the template definition (clang's "array size" wording).
template <class... Ts> int f(int i, Ts... ts) { return ts...[i]; }
