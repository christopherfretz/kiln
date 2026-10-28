// [cpp.pragma]: malformed '#pragma GCC visibility' directives are ignored
// with a warning at the offending token.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 9:29 warning "missing '(' after '#pragma visibility' - ignoring"
//@ diag: 10:35 warning "missing ')' after '#pragma visibility' - ignoring"
//@ diag: 11:38 warning "extra tokens at end of '#pragma visibility' - ignored"
//@ diag: 12:29 warning "expected identifier in '#pragma visibility' - ignored"
#pragma GCC visibility push hidden
#pragma GCC visibility push(hidden
#pragma GCC visibility push(default) extra
#pragma GCC visibility push(1)
int x;
