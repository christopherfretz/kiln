//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:15 error "template parameter has a different kind in template redeclaration"
//@ diag: 11:17 note "previous template declaration is here"
//@ diag: 14:15 error "template parameter has a different kind in template redeclaration"
//@ diag: 13:17 note "previous template declaration is here"
//@ diag: 16:15 error "template parameter has a different kind in template redeclaration"
//@ diag: 15:17 note "previous template declaration is here"
// [temp.over.link]/6: an alias, class or variable template redeclared
// with a non-equivalent template head.
template <class T> using A = T;
template <int N> using A = int;
template <class T> struct C;
template <int N> struct C;
template <class T> extern int v;
template <int N> extern int v;
