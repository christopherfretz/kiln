// [temp.param]/4 (C++17): the complex and vector types (clang extensions)
// are no more valid as the type of a non-type template parameter before
// C++20 than floating-point types; `void` (incomplete) and `_Atomic` types
// never are. Found by the clang negative-test run (WP-912), fixed by WP-956.
//@ std: c++17
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:26 error "a non-type template parameter cannot have type '_Complex float' before C++20"
//@ diag: 16:14 error "a non-type template parameter cannot have type 'V4' (vector of 4 'float' values) before C++20"
//@ diag: 17:14 error "a non-type template parameter cannot have type 'I2' (vector of 2 'int' values) before C++20"
//@ diag: 18:16 error "non-type template parameter has incomplete type 'void'"
//@ diag: 19:23 error "a non-type template parameter cannot have type '_Atomic(int)'"
typedef float V4 __attribute__((ext_vector_type(4)));
typedef int I2 __attribute__((vector_size(8)));
template <_Complex float C> struct A {};
template <V4 v> struct B {};
template <I2 v> struct C {};
template <void V> struct D {};
template <_Atomic int I> struct E {};
template <int I, long L, char *P> struct OK {};
