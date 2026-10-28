// [stmt.label]/1: a label is declared once per function
//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:1 error "redefinition of label 'a'"
//@ diag: 7:1 note "previous definition is here"
void f() {
a:;
a:;
}
