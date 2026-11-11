// C89 3.1.1: the C99 and C11 keywords _Bool, _Complex, _Static_assert,
// _Alignas, _Alignof, _Atomic, _Thread_local, _Noreturn and _Generic are
// extensions in C89 under -pedantic (each "is a C99/C11 extension"); the
// first line comment of this header is one too.
//@ std: c89
//@ args: -pedantic
//@ skip(gcc): GCC has no line comments in C89, and this header is one
//@ strict-diags
//@ diag: 1:1 warning "// comments are not allowed in this language"
//@ diag: 20:1 warning "'_Bool' is a C99 extension"
//@ diag: 21:1 warning "'_Static_assert' is a C11 extension"
//@ diag: 22:1 warning "'_Alignas' is a C11 extension"
//@ diag: 23:9 warning "'_Alignof' is a C11 extension"
//@ diag: 24:1 warning "'_Atomic' is a C11 extension"
//@ diag: 25:1 warning "'_Thread_local' is a C11 extension"
//@ diag: 26:1 warning "'_Noreturn' is a C11 extension"
//@ diag: 27:9 warning "'_Generic' is a C11 extension"
//@ diag: 28:1 warning "'_Atomic' is a C11 extension"
//@ diag: 29:8 warning "'_Complex' is a C99 extension"
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
