// [temp.dep.candidate], [expr.type.conv], [temp.inst]/4: a member
// function template found by a header's dependent call is instantiated
// for the program's arguments and its errors are diagnosed in the header.
// With `--pch-diff`, the image's found members give the same diagnostics,
// byte for byte (WP-1314).
//@ expect: compile-fail
//@ diag: 11:9 error "use of undeclared identifier 'undeclared_wp1314'"
#include "Inputs/member_names.h"
struct NC {};
int a = mf::holder<NC>::go(NC{});
int b = undeclared_wp1314;
