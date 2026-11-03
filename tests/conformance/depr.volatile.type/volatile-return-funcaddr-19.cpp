// [temp.deduct.general]/7, [temp.deduct.funcaddr], [depr.volatile.type]/2:
// the warnings one deduction holds back are kept from every phase, the
// explicit-argument substitution of the function type (at the template's
// name) before the deduced one (at the return type), and are reported at
// the first use of the specialization, once: a second `&n<...>`, a call
// of a specialization already formed and an explicit specialization or
// instantiation (which reports only its own declaration) add none.
// clang 19 without the context notes.
//@ std: c++26
//@ requires: clang<22
//@ strict-diags
//@ diag: 23:21 warning "volatile-qualified return type 'volatile int' is deprecated"
//@ diag: 23:19 warning "volatile-qualified return type 'volatile int' is deprecated"
//@ diag: 29:18 warning "volatile-qualified return type 'volatile int' is deprecated"
//@ diag: 24:30 warning "volatile-qualified return type 'volatile int' is deprecated"
//@ diag: 24:28 warning "volatile-qualified return type 'volatile int' is deprecated"
//@ diag: 25:21 warning "volatile-qualified return type 'volatile int' is deprecated"
//@ diag: 25:19 warning "volatile-qualified return type 'volatile int' is deprecated"
//@ diag: 31:40 warning "volatile-qualified return type 'volatile int' is deprecated"
//@ diag: 32:40 warning "volatile-qualified return type 'volatile long' is deprecated"
//@ diag: 27:21 warning "volatile-qualified return type 'volatile int' is deprecated"
//@ diag: 27:19 warning "volatile-qualified return type 'volatile int' is deprecated"
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
