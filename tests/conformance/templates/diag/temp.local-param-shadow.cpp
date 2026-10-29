//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:27 error "declaration of 'B' shadows template parameter"
//@ diag: 20:17 note "template parameter is declared here"
//@ diag: 21:35 error "declaration of 'T' shadows template parameter"
//@ diag: 21:17 note "template parameter is declared here"
//@ diag: 22:35 error "declaration of 'T' shadows template parameter"
//@ diag: 22:17 note "template parameter is declared here"
//@ diag: 23:36 error "declaration of 'U' shadows template parameter"
//@ diag: 23:17 note "template parameter is declared here"
//@ diag: 24:38 error "declaration of 'V' shadows template parameter"
//@ diag: 24:17 note "template parameter is declared here"
//@ diag: 26:20 error "declaration of 'Y' shadows template parameter"
//@ diag: 25:67 note "template parameter is declared here"
//@ diag: 27:24 error "declaration of 'P' shadows template parameter"
//@ diag: 27:17 note "template parameter is declared here"
// [temp.local]/6: a template parameter cannot be redeclared within its
// scope, including by the template's own name. (Function parameters and
// a function template named like its parameter: not covered yet.)
template <class B> struct B {};
template <class T> void f() { int T; }
template <class T> struct A { int T; };
template <class U> struct C { void U(); };
template <class V> struct D { struct V {}; };
template <class X> struct E { void g() { int Y; } template <class Y>
    void h() { int Y; } };
template <class P, int P> struct G;
