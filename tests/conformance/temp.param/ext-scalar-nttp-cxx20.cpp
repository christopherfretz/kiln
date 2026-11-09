// [temp.param]/6 (C++20): the complex and vector types (clang extensions)
// are structural, so they may be the type of a non-type template parameter
// (the C++20 twin of ext-scalar-nttp-cxx17.cpp); `void` and `_Atomic` types
// still may not. WP-956.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:16 error "non-type template parameter has incomplete type 'void'"
//@ diag: 16:23 error "a non-type template parameter cannot have type '_Atomic(int)'"
typedef float V4 __attribute__((ext_vector_type(4)));
typedef int I2 __attribute__((vector_size(8)));
template <_Complex float C> struct A {};
template <V4 v> struct B {};
template <I2 v> struct C {};
template <void V> struct D {};
template <_Atomic int I> struct E {};
template <float F> struct OK {};
