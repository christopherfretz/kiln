// [temp.param]/1: a template template parameter declares its kind with
// `class` or `typename`. clang 22 reports a missing key only where one
// was plausibly forgotten and recovers the parameter as naming no
// template: every argument for it is rejected and its uses are not
// template names. clang 19's recovery is
// template-template-param-keyless-recovery-clang19.cpp.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 35:31 error "template template parameter requires 'class' or 'typename' after the parameter list"
//@ diag: 38:3 error "template argument does not refer to a class or alias template, or template template parameter"
//@ diag: 36:28 note "template argument refers to a class template 'A', here"
//@ diag: 39:3 error "template argument does not refer to a class or alias template, or template template parameter"
//@ diag: 37:23 note "template argument refers to a class template 'AL', here"
//@ diag: 40:31 error "template template parameter requires 'class' or 'typename' after the parameter list"
//@ diag: 40:36 error "template argument does not refer to a class or alias template, or template template parameter"
//@ diag: 36:28 note "template argument refers to a class template 'A', here"
//@ diag: 41:1 error "too few template arguments for class template 'S2'"
//@ diag: 40:46 note "template is declared here"
//@ diag: 42:31 error "template template parameter requires 'class' or 'typename' after the parameter list"
//@ diag: 42:47 error "no template named 'T3'"
//@ diag: 43:4 error "template argument does not refer to a class or alias template, or template template parameter"
//@ diag: 36:28 note "template argument refers to a class template 'A', here"
//@ diag: 44:31 error "template template parameter requires 'class' or 'typename' after the parameter list"
//@ diag: 45:4 error "template argument does not refer to a class or alias template, or template template parameter"
//@ diag: 36:28 note "template argument refers to a class template 'A', here"
//@ diag: 46:31 error "template template parameter requires 'class' or 'typename' after the parameter list"
//@ diag: 46:58 error "unknown type name 'T5'"
//@ diag: 47:31 error "template template parameter requires 'class' or 'typename' after the parameter list"
//@ diag: 48:31 error "expected identifier"
//@ diag: 48:1 error "extraneous 'template<>' in declaration of struct 'S7'"
//@ diag: 49:30 error "template template parameter requires 'class' or 'typename' after the parameter list"
//@ diag: 50:30 error "template template parameter requires 'class' or 'typename' after the parameter list"
//@ diag: 51:29 error "expected identifier"
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
