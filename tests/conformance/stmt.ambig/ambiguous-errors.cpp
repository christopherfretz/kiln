// [stmt.ambig]: the disambiguation is purely syntactic; a statement that
// parses as a declaration is one even when that makes it ill-formed.
//@ expect: compile-fail
//@ diag: 12:5 error "redefinition of 'x'"
//@ diag: 11:5 note "previous definition is here"
//@ diag: 13:5 error "use of undeclared identifier 'e'"
//@ diag: 14:6 error "use of undeclared identifier 'p'"
typedef int T;
int x;
void g() {
  T(x);
  T(x);
  T(e), (void)0;
  T(*p) + 1;
}
