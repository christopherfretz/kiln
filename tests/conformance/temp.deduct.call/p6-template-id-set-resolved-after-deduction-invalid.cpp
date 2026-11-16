// [temp.deduct.call]/6, [over.over]/5: a function template-id argument
// whose members all deduce against `P` is a non-deduced context, so a
// parameter it alone could deduce stays undeduced; and once the other
// arguments fix `P`, two templates giving the same function type with no
// more specialized one leave no overload for the conversion. Found running
// GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ std: c++20
//@ diag: 20:15 error "no matching function for call to 'take'"
//@ diag: 18:34 note "candidate template ignored: couldn't infer template argument 'U'"
//@ diag: 21:15 error "no matching function for call to 'by_ptr'"
//@ diag: 19:34 note "candidate function [with T = int *, U = int] not viable: no overload of 'h' matching 'void (*)(int *, int)' for 1st argument"
template <class T> void amb(T *, int) {}
template <class T> void amb(T *, long) {}
template <class T, class U> void h(T *, U) {}
template <class T, class U, class V = void> void h(T *, U) {}
template <class T, class U> void take(void (*)(T, U)) {}
template <class T, class U> void by_ptr(void (*)(T, U), U) {}
void bad1() { take<int *>(amb<int>); }
void bad2() { by_ptr<int *>(h<int>, 1); }
