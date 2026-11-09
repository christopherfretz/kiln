// [temp.local]/6: a template-parameter shall not be redeclared within
// its scope. An elaborated-type-specifier `struct T` whose name finds a
// template type parameter declares a new class `T` (clang `ActOnTag`):
// "declaration of 'T' shadows template parameter" at the name, with the
// parameter noted, and the reference names the class found or declared
// in the enclosing namespace (clang test temp.local/p6.cpp:63; WP-1091).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:48 error "declaration of 'T' shadows template parameter"
//@ diag: 15:16 note "template parameter is declared here"
//@ diag: 16:37 error "declaration of 'U' shadows template parameter"
//@ diag: 16:16 note "template parameter is declared here"
//@ diag: 17:52 error "declaration of 'T' shadows template parameter"
//@ diag: 17:16 note "template parameter is declared here"
template<class T> struct A { void f() { struct T *p; (void)p; } };
template<class U> void g() { struct U *p; struct U *q; (void)p; (void)q; }
template<class T> struct C { int n = sizeof(struct T *); };
