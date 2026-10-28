// [namespace.udir]/1: using namespace names a namespace
//@ expect: compile-fail
//@ strict-diags
//@ diag: 6:17 error "expected namespace name"
int N;
using namespace N;
