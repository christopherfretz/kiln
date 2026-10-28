// [cpp.pragma]: '#pragma GCC visibility' push/pop must balance; an unknown
// visibility is ignored (and so is not pushed).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:13 warning "unknown visibility 'bogus'"
//@ diag: 16:13 error "#pragma visibility pop with no matching #pragma visibility push"
#pragma GCC visibility push(hidden)
int a;
#pragma GCC visibility push(default)
int b;
#pragma GCC visibility pop
#pragma GCC visibility pop
#pragma GCC visibility push(protected)
#pragma GCC visibility pop
#pragma GCC visibility push(bogus)
#pragma GCC visibility pop
int c;
