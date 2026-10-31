//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:1 error "too many template parameters in template redeclaration"
//@ diag: 8:1 note "previous template declaration is here"
//@ diag: 11:1 error "too few template parameters in template redeclaration"
//@ diag: 10:1 note "previous template declaration is here"
// [temp.over.link]/6: template heads of different lengths.
template <class T> using B = T;
template <class T, class U> using B = T;
template <class T, class U> struct I;
template <class T> struct I;
