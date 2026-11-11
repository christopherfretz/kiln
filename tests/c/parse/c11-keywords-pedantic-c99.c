// C99 6.4.1: C99 has _Bool and _Complex; the C11 keywords are extensions
// under -pedantic ("is a C11 extension").
//@ std: c99
//@ args: -pedantic
//@ strict-diags
//@ diag: 15:1 warning "'_Static_assert' is a C11 extension"
//@ diag: 16:1 warning "'_Alignas' is a C11 extension"
//@ diag: 17:9 warning "'_Alignof' is a C11 extension"
//@ diag: 18:1 warning "'_Atomic' is a C11 extension"
//@ diag: 19:1 warning "'_Thread_local' is a C11 extension"
//@ diag: 20:1 warning "'_Noreturn' is a C11 extension"
//@ diag: 21:9 warning "'_Generic' is a C11 extension"
//@ diag: 22:1 warning "'_Atomic' is a C11 extension"
_Bool a;
_Static_assert(1, "x");
_Alignas(8) int b;
int c = _Alignof(int);
_Atomic int d;
_Thread_local int e;
_Noreturn void f(void);
int g = _Generic(1, int: 2);
_Atomic(int) h;
double _Complex z;
