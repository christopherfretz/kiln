// [depr.volatile.type]/2, [dcl.fct]/5: a volatile-qualified parameter type
// is deprecated, before the parameter adjustment drops the qualifier: at
// the parameter (a typedef keeps its sugar), at the substituted parameter
// of a deduced function template specialization (at its first use, once
// per specialization; an explicit-argument substitution reports each phase)
// and at a class member instantiation; a pointer to volatile is not
// volatile-qualified.
// clang 22 notes the deduction context.
//@ std: c++26
//@ requires: clang>=22
//@ strict-diags
//@ diag: 34:21 warning "volatile-qualified parameter type 'volatile int' is deprecated"
//@ diag: 35:21 warning "volatile-qualified parameter type 'volatile int' is deprecated"
//@ diag: 36:15 warning "volatile-qualified return type 'volatile int' is deprecated"
//@ diag: 38:24 warning "volatile-qualified parameter type 'int *volatile' is deprecated"
//@ diag: 40:11 warning "volatile-qualified parameter type 'VI' (aka 'volatile int') is deprecated"
//@ diag: 41:19 warning "volatile-qualified parameter type 'volatile int' is deprecated"
//@ diag: 42:12 note "while substituting deduced template arguments into function template 't' [with T = volatile int]"
//@ diag: 43:26 warning "volatile-qualified parameter type 'volatile int' is deprecated"
//@ diag: 44:35 warning "volatile-qualified parameter type 'volatile int' is deprecated"
//@ diag: 44:19 warning "volatile-qualified parameter type 'volatile int' is deprecated"
//@ diag: 45:9 note "while substituting deduced template arguments into function template 'g' [with T = int]"
//@ diag: 46:58 warning "volatile-qualified parameter type 'const volatile T' is deprecated"
//@ diag: 46:19 warning "volatile-qualified parameter type 'const volatile int' is deprecated"
//@ diag: 47:12 note "while substituting deduced template arguments into function template 'h' [with T = int]"
//@ diag: 48:21 warning "volatile-qualified return type 'volatile int' is deprecated"
//@ diag: 49:12 note "while substituting explicitly-specified template arguments into function template 'k' "
//@ diag: 48:21 warning "volatile-qualified parameter type 'volatile int' is deprecated"
//@ diag: 48:19 warning "volatile-qualified return type 'volatile int' is deprecated"
//@ diag: 49:12 note "while substituting deduced template arguments into function template 'k' [with T = volatile int]"
//@ diag: 48:19 warning "volatile-qualified parameter type 'volatile int' is deprecated"
//@ diag: 50:30 warning "volatile-qualified parameter type 'volatile int' is deprecated"
//@ diag: 51:17 note "in instantiation of template class 'C<volatile int>' requested here"
void f(volatile int x);
void f2(volatile int);
volatile int r();
void f3(int volatile * p);
void f4(int * volatile p) {}
typedef volatile int VI;
void f5(VI);
template<class T> void t(T x) {}
void u() { t<volatile int>(1); }
auto l = [](volatile int x) {};
template<class T> T g(volatile int);
int x = g<int>(1);
template<class T> void h(T, volatile T*, const volatile T);
void z() { h(1, (int*)0, 2); }
template<class T> T k(T);
auto pk = &k<volatile int>;
template<class T> struct C { void m(T); };
C<volatile int> ci;
