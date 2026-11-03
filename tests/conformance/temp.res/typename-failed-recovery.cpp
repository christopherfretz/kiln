// [temp.res.general]/5: a typename-specifier is qualified. After a
// failed `typename` the unqualified name is not consumed: an unknown
// name is reported as one, a known type or a missing name as a missing
// type specifier.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:10 error "expected a qualified name after 'typename'"
//@ diag: 25:11 error "expected ';' after top level declarator"
//@ diag: 26:10 error "expected a qualified name after 'typename'"
//@ diag: 26:11 error "expected ';' after top level declarator"
//@ diag: 27:21 error "expected a qualified name after 'typename'"
//@ diag: 27:22 error "expected ';' at end of declaration list"
//@ diag: 27:35 error "expected a qualified name after 'typename'"
//@ diag: 27:36 error "expected ';' at end of declaration list"
//@ diag: 28:36 error "expected a qualified name after 'typename'"
//@ diag: 28:36 error "expected a qualified name after 'typename'"
//@ diag: 28:36 error "expected ')'"
//@ diag: 28:26 note "to match this '('"
//@ diag: 29:10 error "expected a qualified name after 'typename'"
//@ diag: 30:21 error "expected a qualified name after 'typename'"
//@ diag: 30:21 error "unknown type name 'Q'"
//@ diag: 31:21 error "expected a qualified name after 'typename'"
//@ diag: 31:21 error "a type specifier is required for all declarations"
int x;
typename S s1;
typename Q q1;
struct M { typename Q m; typename S n; };
template <class T> void g(typename T t, typename R r);
typename W;
void f() { typename Q a; }
void g() { typename Q; }
