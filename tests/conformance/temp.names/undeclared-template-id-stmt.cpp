// [stmt.ambig], [temp.names]/3.3: at block scope an undeclared template-id
// before a ptr-operator or a cv-qualifier is ambiguous and the declarator
// decides (clang's `TNK_Undeclared_template`, `Ambiguous` in
// `isCXXDeclarationSpecifier`): a declaration reports "no template named"
// and goes on with an invalid `int` type; `w3<int> & gi + 1;` is an
// expression.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:12 error "no template named 'w1'"
//@ diag: 16:22 error "declaration of reference variable 'gi' requires an initializer"
//@ diag: 17:12 error "no template named 'w2'"
//@ diag: 18:12 error "use of undeclared identifier 'w3'"
//@ diag: 19:12 error "no template named 'w4'"
//@ diag: 20:12 error "no template named 'w5'"
int gi;
void p() { w1<int> & gi; }
void q() { w2<int> * gi; }
void r() { w3<int> & gi + 1; }
void s() { w4<int> const *j; }
void t() { w5<int> *(p); }
int ok = 0;
