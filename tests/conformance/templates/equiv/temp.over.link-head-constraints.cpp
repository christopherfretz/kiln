//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:11 error "type constraint differs in template redeclaration"
//@ diag: 14:11 note "previous template declaration is here"
//@ diag: 17:29 error "requires clause differs in template redeclaration"
//@ diag: 16:29 note "previous template declaration is here"
//@ diag: 19:1 error "requires clause differs in template redeclaration"
//@ diag: 18:29 note "previous template declaration is here"
// [temp.over.link]/6: type-constraints and requires-clauses of a
// redeclared template head must be equivalent (a different concept, or a
// clause that is missing, is not).
template <class T> concept K = true;
template <class T> concept K2 = true;
template <K T> struct G;
template <K2 T> struct G;
template <class T> requires K<T> struct H;
template <class T> requires K2<T> struct H;
template <class T> requires K<T> struct H2;
template <class T> struct H2;
