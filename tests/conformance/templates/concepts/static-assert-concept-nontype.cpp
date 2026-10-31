//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:15 error "static assertion failed"
//@ diag: 8:15 note "because 3 does not satisfy 'Even'"
//@ diag: 7:33 note "because '3 % 2 == 0' (1 == 0) evaluated to false"
// [temp.constr.atomic]: a concept over a non-type template argument.
template <int N> concept Even = N % 2 == 0;
static_assert(Even<3>);
