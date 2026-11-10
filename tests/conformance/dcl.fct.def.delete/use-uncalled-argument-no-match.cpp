// [dcl.fct.def.delete]/2, [over.match.viable]/4: a deleted function named
// as an argument is ill-formed by itself. clang reports that use once at
// the name and does not then also fail overload resolution for the call
// it is an argument of, even when no parameter could take the function.
//@ xfail(kiln): WP-1170 follow-up, the call is also diagnosed "no matching function for call to 'g'"
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:27 error "attempt to use a deleted function"
//@ diag: 10:6 note "'del' has been explicitly marked deleted here"
void del(int) = delete;
int g(int, int);
void argument() { (void)g(del, 1); }
