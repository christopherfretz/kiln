// Clang extension: an _Atomic(T) object of class type T is initialized
// from a T or a class derived from T (copy, direct or a one-element
// braced list), converted by NonAtomicToAtomic; it is assigned from a T or
// an _Atomic(T) (C11 6.5.16.1). Other sources, an _Atomic(T) initializer
// and empty or nested braces are rejected; assignment errors are at '='.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 28:30 error "no viable conversion from 'C' to '_Atomic(S3)'"
//@ diag: 20:12 note "candidate function"
//@ diag: 29:30 error "no viable conversion from 'C' to '_Atomic(S3)'"
//@ diag: 20:12 note "candidate function"
//@ diag: 30:40 error "cannot initialize a variable of type '_Atomic(S3)' with an lvalue of type '_Atomic(S3)'"
//@ diag: 31:28 error "illegal initializer type '_Atomic(S3)'"
//@ diag: 32:28 error "illegal initializer type '_Atomic(S3)'"
//@ diag: 33:20 error "assigning to '_Atomic(S3)' from incompatible type 'D'"
//@ diag: 34:20 error "assigning to '_Atomic(S3)' from incompatible type 'C'"
//@ diag: 35:22 error "assigning to '_Atomic(int)' from incompatible type 'S3'"
struct S3 { char c[3]; };
struct D : S3 {};
struct C { operator S3(); };
_Atomic(S3) e;
void ok1(S3 s) { _Atomic(S3) x = s; _Atomic(S3) y(s); _Atomic(S3) z{s}; }
void ok2(D d) { _Atomic(S3) x = d; _Atomic(S3) y{d}; }
void ok3(const S3 &s, volatile _Atomic(S3) &v) { e = s; e = v; v = e; }
void ok4(S3 s) { _Atomic(S3) x = {s}; }
void ok5() { e = S3{}; }
_Atomic(int) ai;
void bad1(C c) { _Atomic(S3) x = c; }
void bad2(C c) { _Atomic(S3) x{c}; }
void bad3(_Atomic(S3) a) { _Atomic(S3) x = a; }
void bad4() { _Atomic(S3) x{}; }
void bad5() { _Atomic(S3) x{{1, 2, 3}}; }
void bad6(D d) { e = d; }
void bad7(C c) { e = c; }
void bad8(S3 s) { ai = s; ai = 1; }
