//@ strict-diags
//@ diag: 15:7 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 15:7 note "read of non-constexpr variable 'c' is not allowed in a constant expression"
//@ diag: 13:20 note "declared here"
//@ diag: 15:5 warning "variable length array folded to constant array as an extension"
//@ diag: 16:7 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 16:12 note "read of non-constexpr variable 'd' is not allowed in a constant expression"
//@ diag: 14:14 note "declared here"
//@ diag: 16:5 warning "variable length array folded to constant array as an extension"
// WP-165: a namespace-scope array bound that is not a constant expression
// but folds (clang `EvaluateAsInt` reads `const` variables) is a constant
// array, as an extension (-Wgnu-folding-constant).
const _Atomic(int) c = 4;
const double d = 3.0;
int g[c];
int h[(int)d];
static_assert(sizeof(g) == 4 * sizeof(int));
static_assert(sizeof(h) == 3 * sizeof(int));
