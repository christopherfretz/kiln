// Clang extension (C11 6.7.2.4 in C++): `_Atomic ( type-name )` is an
// atomic type specifier, usable in a class template member (libc++
// `__cxx_atomic_base_impl`); it is rejected on array, function, reference,
// atomic and qualified types
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:1 error "_Atomic cannot be applied to reference type 'int &'"
//@ diag: 14:1 error "_Atomic cannot be applied to array type 'int[2]'"
//@ diag: 15:1 error "_Atomic cannot be applied to qualified type 'const int'"
template <class T> struct Impl { _Atomic(T) value; };
Impl<int> i;
_Atomic(long) l;
_Atomic(int&) *r;
_Atomic(int[2]) a;
_Atomic(const int) c;
