// [dcl.fct.def.delete]/2: naming a deleted function other than to declare
// it is ill-formed. As the left operand of a comma the uncalled name is
// that use: clang reports it once and drops the ill-formed operand, so no
// -Wunused-value follows for it or for the comma.
//@ xfail(kiln): WP-1170 follow-up, the deleted operand stays a valid expression and draws two -Wunused-value warnings
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:22 error "attempt to use a deleted function"
//@ diag: 10:6 note "'del' has been explicitly marked deleted here"
void del(int) = delete;
void comma_left() { (del, 1); }
