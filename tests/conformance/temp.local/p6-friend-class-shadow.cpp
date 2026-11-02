//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:44 error "declaration of 'T' shadows template parameter"
//@ diag: 9:17 note "template parameter is declared here"
// [temp.local]/6: a template-parameter shall not be redeclared within its
// scope; `friend class T;` naming one is such a redeclaration (clang
// reports it in ActOnTag and declares a new class T in the enclosing
// namespace).
template <class T> class A3 { friend class T; };
