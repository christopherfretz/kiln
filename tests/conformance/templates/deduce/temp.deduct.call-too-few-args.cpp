// [over.match.viable]/2: a template whose parameters outnumber the arguments.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:12 error "no matching function for call to 'f'"
//@ diag: 6:25 note "candidate function template not viable: requires 2 arguments, but 1 was provided"
template <class T> void f(T, T);
void g() { f(1); }
