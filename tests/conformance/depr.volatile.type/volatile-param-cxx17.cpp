// [depr.volatile.type]: the deprecation is C++20; C++17 accepts a
// volatile-qualified parameter type silently.
//@ std: c++17
//@ strict-diags
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
