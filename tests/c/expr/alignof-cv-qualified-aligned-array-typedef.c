// C23 6.5.4.5 (_Alignof), 6.7.4.1: qualifiers on an array type are its
// elements' and do not change alignment; the alignment of a cv-qualified
// array, or of an array of, a typedef carrying GNU
// `__attribute__((aligned))` is the typedef's. Found running GCC's C++
// testsuite (the C++ case; C shares the evaluator).
//@ expect: compile-ok
typedef double T[4][4] __attribute__((aligned(32)));
typedef const T CT;
typedef int I4[4];
typedef I4 I __attribute__((aligned(16)));
typedef double P[4][4];
_Static_assert(_Alignof(T) == 32, "");
_Static_assert(_Alignof(const T) == 32, "");
_Static_assert(_Alignof(volatile T) == 32, "");
_Static_assert(__alignof__(CT) == 32, "");
_Static_assert(_Alignof(CT[2]) == 32, "");
_Static_assert(_Alignof(I[3]) == 16, "");
_Static_assert(_Alignof(const P) == _Alignof(double), "");
_Static_assert(_Alignof(const T) != _Alignof(double), "");
