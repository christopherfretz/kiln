// [dcl.align]/3, [temp.inst]: a dependent alignas(type-id) is checked when
// the template is instantiated; an incomplete type argument (here through a
// class template, a pack expansion and a function template) is rejected like
// the non-dependent alignas-void.cpp. Found by the clang negative-test run
// (WP-912), fixed by WP-957. Valid twin: alignas-dependent-type-ok.cpp.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:27 error "invalid application of 'alignas' to an incomplete type 'Inc'"
//@ diag: 20:8 note "in instantiation of template class 'A<Inc>' requested here"
//@ diag: 18:8 note "forward declaration of 'Inc'"
//@ diag: 21:30 error "invalid application of 'alignas' to an incomplete type 'Inc'"
//@ diag: 22:13 note "in instantiation of template class 'P<int, Inc>' requested here"
//@ diag: 18:8 note "forward declaration of 'Inc'"
//@ diag: 23:31 error "invalid application of 'alignas' to an incomplete type 'Inc'"
//@ diag: 24:15 note "in instantiation of function template specialization 'f<Inc>' requested here"
//@ diag: 18:8 note "forward declaration of 'Inc'"
struct Inc;
template <class T> struct alignas(T) A {};
A<Inc> a1;
template <class... T> struct alignas(T...) P {};
P<int, Inc> p1;
template <class T> void f() { alignas(T) int x; (void)x; }
template void f<Inc>();
