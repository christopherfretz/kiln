//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:15 error "static assertion failed"
//@ diag: 10:15 note "does not satisfy 'Even'"
//@ diag: 9:33 note "because '3 % 2 == 0' (1 == 0) evaluated to false"
// clang 19 and 22 word these differently: the substrings above are common
// to both, and kiln follows its -fclang-compat level (WP-377).
// [temp.constr.atomic]: a concept over a non-type template argument.
template <int N> concept Even = N % 2 == 0;
static_assert(Even<3>);
