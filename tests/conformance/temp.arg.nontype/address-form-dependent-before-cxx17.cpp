// C++98 to C++14 [temp.arg.nontype]/1: the form of a pointer or
// reference argument is checked once a value-dependent argument is
// substituted, a substituted parameter standing for its own argument
// (`&x`, an array, a function): `*q` or `q + 0` with a pointer
// parameter `q` is not an argument (clang 22 at every instantiation;
// GCC 16.1 rejects the dereferences and accepts `q + 0`, which the text
// does not list). C++17 allows any constant expression
// (temp.arg.nontype/address-forms-accepted-cxx17.cpp).
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ requires: clang>=22
//@ strict-diags
//@ args: -std=c++14
//@ diag: 42:34 error "non-type template argument does not refer to any declaration"
//@ diag: 48:8 note "in instantiation of template class 'T1<&x>' requested here"
//@ diag: 34:16 note "template parameter is declared here"
//@ diag: 43:34 error "non-type template argument does not refer to any declaration"
//@ diag: 49:8 note "in instantiation of template class 'T2<&x>' requested here"
//@ diag: 34:16 note "template parameter is declared here"
//@ diag: 44:33 error "non-type template argument does not refer to any declaration"
//@ diag: 50:8 note "in instantiation of template class 'T3<&x>' requested here"
//@ diag: 33:16 note "template parameter is declared here"
//@ diag: 45:33 error "non-type template argument does not refer to any declaration"
//@ diag: 51:8 note "in instantiation of template class 'T4<&x>' requested here"
//@ diag: 33:16 note "template parameter is declared here"
//@ diag: 46:34 error "non-type template argument does not refer to any declaration"
//@ diag: 52:8 note "in instantiation of template class 'T5<&x>' requested here"
//@ diag: 33:16 note "template parameter is declared here"
//@ diag: 47:35 error "non-type template argument does not refer to any declaration"
//@ diag: 53:8 note "in instantiation of template class 'T6<&x>' requested here"
//@ diag: 34:16 note "template parameter is declared here"

template <int *p> struct R {};
template <int &r> struct RR {};
template <void (*f)()> struct FP {};
int x;
int arr[2];
void g();
struct U { static int v; };
int U::v;

template <int *q> struct T1 { RR<*q> a; };
template <int *q> struct T2 { RR<q[0]> a; };
template <int *q> struct T3 { R<q + 0> a; };
template <int *q> struct T4 { R<static_cast<int *>(q)> a; };
template <int *q> struct T5 { R<&*q> a; };
template <int *q> struct T6 { RR<(*q)> a; };
T1<&x> t1;
T2<&x> t2;
T3<&x> t3;
T4<&x> t4;
T5<&x> t5;
T6<&x> t6;

template <int *q> struct OK1 { R<q> a; R<(q)> b; };
template <int &r> struct OK2 { R<&r> a; RR<r> b; RR<(r)> c; };
template <class V> struct OK3 { R<&V::v> a; RR<V::v> b; };
template <void (*f)()> struct OK4 { FP<f> a; };
template <int *q = &x, int *p = q> struct OK5 { R<p> a; };
OK1<&x> ok1;
OK1<arr> ok1a;
OK2<x> ok2;
OK3<U> ok3;
OK4<g> ok4;
OK4<&g> ok4a;
OK5<> ok5;
