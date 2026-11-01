//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:24 error "template parameter has a different kind in template template parameter redeclaration"
//@ diag: 9:26 note "previous template template parameter is here"
//@ diag: 13:11 error "type constraint differs in template redeclaration"
//@ diag: 12:11 note "previous template declaration is here"
// [temp.over.link]/6: the lists of a template template parameter, and a
// constrained parameter redeclared unconstrained.
template <template <class> class P> struct F;
template <template <int> class P> struct F;
template <class T> concept K = true;
template <K T> struct G;
template <class T> struct G;
