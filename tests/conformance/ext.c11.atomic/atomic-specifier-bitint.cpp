// Clang extension (C11 6.7.2.4 in C++): `_Atomic` is rejected on every
// bit-precise integer type, power-of-two width or not, also when the
// `_BitInt` reaches it through template instantiation (libc++
// `std::atomic<_BitInt(32)>`, libcxx/atomics/bit-int.verify.cpp). The
// instantiated error's column is left open: clang puts it at `_Atomic`,
// kiln's type substitution at the declarator (no type locations yet).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:1 error "_Atomic cannot be applied to integer type '_BitInt(32)'"
//@ diag: 15:1 error "_Atomic cannot be applied to integer type 'unsigned _BitInt(8)'"
//@ diag: 16:1 error "_Atomic cannot be applied to integer type '_BitInt(7)'"
//@ diag: 17 error "_Atomic cannot be applied to integer type '_BitInt(64)'"
//@ diag: 18:16 note "in instantiation of template class 'A<_BitInt(64)>' requested here"
_Atomic(_BitInt(32)) a;
_Atomic(unsigned _BitInt(8)) b;
_Atomic(_BitInt(7)) c;
template<class T> struct A { _Atomic(T) v; };
A<_BitInt(64)> d;
