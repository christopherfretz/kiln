// WP-1331: C++ has no `wb`/`uwb` literal suffixes (clang 19/22: an
// invalid suffix, at the suffix), even though _BitInt is a C++ extension.
//@ std: c++26
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:10 error "invalid suffix 'wb' on integer constant"
//@ diag: 10:10 error "invalid suffix 'uwb' on integer constant"

int a = 1wb;
int b = 1uwb;
