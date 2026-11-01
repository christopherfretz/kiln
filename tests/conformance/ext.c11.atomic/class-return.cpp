//@ strict-diags
//@ expect: compile-ok
// Clang extension: an _Atomic(T) return value or parameter of class type T
// is initialized from a T or a class derived from T (clang's
// `NeedAtomicConversion`), as a variable is (class-init.cpp; WP-183).
struct S { int a, b; };
struct D : S {};
_Atomic(S) r1(S &s) { return s; }
_Atomic(S) r2(S s) { return s; }
_Atomic(S) r3(D d) { return d; }
_Atomic(S) r4() { return S{1, 2}; }
