// [temp.param]/1: a template template parameter without `class` or
// `typename`: clang 19 reports every one and recovers it as a type
// template template parameter (clang 22: template-template-param-keyless-recovery.cpp).
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang<22
//@ diag: 20:31 error "template template parameter requires 'class' or 'typename' after the parameter list"
//@ diag: 25:31 error "template template parameter requires 'class' or 'typename' after the parameter list"
//@ diag: 27:31 error "template template parameter requires 'class' or 'typename' after the parameter list"
//@ diag: 29:31 error "template template parameter requires 'class' or 'typename' after the parameter list"
//@ diag: 31:31 error "template template parameter requires 'class' or 'typename' after the parameter list"
//@ diag: 32:31 error "template template parameter requires 'class' or 'typename' after the parameter list"
//@ diag: 33:31 error "template template parameter requires 'class' or 'typename' after the parameter list"
//@ diag: 33:31 error "expected identifier"
//@ diag: 33:1 error "extraneous 'template<>' in declaration of struct 'S7'"
//@ diag: 34:30 error "template template parameter requires 'class' or 'typename' after the parameter list"
//@ diag: 35:30 error "template template parameter requires 'class' or 'typename' after the parameter list"
//@ diag: 36:29 error "template template parameter requires 'class' or 'typename' after the parameter list"
//@ diag: 36:29 error "expected identifier"
template <template <typename> T1> struct S;
template <typename> struct A {};
template <typename T> using AL = A<T>;
S<A> *p;
S<AL> *pa;
template <template <typename> T2 = A> struct S2 {};
S2<> s2;
template <template <typename> T3> struct S3 { T3<int> m; };
S3<A> s3;
template <template <typename> T4, class U> struct S4 {};
S4<A, int> *s4;
template <template <typename> T5> struct S5 { void f() { T5 x; } };
template <template <typename> struct T6> struct S6 {};
template <template <typename> int T7> struct S7 {};
template <template <typename>> struct S8 {};
template <template <typename>...T9> struct S9 {};
template <int N, template<> int M> struct S10 {};
