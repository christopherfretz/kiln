//@ expect: compile-fail
//@ strict-diags
//@ xfail(kiln): an unnamed template parameter is located at its keyword, not at the following token as clang does (parser), and an unconstrained type parameter's first token is not recorded, so these two diagnostics are at the wrong column
//@ diag: 11:24 error "template parameter has a different kind in template template parameter redeclaration"
//@ diag: 10:26 note "previous template template parameter is here"
//@ diag: 14:11 error "type constraint differs in template redeclaration"
//@ diag: 13:11 note "previous template declaration is here"
// [temp.over.link]/6: the lists of a template template parameter, and a
// constrained parameter redeclared unconstrained.
template <template <class> class P> struct F;
template <template <int> class P> struct F;
template <class T> concept K = true;
template <K T> struct G;
template <class T> struct G;
