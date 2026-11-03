// [temp.res.general]/5: `typename` followed by an unqualified name that
// names a variable: the declaration is invalid and is not a
// redeclaration of the variable.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:21 error "expected a qualified name after 'typename'"
//@ diag: 13:21 error "unknown type name 'Q'"
//@ diag: 14:21 error "expected a qualified name after 'typename'"
//@ diag: 14:22 error "expected ';' at end of declaration list"
//@ diag: 15:10 error "expected a qualified name after 'typename'"
//@ diag: 15:11 error "expected ';' after top level declarator"
int Q;
void f() { typename Q a; }
struct M { typename Q m; };
typename Q b;
