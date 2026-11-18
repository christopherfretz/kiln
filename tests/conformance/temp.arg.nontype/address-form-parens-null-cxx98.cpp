// C++03 [temp.arg.nontype]/1: an address argument is `&`id-expression
// without parentheses, and there is no null pointer form. clang 22
// accepts the parentheses as an extension (C++11 allows them), warned
// where written and again for each instantiation of a value-dependent
// argument, and rejects a null pointer or null pointer to member (GCC
// 16.1 rejects both the parentheses and the nulls).
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ requires: clang>=22
//@ strict-diags
//@ args: -std=c++98
//@ diag: 34:3 warning "parentheses around address non-type template argument are a C++11 extension"
//@ diag: 35:3 warning "parentheses around address non-type template argument are a C++11 extension"
//@ diag: 36:4 warning "parentheses around address non-type template argument are a C++11 extension"
//@ diag: 37:4 warning "parentheses around address non-type template argument are a C++11 extension"
//@ diag: 38:4 warning "parentheses around address non-type template argument are a C++11 extension"
//@ diag: 39:34 warning "parentheses around address non-type template argument are a C++11 extension"
//@ diag: 40:37 warning "parentheses around address non-type template argument are a C++11 extension"
//@ diag: 39:34 warning "parentheses around address non-type template argument are a C++11 extension"
//@ diag: 41:7 note "in instantiation of template class 'T1<x>' requested here"
//@ diag: 40:37 warning "parentheses around address non-type template argument are a C++11 extension"
//@ diag: 42:11 note "in instantiation of template class 'T2<&S::m>' requested here"
//@ diag: 44:3 error "non-type template argument does not refer to any declaration"
//@ diag: 28:16 note "template parameter is declared here"
//@ diag: 45:4 error "non-type template argument is not a pointer to member constant"

struct S { int m; void f(); };
template <int *p> struct R {};
template <int &r> struct RR {};
template <int S::*pm> struct PM {};
template <void (S::*pf)()> struct PF {};
int x;

R<(&x)> w1;
R<((&x))> w2;
RR<(x)> w3;
PF<(&S::f)> w4;
PM<(&S::m)> w5;
template <int &r> struct T1 { RR<(r)> a; };
template <int S::*q> struct T2 { PM<(q)> a; };
T1<x> t1;
T2<&S::m> t2;

R<(int *)0> e1;
PM<(int S::*)0> e2;

R<&x> ok1;
RR<x> ok2;
PM<&S::m> ok3;
template <int *q> struct T3 { R<q> a; };
T3<&x> ok4;
