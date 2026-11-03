// [temp.deduct.general]/7, [temp.deduct.funcaddr], [depr.volatile.type]/2:
// the warnings one deduction holds back are kept from every phase, the
// explicit-argument substitution of the function type (at the template's
// name) before the deduced one (at the return type), and are reported at
// the first use of the specialization, once: a second `&n<...>`, a call
// of a specialization already formed and an explicit specialization or
// instantiation (which reports only its own declaration) add none.
// clang 22 notes each phase.
//@ std: c++26
//@ requires: clang>=22
//@ strict-diags
//@ diag: 31:21 warning "volatile-qualified return type 'volatile int' is deprecated"
//@ diag: 36:11 note "while substituting explicitly-specified template arguments into function template 'f' "
//@ diag: 31:19 warning "volatile-qualified return type 'volatile int' is deprecated"
//@ diag: 36:11 note "while substituting deduced template arguments into function template 'f' [with T = volatile int]"
//@ diag: 37:18 warning "volatile-qualified return type 'volatile int' is deprecated"
//@ diag: 32:30 warning "volatile-qualified return type 'volatile int' is deprecated"
//@ diag: 37:27 note "while substituting explicitly-specified template arguments into function template 'h' "
//@ diag: 32:28 warning "volatile-qualified return type 'volatile int' is deprecated"
//@ diag: 37:27 note "while substituting deduced template arguments into function template 'h' [with T = volatile int, U = int]"
//@ diag: 33:21 warning "volatile-qualified return type 'volatile int' is deprecated"
//@ diag: 38:21 note "while substituting explicitly-specified template arguments into function template 'k' "
//@ diag: 33:19 warning "volatile-qualified return type 'volatile int' is deprecated"
//@ diag: 38:21 note "while substituting deduced template arguments into function template 'k' [with T = volatile int]"
//@ diag: 39:40 warning "volatile-qualified return type 'volatile int' is deprecated"
//@ diag: 40:40 warning "volatile-qualified return type 'volatile long' is deprecated"
//@ diag: 35:21 warning "volatile-qualified return type 'volatile int' is deprecated"
//@ diag: 42:13 note "while substituting explicitly-specified template arguments into function template 'n' "
//@ diag: 35:19 warning "volatile-qualified return type 'volatile int' is deprecated"
//@ diag: 42:13 note "while substituting deduced template arguments into function template 'n' [with T = volatile int]"
template<class T> T f();
template<class T, class U> T h(U);
template<class T> T k();
template<class T> T m() { return T(); }
template<class T> T n();
auto p = &f<volatile int>;
volatile int (*q)(int) = &h<volatile int>;
using X = decltype(&k<volatile int>);
template<> volatile int m<volatile int>();
template volatile long m<volatile long>();
void use() {
  auto a = &n<volatile int>;
  auto b = &n<volatile int>;
  n<volatile int>();
  f<volatile int>();
}
